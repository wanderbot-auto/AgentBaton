//! Trusted-host-approved fixed command observations, never a terminal, sandbox,
//! TaskAccepted, scientific correctness or Git/production authorization.
use crate::{
    open_root_without_symlinks,
    validation::{
        sha256, valid_id, validate_artifacts, ApprovedRule, ArtifactContractView, BoundArtifact,
        ValidationEvidence, ValidationPolicyContractView, ValidationSubject, ValidationView,
        VersionBinding,
    },
    WorkspaceIdentity, WorkspaceKind,
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{ErrorKind, Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::MetadataExt,
            process::{CommandExt, ExitStatusExt},
        },
    },
    process::{Child, ChildStdin, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub const VALIDATOR_ID: &str = "builtin.fixed-command.v1";
pub const SCOPE: &str = "approved_deterministic_command_only";
#[cfg(target_os = "linux")]
const IMAGE_POLICY: &str = "linux_sealed_elf_snapshot";
#[cfg(target_os = "macos")]
const IMAGE_POLICY: &str = "macos_root_protected_mach_o_path";
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
const IMAGE_POLICY: &str = "unsupported_platform";

/// The approved process identity, not proof of an account or OS sandbox.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ExecutionIdentity {
    pub real_uid: u32,
    pub effective_uid: u32,
    pub real_gid: u32,
    pub effective_gid: u32,
}
fn execution_identity() -> ExecutionIdentity {
    ExecutionIdentity {
        real_uid: unsafe { libc::getuid() },
        effective_uid: unsafe { libc::geteuid() },
        real_gid: unsafe { libc::getgid() },
        effective_gid: unsafe { libc::getegid() },
    }
}

const MAX_PROGRAM: u64 = 16 * 1024 * 1024;
const MAX_SCRIPT: u64 = 64 * 1024;
const MAX_ARTIFACT_TOTAL: u64 = 16 * 1024 * 1024;

/// Not Deserialize: script selection and its reviewed hash must come from the
/// host's authenticated approval store, not an Agent-supplied command payload.
#[derive(Clone)]
pub enum CommandMode {
    /// The trusted host must audit the fixed native program's behavior. A binary
    /// hash cannot prove it will not read more code/configuration. Known shells
    /// are rejected here, but that rejection is not an interpreter classifier.
    NativeExecutable,
    PosixShStdin {
        script: BoundArtifact,
    },
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct RequiredEnforcement {
    pub filesystem_isolation: bool,
    pub network_isolation: bool,
    pub process_tree_containment: bool,
    pub execution_time_input_immutability: bool,
    pub auxiliary_source_immutability: bool,
    pub sealed_executable_image: bool,
}
/// Raw environment values are deliberately neither Debug nor Serialize. Only
/// names/value digests enter the contract/evidence; the environment is not inherited.
#[derive(Clone)]
pub struct CommandSettings {
    pub mode: CommandMode,
    /// Exact literals. For PosixShStdin the approved actual argv adds -s,-- first.
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub expected_exit_codes: Vec<i32>,
    /// Monotonic budget from spawn initiation until a stop/exit is observed.
    /// This does not claim to interrupt a kernel syscall or bound artifact/Git preflight.
    pub wall_time_ms: u64,
    /// Additional bounded nonblocking cleanup/capture budget. Expiration keeps
    /// an unreaped original Child in CommandRun; it does not prove stopped.
    pub cleanup_time_ms: u64,
    /// Retained bytes, not an OS quota for all output or filesystem writes.
    pub stdout_limit: u32,
    pub stderr_limit: u32,
    pub require: RequiredEnforcement,
}
#[derive(Clone, Debug, Serialize)]
pub struct CommandContractView {
    pub validator_id: String,
    pub scope: String,
    pub file_contract: ValidationPolicyContractView,
    pub program: ArtifactContractView,
    pub script: Option<ArtifactContractView>,
    pub mode: String,
    pub argv0: String,
    pub image_policy: String,
    pub execution_identity: ExecutionIdentity,
    pub arguments: Vec<String>,
    pub cwd: WorkspaceIdentity,
    pub environment_sha256: BTreeMap<String, String>,
    pub expected_exit_codes: Vec<i32>,
    pub wall_time_ms: u64,
    pub cleanup_time_ms: u64,
    pub stdout_limit: u32,
    pub stderr_limit: u32,
    pub require: RequiredEnforcement,
}
/// A typed host boundary, not authentication of caller-provided approval labels.
/// No renderer/JSON constructor should call this before native owner verification.
/// ```compile_fail
/// use agentbaton_workspaces::command_validation::ApprovedCommandRule;
/// let forged: ApprovedCommandRule = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Clone)]
pub struct ApprovedCommandRule {
    approval_id: String,
    policy_revision: String,
    view: CommandContractView,
    digest: String,
    settings: CommandSettings,
    program: BoundArtifact,
    inputs: Vec<BoundArtifact>,
    outputs: Vec<BoundArtifact>,
    integrity: ApprovedRule,
}
impl ApprovedCommandRule {
    #[allow(clippy::too_many_arguments)]
    pub fn from_verified_policy(
        approval_id: String,
        policy_revision: String,
        subject: ValidationSubject,
        program: BoundArtifact,
        cwd: WorkspaceIdentity,
        inputs: Vec<BoundArtifact>,
        outputs: Vec<BoundArtifact>,
        mut settings: CommandSettings,
    ) -> Result<Self, CommandError> {
        let program_view = program.contract_view();
        let program_path = program_view
            .canonical_root
            .join(&program_view.requirement.relative_path);
        if !valid_id(&approval_id)
            || !valid_id(&policy_revision)
            || program_view.requirement.expected_sha256.is_none()
            || program_view.requirement.max_bytes > MAX_PROGRAM
            || inputs.is_empty()
            || inputs.len() > 15
            || outputs.is_empty()
            || outputs.len() > 16
            || settings.arguments.len() > 64
            || settings
                .arguments
                .iter()
                .any(|v| v.len() > 4096 || v.contains('\0'))
            || settings.arguments.iter().map(String::len).sum::<usize>() > 16 * 1024
            || settings.environment.len() > 32
            || settings
                .environment
                .iter()
                .any(|(k, v)| !environment_name(k) || v.len() > 4096 || v.contains('\0'))
            || settings
                .environment
                .values()
                .map(String::len)
                .sum::<usize>()
                > 16 * 1024
            || settings.expected_exit_codes.is_empty()
            || settings.expected_exit_codes.len() > 16
            || settings
                .expected_exit_codes
                .iter()
                .any(|v| !(0..=255).contains(v))
            || !(1..=60_000).contains(&settings.wall_time_ms)
            || !(1..=1000).contains(&settings.cleanup_time_ms)
            || settings.stdout_limit > 65_536
            || settings.stderr_limit > 65_536
            || cwd.canonical_root.to_str().is_none()
            || program_path.to_str().is_none()
        {
            return Err(CommandError::InvalidContract);
        }
        let mut total = 0u64;
        let mut locations = BTreeSet::new();
        locations.insert(program_path.clone());
        for artifact in inputs.iter().chain(&outputs) {
            let v = artifact.contract_view();
            total = total
                .checked_add(v.requirement.max_bytes)
                .ok_or(CommandError::InvalidContract)?;
            if v.requirement.max_bytes > 4 * 1024 * 1024
                || !locations.insert(v.canonical_root.join(v.requirement.relative_path))
            {
                return Err(CommandError::InvalidContract);
            }
        }
        if total > MAX_ARTIFACT_TOTAL {
            return Err(CommandError::InvalidContract);
        }
        let mut all_inputs = vec![program.clone()];
        all_inputs.extend(inputs);
        let (mode, script, arguments) = match &settings.mode {
            CommandMode::NativeExecutable => {
                ("native_executable", None, settings.arguments.clone())
            }
            CommandMode::PosixShStdin { script } => {
                let v = script.contract_view();
                if v.requirement.expected_sha256.is_none()
                    || v.requirement.max_bytes > MAX_SCRIPT
                    || !locations.insert(v.canonical_root.join(&v.requirement.relative_path))
                {
                    return Err(CommandError::InvalidContract);
                }
                all_inputs.push(script.clone());
                let mut args = vec!["-s".into(), "--".into()];
                args.extend(settings.arguments.clone());
                ("posix_sh_stdin", Some(v), args)
            }
        };
        settings.expected_exit_codes.sort_unstable();
        settings.expected_exit_codes.dedup();
        let integrity = ApprovedRule::from_verified_policy(
            approval_id.clone(),
            policy_revision.clone(),
            subject,
            &all_inputs,
            &outputs,
            None,
        )
        .map_err(|_| CommandError::InvalidContract)?;
        let view = CommandContractView {
            validator_id: VALIDATOR_ID.into(),
            scope: SCOPE.into(),
            file_contract: integrity.contract_view(),
            program: program_view,
            script,
            mode: mode.into(),
            argv0: program_path.to_string_lossy().into_owned(),
            image_policy: IMAGE_POLICY.into(),
            execution_identity: execution_identity(),
            arguments,
            cwd,
            environment_sha256: settings
                .environment
                .iter()
                .map(|(k, v)| (k.clone(), sha256(v.as_bytes())))
                .collect(),
            expected_exit_codes: settings.expected_exit_codes.clone(),
            wall_time_ms: settings.wall_time_ms,
            cleanup_time_ms: settings.cleanup_time_ms,
            stdout_limit: settings.stdout_limit,
            stderr_limit: settings.stderr_limit,
            require: settings.require.clone(),
        };
        let digest = sha256(&serde_json::to_vec(&view).map_err(|_| CommandError::InvalidContract)?);
        Ok(Self {
            approval_id,
            policy_revision,
            view,
            digest,
            settings,
            program,
            inputs: all_inputs,
            outputs,
            integrity,
        })
    }
    pub fn contract_view(&self) -> CommandContractView {
        self.view.clone()
    }
    pub fn contract_digest(&self) -> &str {
        &self.digest
    }
}
fn environment_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .enumerate()
            .all(|(i, b)| b.is_ascii_alphabetic() || b == b'_' || (i > 0 && b.is_ascii_digit()))
        && !name.starts_with("LD_")
        && !name.starts_with("DYLD_")
        && !matches!(
            name,
            "ENV"
                | "BASH_ENV"
                | "SHELLOPTS"
                | "BASHOPTS"
                | "PYTHONPATH"
                | "PYTHONHOME"
                | "NODE_OPTIONS"
                | "RUBYOPT"
                | "PERL5OPT"
                | "GCONV_PATH"
                | "LOCPATH"
        )
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandError {
    InvalidContract,
    EnforcementUnavailable,
    ProgramChanged,
    ScriptChanged,
    InputChanged,
    CwdChanged,
    UnsupportedProgram,
    SpawnFailed,
    Io,
}
impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "approved command rejected: {self:?}")
    }
}
impl std::error::Error for CommandError {}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunReason {
    Exited,
    CancelledBeforeSpawn,
    CancelRequested,
    DeadlineExceeded,
    CaptureLimit,
    IoFailure,
    CleanupUnknown,
    ArtifactContractFailed,
}
#[derive(Clone, Debug, Serialize)]
pub struct CaptureView {
    pub captured_bytes: u32,
    pub observed_bytes_at_least: u64,
    pub sha256_of_captured: String,
    pub truncated: bool,
    pub eof_observed: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct CleanupView {
    pub owned_leader_reaped: bool,
    pub owned_group_signal_sent: bool,
    pub all_capture_pipes_observed_eof: bool,
    pub descendant_containment: String,
    pub input_immutability: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct CommandValidationView {
    pub validator_id: String,
    pub scope: String,
    pub approval_id: String,
    pub policy_revision: String,
    pub contract: CommandContractView,
    pub contract_digest: String,
    pub process_started: bool,
    pub reason: RunReason,
    pub exit_code: Option<i32>,
    pub exit_signal: Option<i32>,
    pub stdout: CaptureView,
    pub stderr: CaptureView,
    pub cleanup: CleanupView,
    pub file_integrity: Option<ValidationView>,
    pub main_image: String,
    pub main_image_verified: bool,
    pub native_configuration_integrity: String,
    pub auxiliary_validator_source_integrity: String,
    pub protected_program: Option<ProtectedProgramObservation>,
    pub dynamic_dependency_integrity: String,
    pub transitive_execution_integrity: String,
    pub script_bytes_delivered: u64,
    pub process_and_cleanup_elapsed_ms: u64,
    pub task_acceptance: String,
    pub integration_authorized: bool,
    pub passed: bool,
    pub digest: String,
}
/// Owns any unreaped direct child. Unknown cleanup is not a stopped proof; the
/// host must retain/reconcile it, never blindly replay the command.
pub struct CommandRun {
    view: CommandValidationView,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    files: Option<ValidationEvidence>,
    pending: Option<OwnedChild>,
}
impl CommandRun {
    pub fn view(&self) -> CommandValidationView {
        self.view.clone()
    }
    pub fn captured_stdout(&self) -> &[u8] {
        &self.stdout
    }
    pub fn captured_stderr(&self) -> &[u8] {
        &self.stderr
    }
    /// Nonblocking reconciliation of only the retained original direct child.
    pub fn try_reap_owned_leader(&mut self) -> Result<bool, CommandError> {
        if let Some(child) = &mut self.pending {
            if child.finished()? {
                let status = child.reap()?;
                self.view.exit_code = status.code();
                self.view.exit_signal = status.signal();
                self.view.cleanup.owned_leader_reaped = true;
                self.pending = None;
                self.refresh_digest();
            }
        }
        Ok(self.view.cleanup.owned_leader_reaped)
    }
    pub fn verify_current(&self) -> Result<(), CommandError> {
        cwd_file(&self.view.contract)?;
        #[cfg(target_os = "macos")]
        {
            let artifact = BoundArtifact::from_verified_contract(&self.view.contract.program)
                .map_err(|_| CommandError::ProgramChanged)?;
            let bytes = artifact.read().map_err(|_| CommandError::ProgramChanged)?;
            let now = protected_program_view(&self.view.contract, &bytes)?;
            if Some(&now) != self.view.protected_program.as_ref() {
                return Err(CommandError::ProgramChanged);
            }
        }
        self.files
            .as_ref()
            .ok_or(CommandError::InputChanged)?
            .verify_current()
            .map_err(|_| CommandError::InputChanged)
    }
    /// Only the approved command/observed artifact contract. Not a scientific,
    /// TaskAccepted, Git adoption or deployment acceptance token.
    pub fn accept_command(
        &self,
        rule: &ApprovedCommandRule,
    ) -> Result<CommandContractAcceptance, CommandError> {
        if self.view.contract_digest != rule.digest
            || self.view.approval_id != rule.approval_id
            || self.view.policy_revision != rule.policy_revision
            || !self.view.passed
        {
            return Err(CommandError::InvalidContract);
        }
        self.verify_current()?;
        Ok(CommandContractAcceptance {
            scope: SCOPE.into(),
            validation_digest: self.view.digest.clone(),
            contract_digest: rule.digest.clone(),
        })
    }
    fn refresh_digest(&mut self) {
        self.view.digest.clear();
        self.view.digest =
            sha256(&serde_json::to_vec(&self.view).expect("bounded command evidence"));
    }
}
#[derive(Clone, Debug, Serialize)]
/// This token cannot be constructed from an Agent's success JSON.
/// ```compile_fail
/// use agentbaton_workspaces::command_validation::CommandContractAcceptance;
/// let forged: CommandContractAcceptance = serde_json::from_str("{}").unwrap();
/// ```
pub struct CommandContractAcceptance {
    scope: String,
    validation_digest: String,
    contract_digest: String,
}
impl CommandContractAcceptance {
    pub fn validation_digest(&self) -> &str {
        &self.validation_digest
    }
}
fn capture_view(bytes: &[u8], observed: u64, truncated: bool, eof: bool) -> CaptureView {
    CaptureView {
        captured_bytes: bytes.len() as u32,
        observed_bytes_at_least: observed,
        sha256_of_captured: sha256(bytes),
        truncated,
        eof_observed: eof,
    }
}
fn initial(rule: &ApprovedCommandRule, reason: RunReason) -> CommandRun {
    CommandRun {
        view: CommandValidationView {
            validator_id: VALIDATOR_ID.into(),
            scope: SCOPE.into(),
            approval_id: rule.approval_id.clone(),
            policy_revision: rule.policy_revision.clone(),
            contract: rule.view.clone(),
            contract_digest: rule.digest.clone(),
            process_started: false,
            reason,
            exit_code: None,
            exit_signal: None,
            stdout: capture_view(&[], 0, false, true),
            stderr: capture_view(&[], 0, false, true),
            cleanup: CleanupView {
                owned_leader_reaped: false,
                owned_group_signal_sent: false,
                all_capture_pipes_observed_eof: true,
                descendant_containment: "not_enforced_unknown".into(),
                input_immutability: "before_after_observations_only".into(),
            },
            file_integrity: None,
            main_image: "not_started".into(),
            main_image_verified: false,
            native_configuration_integrity: "not_established".into(),
            auxiliary_validator_source_integrity: "not_established".into(),
            protected_program: None,
            dynamic_dependency_integrity: "not_established".into(),
            transitive_execution_integrity: "not_established".into(),
            script_bytes_delivered: 0,
            process_and_cleanup_elapsed_ms: 0,
            task_acceptance: "not_established".into(),
            integration_authorized: false,
            passed: false,
            digest: String::new(),
        },
        stdout: vec![],
        stderr: vec![],
        files: None,
        pending: None,
    }
}
fn cwd_file(contract: &CommandContractView) -> Result<File, CommandError> {
    let directory = open_root_without_symlinks(&contract.cwd.canonical_root)
        .map_err(|_| CommandError::CwdChanged)?;
    let metadata = directory.metadata().map_err(|_| CommandError::CwdChanged)?;
    if (metadata.dev(), metadata.ino()) != (contract.cwd.device, contract.cwd.inode) {
        return Err(CommandError::CwdChanged);
    }
    let current =
        crate::inspect(&contract.cwd.canonical_root).map_err(|_| CommandError::CwdChanged)?;
    match (
        &contract.cwd.kind,
        &current.kind,
        &contract.file_contract.subject.version,
    ) {
        (WorkspaceKind::Directory, WorkspaceKind::Directory, VersionBinding::Data { .. }) => (),
        (
            WorkspaceKind::Directory,
            WorkspaceKind::Directory,
            VersionBinding::WorkspaceArtifacts { .. },
        ) => (),
        (
            WorkspaceKind::Git {
                common_dir,
                head,
                dirty: false,
            },
            WorkspaceKind::Git {
                common_dir: now,
                head: current,
                dirty: false,
            },
            version,
        ) if common_dir == now && head == current => match version {
            VersionBinding::Git {
                target_ref,
                expected_target_head,
                integration_candidate,
                tree,
                ..
            } => {
                if current != integration_candidate
                    || crate::git_text(
                        &contract.cwd.canonical_root,
                        &["rev-parse", "--verify", "HEAD^{tree}"],
                    )
                    .map_err(|_| CommandError::CwdChanged)?
                        != *tree
                    || crate::git_text(
                        &contract.cwd.canonical_root,
                        &["rev-parse", "--verify", target_ref],
                    )
                    .map_err(|_| CommandError::CwdChanged)?
                        != *expected_target_head
                {
                    return Err(CommandError::CwdChanged);
                }
            }
            VersionBinding::WorkspaceArtifacts { base_revision, .. }
                if current == base_revision => {}
            _ => return Err(CommandError::CwdChanged),
        },
        _ => return Err(CommandError::CwdChanged),
    }
    Ok(directory)
}
fn preflight(rule: &ApprovedCommandRule) -> Result<(Vec<u8>, Vec<u8>, File), CommandError> {
    if execution_identity() != rule.view.execution_identity {
        return Err(CommandError::EnforcementUnavailable);
    }
    // A host-wide SIGCHLD reaper would invalidate Child ownership/PGID safety.
    // The trusted embedding host must keep this disposition for the run lifetime.
    let mut disposition: libc::sigaction = unsafe { std::mem::zeroed() };
    if unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), &mut disposition) } < 0
        || disposition.sa_sigaction != libc::SIG_DFL
        || disposition.sa_flags & libc::SA_NOCLDWAIT != 0
    {
        return Err(CommandError::EnforcementUnavailable);
    }
    let required = &rule.settings.require;
    if required.filesystem_isolation
        || required.network_isolation
        || required.process_tree_containment
        || required.execution_time_input_immutability
        || required.auxiliary_source_immutability
    {
        return Err(CommandError::EnforcementUnavailable);
    }
    #[cfg(not(target_os = "linux"))]
    if required.sealed_executable_image {
        return Err(CommandError::EnforcementUnavailable);
    }
    let cwd = cwd_file(&rule.view)?;
    let program = rule
        .program
        .read()
        .map_err(|_| CommandError::ProgramChanged)?;
    if Some(sha256(&program)) != rule.view.program.requirement.expected_sha256 {
        return Err(CommandError::ProgramChanged);
    }
    let script = match &rule.settings.mode {
        CommandMode::NativeExecutable => vec![],
        CommandMode::PosixShStdin { script } => {
            let bytes = script.read().map_err(|_| CommandError::ScriptChanged)?;
            if Some(sha256(&bytes))
                != rule
                    .view
                    .script
                    .as_ref()
                    .unwrap()
                    .requirement
                    .expected_sha256
            {
                return Err(CommandError::ScriptChanged);
            }
            bytes
        }
    };
    for input in &rule.inputs {
        let v = input.contract_view();
        let bytes = input.read().map_err(|_| CommandError::InputChanged)?;
        if Some(sha256(&bytes)) != v.requirement.expected_sha256 {
            return Err(CommandError::InputChanged);
        }
    }
    Ok((program, script, cwd))
}

struct OwnedChild {
    child: Child,
    reaped: bool,
    group_signal_sent: bool,
}
impl OwnedChild {
    fn finished(&self) -> Result<bool, CommandError> {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // WNOWAIT preserves the original leader PID until all group signals end.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                self.child.id() as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result < 0 {
            return Err(CommandError::Io);
        }
        Ok(unsafe { info.si_pid() } != 0)
    }
    fn kill_owned(&mut self) {
        if !self.reaped && !self.group_signal_sent && self.finished().is_ok() {
            // Only the group we created; an unreaped original child prevents PID reuse.
            let status = unsafe { libc::kill(-(self.child.id() as i32), libc::SIGKILL) };
            self.group_signal_sent = status == 0;
            let _ = self.child.kill();
        }
    }
    fn reap(&mut self) -> Result<std::process::ExitStatus, CommandError> {
        if self.reaped || !self.finished()? {
            return Err(CommandError::Io);
        }
        let status = self.child.wait().map_err(|_| CommandError::Io)?;
        self.reaped = true;
        Ok(status)
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.reaped {
            self.kill_owned();
            if self.finished().unwrap_or(false) {
                let _ = self.reap();
            }
        }
    }
}
struct Capture<T> {
    pipe: Option<T>,
    bytes: Vec<u8>,
    observed: u64,
    truncated: bool,
    eof: bool,
    limit: usize,
    setup_failed: bool,
}
impl<T: Read + AsRawFd> Capture<T> {
    fn new(pipe: Option<T>, limit: u32) -> Self {
        let setup_failed = pipe
            .as_ref()
            .is_none_or(|p| nonblocking(p.as_raw_fd()).is_err());
        Self {
            pipe: if setup_failed { None } else { pipe },
            bytes: vec![],
            observed: 0,
            truncated: false,
            eof: false,
            limit: limit as usize,
            setup_failed,
        }
    }
    fn pump(&mut self) -> Result<bool, CommandError> {
        let Some(pipe) = self.pipe.as_mut() else {
            return Ok(false);
        };
        let mut buffer = [0u8; 4096];
        match pipe.read(&mut buffer) {
            Ok(0) => {
                self.eof = true;
                self.pipe.take();
                Ok(true)
            }
            Ok(n) => {
                self.observed = self.observed.saturating_add(n as u64);
                let remaining = self.limit.saturating_sub(self.bytes.len());
                self.bytes.extend_from_slice(&buffer[..remaining.min(n)]);
                if n > remaining {
                    self.truncated = true;
                    self.pipe.take();
                }
                Ok(true)
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => Ok(false),
            Err(e) if e.kind() == ErrorKind::Interrupted => Ok(true),
            Err(_) => Err(CommandError::Io),
        }
    }
    fn fd(&self) -> Option<i32> {
        self.pipe.as_ref().map(AsRawFd::as_raw_fd)
    }
    fn view(&self) -> CaptureView {
        capture_view(&self.bytes, self.observed, self.truncated, self.eof)
    }
}
fn nonblocking(fd: i32) -> Result<(), CommandError> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        Err(CommandError::Io)
    } else {
        Ok(())
    }
}
fn poll_pipes(
    stdout: Option<i32>,
    stderr: Option<i32>,
    stdin: Option<i32>,
    deadline: Instant,
) -> Result<(), CommandError> {
    let mut descriptors: Vec<libc::pollfd> = stdout
        .into_iter()
        .chain(stderr)
        .map(|fd| libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        })
        .collect();
    if let Some(fd) = stdin {
        descriptors.push(libc::pollfd {
            fd,
            events: libc::POLLOUT,
            revents: 0,
        });
    }
    let ms = deadline
        .saturating_duration_since(Instant::now())
        .as_millis()
        .min(5) as i32;
    let result = unsafe { libc::poll(descriptors.as_mut_ptr(), descriptors.len() as _, ms) };
    if result < 0 && std::io::Error::last_os_error().kind() != ErrorKind::Interrupted {
        Err(CommandError::Io)
    } else {
        Ok(())
    }
}
fn feed(
    stdin: &mut Option<ChildStdin>,
    script: &[u8],
    offset: &mut usize,
) -> Result<bool, CommandError> {
    let Some(pipe) = stdin.as_mut() else {
        return Ok(false);
    };
    if *offset == script.len() {
        stdin.take();
        return Ok(true);
    }
    let end = (*offset + 4096).min(script.len());
    match pipe.write(&script[*offset..end]) {
        Ok(0) => Err(CommandError::Io),
        Ok(n) => {
            *offset += n;
            if *offset == script.len() {
                stdin.take();
            }
            Ok(true)
        }
        Err(e) if e.kind() == ErrorKind::WouldBlock => Ok(false),
        Err(e) if e.kind() == ErrorKind::Interrupted => Ok(true),
        Err(_) => Err(CommandError::Io),
    }
}
#[cfg(target_os = "linux")]
fn executable_snapshot(bytes: &[u8]) -> Result<File, CommandError> {
    if !bytes.starts_with(b"\x7fELF") {
        return Err(CommandError::UnsupportedProgram);
    }
    let fd = unsafe {
        libc::memfd_create(
            c"baton-approved-validator".as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
        )
    };
    if fd < 0 {
        return Err(CommandError::EnforcementUnavailable);
    }
    let mut file = unsafe { <File as std::os::fd::FromRawFd>::from_raw_fd(fd) };
    file.write_all(bytes).map_err(|_| CommandError::Io)?;
    if unsafe { libc::fchmod(fd, 0o500) } < 0
        || unsafe {
            libc::fcntl(
                fd,
                libc::F_ADD_SEALS,
                libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL,
            )
        } < 0
    {
        return Err(CommandError::EnforcementUnavailable);
    }
    Ok(file)
}
/// Root-protected path observations for macOS. These do not cover privileged
/// OS updates, dynamic libraries, loader configuration, or hidden native state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProtectedProgramObservation {
    pub device: u64,
    pub inode: u64,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub bytes: u64,
    pub mtime: i64,
    pub mtime_nsec: i64,
    pub ctime: i64,
    pub ctime_nsec: i64,
}
#[cfg(target_os = "macos")]
fn protected_program(
    rule: &ApprovedCommandRule,
    bytes: &[u8],
) -> Result<ProtectedProgramObservation, CommandError> {
    protected_program_view(&rule.view, bytes)
}
#[cfg(target_os = "macos")]
fn protected_program_view(
    view: &CommandContractView,
    bytes: &[u8],
) -> Result<ProtectedProgramObservation, CommandError> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt, path::Path};
    let identity = execution_identity();
    if identity != view.execution_identity
        || identity.effective_uid == 0
        || identity.real_uid != identity.effective_uid
        || identity.real_gid != identity.effective_gid
    {
        return Err(CommandError::EnforcementUnavailable);
    }
    let magic: &[u8] = bytes.get(..4).ok_or(CommandError::UnsupportedProgram)?;
    if !matches!(
        magic,
        b"\xcf\xfa\xed\xfe"
            | b"\xfe\xed\xfa\xcf"
            | b"\xce\xfa\xed\xfe"
            | b"\xfe\xed\xfa\xce"
            | b"\xca\xfe\xba\xbe"
            | b"\xbe\xba\xfe\xca"
            | b"\xca\xfe\xba\xbf"
            | b"\xbf\xba\xfe\xca"
    ) {
        return Err(CommandError::UnsupportedProgram);
    }
    if Some(sha256(bytes)) != view.program.requirement.expected_sha256 {
        return Err(CommandError::ProgramChanged);
    }
    let path = Path::new(&view.argv0);
    if std::fs::canonicalize(path).map_err(|_| CommandError::ProgramChanged)? != path {
        return Err(CommandError::EnforcementUnavailable);
    }
    // Check every canonical ancestor including /. Mode bits alone miss ACLs;
    // access(W_OK) uses the same real/effective identity required above.
    for (index, component) in path.ancestors().enumerate() {
        let metadata =
            std::fs::symlink_metadata(component).map_err(|_| CommandError::ProgramChanged)?;
        if metadata.file_type().is_symlink()
            || metadata.uid() != 0
            || metadata.mode() & 0o022 != 0
            || (index == 0
                && (!metadata.is_file()
                    || metadata.mode() & 0o111 == 0
                    || metadata.mode() & 0o6000 != 0))
            || (index > 0 && !metadata.is_dir())
        {
            return Err(CommandError::EnforcementUnavailable);
        }
        let name = CString::new(component.as_os_str().as_bytes())
            .map_err(|_| CommandError::InvalidContract)?;
        if unsafe { libc::access(name.as_ptr(), libc::W_OK) } == 0 {
            return Err(CommandError::EnforcementUnavailable);
        }
        let error = std::io::Error::last_os_error().raw_os_error();
        if !matches!(
            error,
            Some(libc::EACCES) | Some(libc::EROFS) | Some(libc::EPERM)
        ) {
            return Err(CommandError::EnforcementUnavailable);
        }
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|_| CommandError::ProgramChanged)?;
    Ok(ProtectedProgramObservation {
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        uid: metadata.uid(),
        gid: metadata.gid(),
        bytes: metadata.len(),
        mtime: metadata.mtime(),
        mtime_nsec: metadata.mtime_nsec(),
        ctime: metadata.ctime(),
        ctime_nsec: metadata.ctime_nsec(),
    })
}
#[cfg(target_os = "linux")]
fn approved_shell(path: &str) -> bool {
    matches!(
        path,
        "/usr/bin/dash" | "/usr/bin/bash" | "/bin/bash" | "/bin/sh"
    )
}
#[cfg(not(target_os = "linux"))]
fn approved_shell(path: &str) -> bool {
    path == "/bin/sh"
}
/// Runs a fixed approved command. It does not retry, adopt changes, execute a
/// repository instruction or promote an output-reported success flag.
pub fn run_approved_command(
    rule: &ApprovedCommandRule,
    cancel: &AtomicBool,
) -> Result<CommandRun, CommandError> {
    if cancel.load(Ordering::Relaxed) {
        let mut run = initial(rule, RunReason::CancelledBeforeSpawn);
        run.refresh_digest();
        return Ok(run);
    }
    let (program, script, cwd) = preflight(rule)?;
    if matches!(rule.settings.mode, CommandMode::NativeExecutable)
        && approved_shell(&rule.view.argv0)
    {
        return Err(CommandError::UnsupportedProgram);
    }
    if cancel.load(Ordering::Relaxed) {
        let mut run = initial(rule, RunReason::CancelledBeforeSpawn);
        run.refresh_digest();
        return Ok(run);
    }
    #[cfg(target_os = "linux")]
    let image = executable_snapshot(&program)?;
    #[cfg(target_os = "linux")]
    let mut command = Command::new(format!("/proc/self/fd/{}", image.as_raw_fd()));
    #[cfg(target_os = "macos")]
    let protected = protected_program(rule, &program)?;
    #[cfg(target_os = "macos")]
    let mut command = Command::new(&rule.view.argv0);
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (program, script, cwd);
        return Err(CommandError::EnforcementUnavailable);
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        if matches!(rule.settings.mode, CommandMode::PosixShStdin { .. })
            && !approved_shell(&rule.view.argv0)
        {
            return Err(CommandError::UnsupportedProgram);
        }
        let cwd_fd = cwd.as_raw_fd();
        command
            .arg0(&rule.view.argv0)
            .args(&rule.view.arguments)
            .env_clear()
            .envs(&rule.settings.environment)
            .stdin(
                if matches!(rule.settings.mode, CommandMode::PosixShStdin { .. }) {
                    Stdio::piped()
                } else {
                    Stdio::null()
                },
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        // Only async-signal-safe fchdir in the fork child. The pinned descriptor
        // remains alive until spawn returns and is CLOEXEC thereafter.
        unsafe {
            command.pre_exec(move || {
                if libc::fchdir(cwd_fd) < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
        let execution_start = Instant::now();
        let child = command.spawn().map_err(|_| CommandError::SpawnFailed)?;
        let mut owned = OwnedChild {
            child,
            reaped: false,
            group_signal_sent: false,
        };
        let mut stdout = Capture::new(owned.child.stdout.take(), rule.settings.stdout_limit);
        let mut stderr = Capture::new(owned.child.stderr.take(), rule.settings.stderr_limit);
        let mut stdin = owned.child.stdin.take();
        let stdin_failed = stdin
            .as_ref()
            .is_some_and(|s| nonblocking(s.as_raw_fd()).is_err());
        if stdin_failed {
            stdin.take();
        }
        let mut offset = 0;
        let deadline = execution_start + Duration::from_millis(rule.settings.wall_time_ms);
        let mut reason = loop {
            if stdout.setup_failed || stderr.setup_failed || stdin_failed {
                break RunReason::IoFailure;
            }
            if cancel.load(Ordering::Relaxed) {
                break RunReason::CancelRequested;
            }
            if Instant::now() >= deadline {
                break RunReason::DeadlineExceeded;
            }
            let progress = match (
                stdout.pump(),
                stderr.pump(),
                feed(&mut stdin, &script, &mut offset),
            ) {
                (Ok(a), Ok(b), Ok(c)) => a || b || c,
                _ => break RunReason::IoFailure,
            };
            if stdout.truncated || stderr.truncated {
                break RunReason::CaptureLimit;
            }
            match owned.finished() {
                Ok(true) => break RunReason::Exited,
                Ok(false) => (),
                Err(_) => break RunReason::IoFailure,
            }
            if !progress
                && poll_pipes(
                    stdout.fd(),
                    stderr.fd(),
                    stdin.as_ref().map(AsRawFd::as_raw_fd),
                    deadline,
                )
                .is_err()
            {
                break RunReason::IoFailure;
            }
        };
        if reason == RunReason::Exited && offset != script.len() {
            reason = RunReason::IoFailure;
        }
        stdin.take();
        owned.kill_owned();
        let cleanup = Instant::now() + Duration::from_millis(rule.settings.cleanup_time_ms);
        let mut status = None;
        loop {
            let mut progress = false;
            if !owned.reaped && owned.finished().unwrap_or(false) {
                status = owned.reap().ok();
                progress = true;
            }
            if let Ok(p) = stdout.pump() {
                progress |= p;
            }
            if let Ok(p) = stderr.pump() {
                progress |= p;
            }
            if owned.reaped && stdout.pipe.is_none() && stderr.pipe.is_none() {
                break;
            }
            if Instant::now() >= cleanup {
                break;
            }
            if !progress {
                let _ = poll_pipes(stdout.fd(), stderr.fd(), None, cleanup);
            }
        }
        if (stdout.truncated || stderr.truncated) && reason == RunReason::Exited {
            reason = RunReason::CaptureLimit;
        }
        if (!owned.reaped || !stdout.eof || !stderr.eof) && reason == RunReason::Exited {
            reason = RunReason::CleanupUnknown;
        }
        let process_and_cleanup_elapsed_ms =
            execution_start.elapsed().as_millis().min(u64::MAX as u128) as u64;
        let files = validate_artifacts(
            &rule.integrity,
            rule.inputs.clone(),
            rule.outputs.clone(),
            None,
        )
        .ok();
        let stable = files.as_ref().is_some_and(|v| v.verify_current().is_ok())
            && cwd_file(&rule.view).is_ok();
        #[cfg(target_os = "linux")]
        let image_verified = true;
        #[cfg(target_os = "macos")]
        let image_verified = rule
            .program
            .read()
            .ok()
            .and_then(|bytes| protected_program(rule, &bytes).ok())
            .as_ref()
            == Some(&protected);
        let stable = stable && image_verified;
        let artifact_passed = files.as_ref().is_some_and(|v| v.view().passed);
        if (!stable || !artifact_passed) && reason == RunReason::Exited {
            reason = RunReason::ArtifactContractFailed;
        }
        let mut run = initial(rule, reason);
        run.view.process_started = true;
        run.view.script_bytes_delivered = offset as u64;
        run.view.process_and_cleanup_elapsed_ms = process_and_cleanup_elapsed_ms;
        run.view.main_image = IMAGE_POLICY.into();
        run.view.main_image_verified = image_verified;
        #[cfg(target_os = "macos")]
        {
            run.view.protected_program = Some(protected);
        }
        run.view.exit_code = status.as_ref().and_then(std::process::ExitStatus::code);
        run.view.exit_signal = status.as_ref().and_then(ExitStatusExt::signal);
        run.view.stdout = stdout.view();
        run.view.stderr = stderr.view();
        run.view.cleanup = CleanupView {
            owned_leader_reaped: owned.reaped,
            owned_group_signal_sent: owned.group_signal_sent,
            all_capture_pipes_observed_eof: stdout.eof && stderr.eof,
            descendant_containment: "not_enforced_unknown".into(),
            input_immutability: "before_after_observations_only".into(),
        };
        run.view.file_integrity = files.as_ref().map(ValidationEvidence::view);
        run.view.passed = reason == RunReason::Exited
            && owned.reaped
            && stable
            && artifact_passed
            && run
                .view
                .exit_code
                .is_some_and(|code| rule.settings.expected_exit_codes.contains(&code));
        run.stdout = stdout.bytes;
        run.stderr = stderr.bytes;
        run.files = files;
        if !owned.reaped {
            run.pending = Some(owned);
        }
        run.refresh_digest();
        Ok(run)
    }
}
