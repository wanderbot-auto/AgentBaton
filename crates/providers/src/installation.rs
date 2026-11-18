//! Selected executable provenance, digest verification and bounded diagnostic probes.
//! Hashes bind only the selected executable bytes, not loaded libraries, scripts
//! passed as argv, interpreter binaries or external configuration. Verification
//! immediately before path-based spawn reduces drift but does not close TOCTOU.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
#[cfg(not(unix))]
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct BinaryDigest(String);
impl BinaryDigest {
    pub fn parse(value: &str) -> Result<Self, InstallationError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(InstallationError::InvalidDigest);
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl<'de> Deserialize<'de> for BinaryDigest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value)
            .map_err(|_| serde::de::Error::custom("expected lowercase SHA-256 digest"))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallationError {
    InvalidPath,
    InvalidDigest,
    InvalidApproval,
    InvalidDescriptor,
    DigestMismatch,
    FileChanged,
    ResourceLimit,
    Io,
    StartupFailed,
}
impl fmt::Display for InstallationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "installation verification: {self:?}")
    }
}
impl std::error::Error for InstallationError {}

/// Inspect a user-selected canonical absolute executable. This does not grant
/// permission to execute it, discover installations, or read credential files.
pub fn executable_digest(path: &Path) -> Result<BinaryDigest, InstallationError> {
    if !path.is_absolute()
        || fs::canonicalize(path).map_err(|_| InstallationError::InvalidPath)? != path
    {
        return Err(InstallationError::InvalidPath);
    }
    #[cfg(unix)]
    let mut file = {
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| InstallationError::Io)?
    };
    #[cfg(not(unix))]
    let mut file = File::open(path).map_err(|_| InstallationError::Io)?;
    let before = file.metadata().map_err(|_| InstallationError::Io)?;
    if !before.is_file() {
        return Err(InstallationError::InvalidPath);
    }
    if before.len() > MAX_EXECUTABLE_BYTES {
        return Err(InstallationError::ResourceLimit);
    }
    let mut hasher = Sha256::new();
    let mut bytes = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = file.read(&mut bytes).map_err(|_| InstallationError::Io)?;
        if n == 0 {
            break;
        }
        total = total.saturating_add(n as u64);
        if total > MAX_EXECUTABLE_BYTES {
            return Err(InstallationError::ResourceLimit);
        }
        hasher.update(&bytes[..n]);
    }
    let after = file.metadata().map_err(|_| InstallationError::Io)?;
    if before.len() != after.len()
        || total != after.len()
        || before.modified().ok() != after.modified().ok()
    {
        return Err(InstallationError::FileChanged);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.dev() != after.dev()
            || before.ino() != after.ino()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
        {
            return Err(InstallationError::FileChanged);
        }
    }
    Ok(BinaryDigest(format!("{:x}", hasher.finalize())))
}
pub fn verify_executable(path: &Path, expected: &BinaryDigest) -> Result<(), InstallationError> {
    if executable_digest(path)? != *expected {
        return Err(InstallationError::DigestMismatch);
    }
    Ok(())
}
fn utc_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionProbeState {
    Succeeded,
    ExitFailure,
    TimedOut,
    OutputLimit,
    InvalidOutput,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionEvidence {
    pub source: VersionEvidenceSource,
    pub executable_sha256: BinaryDigest,
    pub observed_at_unix_ms: u64,
    pub state: VersionProbeState,
    /// Untrusted printable version response, not a claim of vendor identity.
    /// No stderr, parent environment values or full command output are retained.
    pub version_line: Option<String>,
    pub exit_code: Option<i32>,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub process_reaped: bool,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionEvidenceSource {
    SelectedExecutableVersionProbe,
}

#[cfg(unix)]
mod probes {
    use super::*;
    use crate::connections::ProtocolRoute;
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    use std::io::ErrorKind;
    use std::os::fd::AsRawFd;
    use std::os::unix::process::{CommandExt, ExitStatusExt};
    use std::process::{Child, Command, ExitStatus, Stdio};
    use std::time::{Duration, Instant};

    /// Approval must be resolved by the caller's trusted authorization store.
    /// `--version` runs code, even though it normally avoids model calls.
    /// The executable may itself access native config; env_clear is not a sandbox.
    #[derive(Clone)]
    pub struct VersionProbeRequest {
        pub executable: PathBuf,
        pub expected_sha256: BinaryDigest,
        pub working_directory: PathBuf,
        pub approval_reference: String,
        pub environment: BTreeMap<OsString, OsString>,
        pub timeout: Duration,
    }
    impl fmt::Debug for VersionProbeRequest {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("VersionProbeRequest")
                .field("executable", &self.executable)
                .field("expected_sha256", &self.expected_sha256)
                .field("working_directory", &self.working_directory)
                .field(
                    "environment_names",
                    &self.environment.keys().collect::<Vec<_>>(),
                )
                .finish_non_exhaustive()
        }
    }
    struct OwnedProbe {
        child: Child,
        reaped: bool,
    }
    impl OwnedProbe {
        fn finished(&self) -> Result<bool, InstallationError> {
            // SAFETY: zero is a valid initial siginfo_t; P_PID selects our direct
            // unreaped child, and WNOWAIT explicitly preserves PID ownership.
            let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
            let status = unsafe {
                libc::waitid(
                    libc::P_PID,
                    self.child.id() as libc::id_t,
                    &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if status < 0 {
                return Err(InstallationError::Io);
            }
            Ok(unsafe { info.si_pid() } != 0)
        }
        fn stop(&mut self) -> Result<std::process::ExitStatus, InstallationError> {
            if self.reaped {
                return Err(InstallationError::Io);
            }
            // SAFETY: this child owns the group created by process_group(0), and
            // waitid(WNOWAIT) never releases its PID before this group signal.
            unsafe {
                libc::kill(-(self.child.id() as i32), libc::SIGKILL);
            }
            let _ = self.child.kill();
            let result = self.child.wait().map_err(|_| InstallationError::Io);
            self.reaped = result.is_ok();
            result
        }
    }
    impl Drop for OwnedProbe {
        fn drop(&mut self) {
            if !self.reaped {
                let _ = self.stop();
            }
        }
    }

    const OUTPUT_LIMIT: u64 = 4096;

    struct StreamCapture {
        bytes: Vec<u8>,
        bytes_read: u64,
        limit: Option<u64>,
        retain: bool,
        eof: bool,
    }

    impl StreamCapture {
        fn new(limit: Option<u64>, retain: bool) -> Self {
            Self {
                bytes: Vec::new(),
                bytes_read: 0,
                limit,
                retain,
                eof: false,
            }
        }

        fn drain(&mut self, stream: &mut dyn Read) -> Result<bool, InstallationError> {
            let mut progressed = false;
            for _ in 0..16 {
                let mut chunk = [0u8; OUTPUT_LIMIT as usize];
                match stream.read(&mut chunk) {
                    Ok(0) => {
                        self.eof = true;
                        break;
                    }
                    Ok(n) => {
                        progressed = true;
                        self.bytes_read = self.bytes_read.saturating_add(n as u64);
                        if self.over_limit() {
                            break;
                        }
                        if self.retain {
                            self.bytes.extend_from_slice(&chunk[..n]);
                        }
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(_) => return Err(InstallationError::Io),
                }
            }
            Ok(progressed)
        }

        fn over_limit(&self) -> bool {
            self.limit.is_some_and(|limit| self.bytes_read > limit)
        }
    }

    enum ProbeEnd {
        Exited,
        TimedOut,
        OutputLimit,
    }

    enum ProbeOutput {
        Version,
        AuthStatus,
    }

    struct ProbeRun {
        end: ProbeEnd,
        observed_at_unix_ms: u64,
        exited: Option<ExitStatus>,
        stdout: StreamCapture,
        stderr: StreamCapture,
        process_reaped: bool,
    }

    fn validate_approval(reference: &str, timeout: Duration) -> Result<(), InstallationError> {
        if reference.trim().is_empty()
            || reference.len() > 256
            || timeout.is_zero()
            || timeout > Duration::from_secs(10)
        {
            return Err(InstallationError::InvalidApproval);
        }
        Ok(())
    }

    fn run_probe(
        mut command: Command,
        timeout: Duration,
        output: ProbeOutput,
    ) -> Result<ProbeRun, InstallationError> {
        let child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .map_err(|_| InstallationError::StartupFailed)?;
        let mut owned = OwnedProbe {
            child,
            reaped: false,
        };
        let mut stdout = owned.child.stdout.take().ok_or(InstallationError::Io)?;
        let mut stderr = owned.child.stderr.take().ok_or(InstallationError::Io)?;
        for fd in [stdout.as_raw_fd(), stderr.as_raw_fd()] {
            crate::transport::nonblocking(fd).map_err(|_| InstallationError::Io)?;
        }
        let observed_at_unix_ms = utc_millis();
        let deadline = Instant::now() + timeout;
        let (mut out, mut err) = match output {
            ProbeOutput::Version => (
                StreamCapture::new(Some(OUTPUT_LIMIT), true),
                StreamCapture::new(None, false),
            ),
            ProbeOutput::AuthStatus => (
                StreamCapture::new(Some(OUTPUT_LIMIT), false),
                StreamCapture::new(Some(OUTPUT_LIMIT), true),
            ),
        };
        let mut exited = None;
        let end = loop {
            if Instant::now() >= deadline {
                break ProbeEnd::TimedOut;
            }
            let stdout_progress = out.drain(&mut stdout)?;
            let stderr_progress = err.drain(&mut stderr)?;
            if out.over_limit() || err.over_limit() {
                break ProbeEnd::OutputLimit;
            }
            if exited.is_none() && owned.finished()? {
                exited = Some(owned.stop()?);
            }
            if exited.is_some() && out.eof && err.eof {
                break ProbeEnd::Exited;
            }
            if !stdout_progress && !stderr_progress {
                crate::transport::wait_for_io(
                    (!out.eof).then(|| stdout.as_raw_fd()),
                    (!err.eof).then(|| stderr.as_raw_fd()),
                    None,
                    deadline,
                )
                .map_err(|_| InstallationError::Io)?;
            }
        };
        if !owned.reaped {
            let _ = owned.stop()?;
        }
        Ok(ProbeRun {
            end,
            observed_at_unix_ms,
            exited,
            stdout: out,
            stderr: err,
            process_reaped: owned.reaped,
        })
    }

    pub fn probe_version(
        request: VersionProbeRequest,
    ) -> Result<VersionEvidence, InstallationError> {
        validate_approval(&request.approval_reference, request.timeout)?;
        if !request.working_directory.is_absolute() || !request.working_directory.is_dir() {
            return Err(InstallationError::InvalidApproval);
        }
        verify_executable(&request.executable, &request.expected_sha256)?;
        let mut command = Command::new(&request.executable);
        command
            .arg("--version")
            .current_dir(&request.working_directory)
            .env_clear()
            .envs(&request.environment);
        let run = run_probe(command, request.timeout, ProbeOutput::Version)?;
        let mut version_line = None;
        let exit_code = run.exited.and_then(|status| status.code());
        let state = match run.end {
            ProbeEnd::TimedOut => VersionProbeState::TimedOut,
            ProbeEnd::OutputLimit => VersionProbeState::OutputLimit,
            ProbeEnd::Exited if exit_code != Some(0) => VersionProbeState::ExitFailure,
            ProbeEnd::Exited => {
                version_line = std::str::from_utf8(&run.stdout.bytes)
                    .ok()
                    .map(str::trim)
                    .filter(|value| {
                        !value.is_empty()
                            && value.len() <= 256
                            && !value.chars().any(char::is_control)
                    })
                    .map(str::to_owned);
                if version_line.is_some() {
                    VersionProbeState::Succeeded
                } else {
                    VersionProbeState::InvalidOutput
                }
            }
        };
        // A matching version string never overrides the approved digest.
        verify_executable(&request.executable, &request.expected_sha256)?;
        Ok(VersionEvidence {
            source: VersionEvidenceSource::SelectedExecutableVersionProbe,
            executable_sha256: request.expected_sha256,
            observed_at_unix_ms: run.observed_at_unix_ms,
            state,
            version_line,
            exit_code,
            stdout_bytes: run.stdout.bytes_read,
            stderr_bytes: run.stderr.bytes_read,
            process_reaped: run.process_reaped,
        })
    }
    const REVIEWED_CODEX_SOURCE: &str = "970111f6cb62cc02fe36a82ea7d71b37a2889c97";
    const AUTH_STATUS_ARGS: [&str; 2] = ["login", "status"];

    /// This capability requires a verifier that enforces read-only credentials
    /// and denied network egress for the exact launch. It currently has no
    /// production constructor or external deserialization path.
    pub struct VerifiedAuthProbeContext {
        executable: PathBuf,
        executable_sha256: BinaryDigest,
        approval_reference: String,
        working_directory: PathBuf,
        codex_home: PathBuf,
        scratch_directory: PathBuf,
    }

    #[derive(Clone, Copy)]
    pub struct AuthStatusProbeRequest<'a> {
        pub route: ProtocolRoute,
        /// Source revision from a trusted build-provenance record.
        pub reviewed_source_revision: &'a str,
        pub executable: &'a Path,
        pub expected_sha256: &'a BinaryDigest,
        pub version: &'a VersionEvidence,
        /// Auth diagnostic approval is distinct from version-probe approval.
        pub approval_reference: &'a str,
        pub timeout: Duration,
        pub context: Option<&'a VerifiedAuthProbeContext>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum AuthStatusProbeState {
        Observed,
        UnsupportedRoute,
        UnreviewedBuild,
        UnverifiedContext,
        ExitFailure,
        Interrupted,
        TimedOut,
        OutputLimit,
        InvalidOutput,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum LocalAuthObservation {
        Unknown,
        CachedChatGptLogin,
        ApiKeyConfigured,
        NotLoggedInLocally,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum AuthIdentityVerification {
        Unknown,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct AuthStatusProcessEvidence {
        pub exit_code: Option<i32>,
        pub exit_signal: Option<i32>,
        pub stdout_bytes: u64,
        pub stderr_bytes: u64,
        /// Reaping evidence applies to the owned direct child.
        pub process_reaped: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct AuthStatusEvidence {
        pub state: AuthStatusProbeState,
        pub local_observation: LocalAuthObservation,
        pub identity_verification: AuthIdentityVerification,
        pub observed_at_unix_ms: u64,
        pub executable_sha256: Option<BinaryDigest>,
        pub process: Option<AuthStatusProcessEvidence>,
    }

    impl AuthStatusEvidence {
        fn not_run(state: AuthStatusProbeState) -> Self {
            Self {
                state,
                local_observation: LocalAuthObservation::Unknown,
                identity_verification: AuthIdentityVerification::Unknown,
                observed_at_unix_ms: utc_millis(),
                executable_sha256: None,
                process: None,
            }
        }
    }

    /// Independent CLI observation of local authentication state.
    pub fn probe_auth_status(
        request: AuthStatusProbeRequest<'_>,
    ) -> Result<AuthStatusEvidence, InstallationError> {
        use AuthStatusProbeState as State;
        validate_approval(request.approval_reference, request.timeout)?;
        if request.route != ProtocolRoute::CodexAppServer {
            return Ok(AuthStatusEvidence::not_run(State::UnsupportedRoute));
        }
        if request.reviewed_source_revision != REVIEWED_CODEX_SOURCE
            || &request.version.executable_sha256 != request.expected_sha256
            || request.version.state != VersionProbeState::Succeeded
            || request.version.exit_code != Some(0)
            || !request.version.process_reaped
            // The audited source has a development version. The reported
            // label alone does not establish binary provenance.
            || !matches!(
                request.version.version_line.as_deref(),
                Some("codex-cli 0.0.0" | "codex 0.0.0")
            )
        {
            return Ok(AuthStatusEvidence::not_run(State::UnreviewedBuild));
        }
        let Some(context) = request.context else {
            return Ok(AuthStatusEvidence::not_run(State::UnverifiedContext));
        };
        if context.approval_reference != request.approval_reference
            || context.executable != request.executable
            || &context.executable_sha256 != request.expected_sha256
        {
            return Err(InstallationError::InvalidApproval);
        }
        for directory in [
            &context.working_directory,
            &context.codex_home,
            &context.scratch_directory,
        ] {
            if !directory.is_dir()
                || fs::canonicalize(directory).map_err(|_| InstallationError::InvalidPath)?
                    != *directory
            {
                return Err(InstallationError::InvalidPath);
            }
        }
        verify_executable(request.executable, request.expected_sha256)?;
        let mut command = Command::new(request.executable);
        command
            .args(AUTH_STATUS_ARGS)
            .current_dir(&context.working_directory)
            .env_clear()
            .env("CODEX_HOME", &context.codex_home)
            .env("TMPDIR", &context.scratch_directory);
        let run = run_probe(command, request.timeout, ProbeOutput::AuthStatus)?;
        verify_executable(request.executable, request.expected_sha256)?;
        Ok(project_auth_run(&run, request.expected_sha256.clone()))
    }

    fn project_auth_run(run: &ProbeRun, digest: BinaryDigest) -> AuthStatusEvidence {
        use AuthStatusProbeState as State;
        use LocalAuthObservation as Observation;
        let exit_code = run.exited.and_then(|status| status.code());
        let exit_signal = run.exited.and_then(|status| status.signal());
        let (state, local_observation) = match run.end {
            ProbeEnd::TimedOut => (State::TimedOut, Observation::Unknown),
            ProbeEnd::OutputLimit => (State::OutputLimit, Observation::Unknown),
            ProbeEnd::Exited if exit_signal.is_some() => (State::Interrupted, Observation::Unknown),
            ProbeEnd::Exited => match std::str::from_utf8(&run.stderr.bytes) {
                Ok(text) if run.stdout.bytes_read == 0 => match (exit_code, text) {
                    (Some(0), "Logged in using ChatGPT\n") => {
                        (State::Observed, Observation::CachedChatGptLogin)
                    }
                    (Some(1), "Not logged in\n") => {
                        (State::Observed, Observation::NotLoggedInLocally)
                    }
                    (Some(0), text) if has_masked_api_key_status(text) => {
                        (State::Observed, Observation::ApiKeyConfigured)
                    }
                    (Some(0), _) => (State::InvalidOutput, Observation::Unknown),
                    (_, _) => (State::ExitFailure, Observation::Unknown),
                },
                _ => (State::InvalidOutput, Observation::Unknown),
            },
        };
        AuthStatusEvidence {
            state,
            local_observation,
            identity_verification: AuthIdentityVerification::Unknown,
            observed_at_unix_ms: run.observed_at_unix_ms,
            executable_sha256: Some(digest),
            process: Some(AuthStatusProcessEvidence {
                exit_code,
                exit_signal,
                stdout_bytes: run.stdout.bytes_read,
                stderr_bytes: run.stderr.bytes_read,
                process_reaped: run.process_reaped,
            }),
        }
    }

    fn has_masked_api_key_status(text: &str) -> bool {
        let Some(key) = text.strip_prefix("API key configured - ").and_then(|text| {
            text.strip_suffix("\nAuthentication is checked when you send a request.\n")
        }) else {
            return false;
        };
        let bytes = key.as_bytes();
        bytes == b"***"
            || (bytes.len() == 16
                && &bytes[8..11] == b"***"
                && bytes[..8]
                    .iter()
                    .chain(&bytes[11..])
                    .all(|b| b.is_ascii_alphanumeric() || matches!(*b, b'_' | b'-')))
    }

    #[cfg(test)]
    mod auth_fixture_tests {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        const PRIVATE: &str = "fixture-sensitive-output";

        struct Fixture {
            dir: tempfile::TempDir,
            context: VerifiedAuthProbeContext,
            version: VersionEvidence,
        }

        impl Fixture {
            fn new(mode: &str) -> Self {
                let dir = tempfile::tempdir().unwrap();
                let root = dir.path().canonicalize().unwrap();
                let executable = root.join("provider-fixture");
                let codex_home = root.join("profile");
                let scratch_directory = root.join("scratch");
                fs::create_dir(&codex_home).unwrap();
                fs::create_dir(&scratch_directory).unwrap();
                fs::write(codex_home.join("generated-input"), b"read-only fixture").unwrap();
                let python = Command::new("python3")
                    .args(["-c", "import sys; print(sys.executable)"])
                    .output()
                    .unwrap();
                assert!(python.status.success());
                let python =
                    fs::canonicalize(String::from_utf8(python.stdout).unwrap().trim()).unwrap();
                let script = format!(
                    r##"#!{}
import os, signal, sys, time
if sys.argv[1:] == ['--version']:
    print('codex-cli 0.0.0')
    sys.exit(0)
assert sys.argv[1:] == ['login', 'status']
root = os.getcwd()
assert os.environ['CODEX_HOME'] == os.path.join(root, 'profile')
assert os.environ['TMPDIR'] == os.path.join(root, 'scratch')
assert not any(name in os.environ for name in ('HOME', 'OPENAI_API_KEY', 'GH_TOKEN'))
assert os.read(0, 1) == b''
with open('probe.pid', 'w') as pid:
    pid.write(str(os.getpid()))
mode = {:?}
private = {:?}.encode()
chatgpt = b'Logged in using ChatGPT\n'
api = b'API key configured - '
api_note = b'\nAuthentication is checked when you send a request.\n'
if mode == 'chatgpt': os.write(2, chatgpt)
elif mode == 'short_api': os.write(2, api + b'***' + api_note)
elif mode == 'masked_api': os.write(2, api + b'privFst8***last5' + api_note)
elif mode == 'logged_out':
    os.write(2, b'Not logged in\n')
    sys.exit(1)
elif mode == 'exit_failure':
    os.write(2, private)
    sys.exit(7)
elif mode == 'invalid_utf8': os.write(2, b'\xff' + private)
elif mode == 'extra_text': os.write(2, chatgpt + private)
elif mode == 'unmasked_key': os.write(2, api + private + api_note)
elif mode == 'wrong_stream':
    os.write(1, private)
    os.write(2, chatgpt)
elif mode == 'wrong_exit': os.write(2, b'Not logged in\n')
elif mode == 'signal':
    os.write(2, private)
    os.kill(os.getpid(), signal.SIGTERM)
elif mode == 'timeout':
    os.write(2, private)
    time.sleep(60)
elif mode in ('flood_stdout', 'flood_stderr'):
    os.write(1 if mode == 'flood_stdout' else 2, private * 5000)
elif mode == 'selfchange':
    with open(__file__, 'a') as executable:
        executable.write('\n# binary changed during probe\n')
    os.write(2, private)
else: raise ValueError('unknown fixture mode')
"##,
                    python.display(),
                    mode,
                    PRIVATE
                );
                fs::write(&executable, script).unwrap();
                fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
                let digest = executable_digest(&executable).unwrap();
                let version = probe_version(VersionProbeRequest {
                    executable: executable.clone(),
                    expected_sha256: digest.clone(),
                    working_directory: root.clone(),
                    approval_reference: "generated-version-approval".into(),
                    environment: BTreeMap::new(),
                    timeout: Duration::from_secs(2),
                })
                .unwrap();
                assert_eq!(version.state, VersionProbeState::Succeeded);
                Self {
                    dir,
                    context: VerifiedAuthProbeContext {
                        executable,
                        executable_sha256: digest,
                        approval_reference: "generated-auth-approval".into(),
                        working_directory: root,
                        codex_home,
                        scratch_directory,
                    },
                    version,
                }
            }

            fn request(&self) -> AuthStatusProbeRequest<'_> {
                AuthStatusProbeRequest {
                    route: ProtocolRoute::CodexAppServer,
                    reviewed_source_revision: REVIEWED_CODEX_SOURCE,
                    executable: &self.context.executable,
                    expected_sha256: &self.context.executable_sha256,
                    version: &self.version,
                    approval_reference: &self.context.approval_reference,
                    timeout: Duration::from_secs(2),
                    context: Some(&self.context),
                }
            }

            fn outcome(&self) -> AuthStatusEvidence {
                let outcome = probe_auth_status(self.request()).unwrap();
                self.assert_reaped();
                assert_eq!(
                    outcome.identity_verification,
                    AuthIdentityVerification::Unknown
                );
                assert_eq!(
                    outcome.executable_sha256.as_ref(),
                    Some(&self.context.executable_sha256)
                );
                assert!(outcome.observed_at_unix_ms > 0);
                let process = outcome.process.as_ref().unwrap();
                assert!(process.process_reaped);
                let json = serde_json::to_string(&outcome).unwrap();
                let debug = format!("{outcome:?}");
                for text in [json, debug] {
                    for private in [PRIVATE, "privFst8", "last5", "Logged in using ChatGPT"] {
                        assert!(!text.contains(private));
                    }
                }
                assert_eq!(
                    fs::read(self.context.codex_home.join("generated-input")).unwrap(),
                    b"read-only fixture"
                );
                outcome
            }

            fn marker(&self) -> PathBuf {
                self.dir.path().join("probe.pid")
            }

            fn assert_reaped(&self) {
                let pid = fs::read_to_string(self.marker()).unwrap().parse().unwrap();
                // SAFETY: signal zero checks existence without delivering a signal.
                assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
                assert_eq!(
                    std::io::Error::last_os_error().raw_os_error(),
                    Some(libc::ESRCH)
                );
            }
        }

        #[test]
        fn generated_statuses_use_fixed_command_and_project_closed_local_hints() {
            use LocalAuthObservation::*;
            for (mode, expected, code) in [
                ("chatgpt", CachedChatGptLogin, 0),
                ("short_api", ApiKeyConfigured, 0),
                ("masked_api", ApiKeyConfigured, 0),
                ("logged_out", NotLoggedInLocally, 1),
            ] {
                let outcome = Fixture::new(mode).outcome();
                assert_eq!(outcome.state, AuthStatusProbeState::Observed, "{mode}");
                assert_eq!(outcome.local_observation, expected, "{mode}");
                let process = outcome.process.unwrap();
                assert_eq!(process.exit_code, Some(code), "{mode}");
                assert_eq!(process.stdout_bytes, 0);
                assert!(process.stderr_bytes > 0);
            }
        }

        #[test]
        fn generated_errors_and_ambiguous_output_keep_local_state_unknown() {
            use AuthStatusProbeState::*;
            for (mode, expected) in [
                ("exit_failure", ExitFailure),
                ("invalid_utf8", InvalidOutput),
                ("extra_text", InvalidOutput),
                ("unmasked_key", InvalidOutput),
                ("wrong_stream", InvalidOutput),
                ("wrong_exit", InvalidOutput),
                ("signal", Interrupted),
                ("flood_stdout", OutputLimit),
                ("flood_stderr", OutputLimit),
            ] {
                let outcome = Fixture::new(mode).outcome();
                assert_eq!(outcome.state, expected, "{mode}");
                assert_eq!(
                    outcome.local_observation,
                    LocalAuthObservation::Unknown,
                    "{mode}"
                );
                let process = outcome.process.unwrap();
                if mode == "signal" {
                    assert_eq!(process.exit_signal, Some(libc::SIGTERM));
                }
                let (stdout, stderr) = (process.stdout_bytes, process.stderr_bytes);
                assert!(stdout <= OUTPUT_LIMIT * 2 && stderr <= OUTPUT_LIMIT * 2);
                if expected == OutputLimit {
                    assert!(stdout > OUTPUT_LIMIT || stderr > OUTPUT_LIMIT);
                }
            }
        }

        #[test]
        fn generated_timeout_reaps_the_child_and_returns_unknown() {
            let fixture = Fixture::new("timeout");
            let started = Instant::now();
            let outcome = probe_auth_status(AuthStatusProbeRequest {
                timeout: Duration::from_millis(500),
                ..fixture.request()
            })
            .unwrap();
            assert!(started.elapsed() < Duration::from_secs(3));
            fixture.assert_reaped();
            assert_eq!(outcome.state, AuthStatusProbeState::TimedOut);
            assert_eq!(outcome.local_observation, LocalAuthObservation::Unknown);
            assert_eq!(
                outcome.identity_verification,
                AuthIdentityVerification::Unknown
            );
            assert!(outcome.process.as_ref().unwrap().process_reaped);
            assert!(!serde_json::to_string(&outcome).unwrap().contains(PRIVATE));
            assert!(!format!("{outcome:?}").contains(PRIVATE));
        }

        #[test]
        fn context_must_bind_approval_and_digest_and_stay_canonical() {
            let mut fixture = Fixture::new("chatgpt");
            let request = fixture.request();
            assert_eq!(
                probe_auth_status(AuthStatusProbeRequest {
                    approval_reference: "another-reference",
                    ..request
                }),
                Err(InstallationError::InvalidApproval)
            );
            let link = fixture.dir.path().join("profile-alias");
            std::os::unix::fs::symlink(&fixture.context.codex_home, &link).unwrap();
            fixture.context.codex_home = link;
            assert_eq!(
                probe_auth_status(fixture.request()),
                Err(InstallationError::InvalidPath)
            );
            assert!(!fixture.marker().exists());
        }

        #[test]
        fn binary_drift_refuses_launch_or_discards_projection_after_reaping() {
            let fixture = Fixture::new("chatgpt");
            fs::write(&fixture.context.executable, b"replaced bytes").unwrap();
            assert_eq!(
                probe_auth_status(fixture.request()),
                Err(InstallationError::DigestMismatch)
            );
            assert!(!fixture.marker().exists());

            let fixture = Fixture::new("selfchange");
            let error = probe_auth_status(fixture.request()).unwrap_err();
            assert_eq!(error, InstallationError::DigestMismatch);
            assert!(!error.to_string().contains(PRIVATE));
            fixture.assert_reaped();
        }
    }
}
#[cfg(unix)]
pub use probes::{
    probe_auth_status, probe_version, AuthIdentityVerification, AuthStatusEvidence,
    AuthStatusProbeRequest, AuthStatusProbeState, AuthStatusProcessEvidence, LocalAuthObservation,
    VerifiedAuthProbeContext, VersionProbeRequest,
};
