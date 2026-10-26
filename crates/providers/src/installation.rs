//! Selected executable provenance, digest verification and bounded version probes.
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
mod version_probe {
    use super::*;
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    use std::io::ErrorKind;
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command, Stdio};
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

    pub fn probe_version(
        request: VersionProbeRequest,
    ) -> Result<VersionEvidence, InstallationError> {
        if request.approval_reference.trim().is_empty()
            || request.approval_reference.len() > 256
            || !request.working_directory.is_absolute()
            || !request.working_directory.is_dir()
            || request.timeout.is_zero()
            || request.timeout > Duration::from_secs(10)
        {
            return Err(InstallationError::InvalidApproval);
        }
        verify_executable(&request.executable, &request.expected_sha256)?;
        let child = Command::new(&request.executable)
            .arg("--version")
            .current_dir(&request.working_directory)
            .env_clear()
            .envs(&request.environment)
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
        let mut evidence = VersionEvidence {
            source: VersionEvidenceSource::SelectedExecutableVersionProbe,
            executable_sha256: request.expected_sha256,
            observed_at_unix_ms: utc_millis(),
            state: VersionProbeState::InvalidOutput,
            version_line: None,
            exit_code: None,
            stdout_bytes: 0,
            stderr_bytes: 0,
            process_reaped: false,
        };
        let deadline = Instant::now() + request.timeout;
        let mut text = Vec::new();
        let mut exited = false;
        let mut stdout_eof = false;
        let mut stderr_eof = false;
        loop {
            if Instant::now() >= deadline {
                evidence.state = VersionProbeState::TimedOut;
                break;
            }
            let mut io_progress = false;
            for (stream, is_stdout) in [
                (&mut stdout as &mut dyn Read, true),
                (&mut stderr as &mut dyn Read, false),
            ] {
                for _ in 0..16 {
                    let mut bytes = [0u8; 4096];
                    match stream.read(&mut bytes) {
                        Ok(0) => {
                            if is_stdout {
                                stdout_eof = true;
                            } else {
                                stderr_eof = true;
                            }
                            break;
                        }
                        Ok(n) => {
                            io_progress = true;
                            if is_stdout {
                                evidence.stdout_bytes =
                                    evidence.stdout_bytes.saturating_add(n as u64);
                                if evidence.stdout_bytes > 4096 {
                                    evidence.state = VersionProbeState::OutputLimit;
                                    break;
                                }
                                text.extend_from_slice(&bytes[..n]);
                            } else {
                                evidence.stderr_bytes =
                                    evidence.stderr_bytes.saturating_add(n as u64);
                            }
                        }
                        Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                        Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                        Err(_) => return Err(InstallationError::Io),
                    }
                }
                if evidence.state == VersionProbeState::OutputLimit {
                    break;
                }
            }
            if evidence.state == VersionProbeState::OutputLimit {
                break;
            }
            if !exited && owned.finished()? {
                let status = owned.stop()?;
                evidence.exit_code = status.code();
                exited = true;
            }
            if exited && stdout_eof && stderr_eof {
                if evidence.exit_code != Some(0) {
                    evidence.state = VersionProbeState::ExitFailure;
                } else if let Ok(value) = std::str::from_utf8(&text) {
                    let value = value.trim();
                    if !value.is_empty()
                        && value.len() <= 256
                        && !value.chars().any(char::is_control)
                    {
                        evidence.state = VersionProbeState::Succeeded;
                        evidence.version_line = Some(value.into());
                    }
                }
                break;
            }
            if !io_progress {
                crate::transport::wait_for_io(
                    (!stdout_eof).then(|| stdout.as_raw_fd()),
                    (!stderr_eof).then(|| stderr.as_raw_fd()),
                    None,
                    deadline,
                )
                .map_err(|_| InstallationError::Io)?;
            }
        }
        if !owned.reaped {
            let _ = owned.stop()?;
        }
        evidence.process_reaped = owned.reaped;
        // Detect executable drift during even a successful probe. A matching
        // version string never overrides the approved digest.
        verify_executable(&request.executable, &evidence.executable_sha256)?;
        Ok(evidence)
    }
}
#[cfg(unix)]
pub use version_probe::{probe_version, VersionProbeRequest};
