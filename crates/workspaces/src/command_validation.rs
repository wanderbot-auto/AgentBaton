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
    pub exit_observed_within_budget: bool,
    pub cleanup_completed_within_budget: bool,
    pub cancellation_observed: bool,
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
    /// Final ownership cleanup of this exact retained original child only.
    /// May wait beyond an expired execution budget until the kernel returns a
    /// real wait receipt. This never backfills timely cleanup, EOF, PASS or any
    /// descendant-containment claim and does not start/retry a command.
    pub fn stop_and_reap_owned_leader(&mut self) -> Result<(), CommandError> {
        let Some(owned) = self.pending.as_mut() else {
            return Ok(());
        };
        if !owned.reaped {
            // Observe without consuming PID ownership before any group signal.
            loop {
                let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
                let observed = unsafe {
                    libc::waitid(
                        libc::P_PID,
                        owned.child.id() as libc::id_t,
                        &mut info,
                        libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                    )
                };
                if observed >= 0 {
                    break;
                }
                if std::io::Error::last_os_error().kind() != ErrorKind::Interrupted {
                    return Err(CommandError::Io);
                }
            }
            if !owned.group_signal_sent {
                let status = unsafe { libc::kill(-(owned.child.id() as i32), libc::SIGKILL) };
                owned.group_signal_sent = status == 0;
            }
            let _ = owned.child.kill();
            let status = owned.child.wait().map_err(|_| CommandError::Io)?;
            owned.reaped = true;
            self.view.exit_code = status.code();
            self.view.exit_signal = status.signal();
        }
        self.view.cleanup.owned_leader_reaped = true;
        self.view.cleanup.owned_group_signal_sent |= owned.group_signal_sent;
        self.pending = None;
        self.refresh_digest();
        Ok(())
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
            || !self.view.exit_observed_within_budget
            || !self.view.cleanup_completed_within_budget
            || self.view.cancellation_observed
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
            exit_observed_within_budget: false,
            cleanup_completed_within_budget: false,
            cancellation_observed: reason == RunReason::CancelledBeforeSpawn,
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
    #[cfg(test)]
    preflight_test_control::wait_if_requested();
    Ok((program, script, cwd))
}

// A thread-local, unit-test-only latency control. No environment variable, IPC,
// test permission or delay hook exists in the default library artifact.
#[cfg(test)]
mod preflight_test_control {
    use std::{cell::Cell, time::Instant};
    thread_local! {
        pub(super) static WAIT_UNTIL: Cell<Option<Instant>> = const { Cell::new(None) };
        pub(super) static PRESPAWN_WAIT_UNTIL: Cell<Option<Instant>> = const { Cell::new(None) };
    }
    pub(super) fn wait_if_requested() {
        wait_until(WAIT_UNTIL.with(Cell::take));
    }
    pub(super) fn before_spawn_if_requested() {
        wait_until(PRESPAWN_WAIT_UNTIL.with(Cell::take));
    }
    fn wait_until(until: Option<Instant>) {
        if let Some(until) = until {
            loop {
                let remaining = until.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                std::thread::sleep(remaining);
            }
        }
    }
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
    run_approved_command_inner(rule, cancel, None)
}

/// Execute the same approved command with an additional absolute host runtime
/// reservation. It only narrows the approved process/cleanup budgets. Expiration
/// observed before actual spawn produces a not-started run; expiration after
/// spawn retains any unreaped original child for the host to reconcile.
///
/// Checkpoints do not interrupt a stalled kernel syscall or provide an OS quota.
pub fn run_approved_command_with_deadline(
    rule: &ApprovedCommandRule,
    cancel: &AtomicBool,
    deadline: Instant,
) -> Result<CommandRun, CommandError> {
    run_approved_command_inner(rule, cancel, Some(deadline))
}

fn not_started_if_stopped(
    rule: &ApprovedCommandRule,
    cancel: &AtomicBool,
    absolute_deadline: Option<Instant>,
) -> Option<CommandRun> {
    let reason = if cancel.load(Ordering::Relaxed) {
        RunReason::CancelledBeforeSpawn
    } else if absolute_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        RunReason::DeadlineExceeded
    } else {
        return None;
    };
    let mut run = initial(rule, reason);
    run.refresh_digest();
    Some(run)
}

fn narrowed_deadline(local: Instant, absolute: Option<Instant>) -> Instant {
    absolute.map_or(local, |deadline| local.min(deadline))
}

fn run_approved_command_inner(
    rule: &ApprovedCommandRule,
    cancel: &AtomicBool,
    absolute_deadline: Option<Instant>,
) -> Result<CommandRun, CommandError> {
    if let Some(run) = not_started_if_stopped(rule, cancel, absolute_deadline) {
        return Ok(run);
    }
    let (program, script, cwd) = preflight(rule)?;
    if matches!(rule.settings.mode, CommandMode::NativeExecutable)
        && approved_shell(&rule.view.argv0)
    {
        return Err(CommandError::UnsupportedProgram);
    }
    if let Some(run) = not_started_if_stopped(rule, cancel, absolute_deadline) {
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
        #[cfg(test)]
        preflight_test_control::before_spawn_if_requested();
        // Includes the full artifact preflight, executable snapshot/protection
        // checks and command construction. Never spawn after observing that the
        // host reservation expired during any of those operations.
        if absolute_deadline.is_some() {
            if let Some(run) = not_started_if_stopped(rule, cancel, absolute_deadline) {
                return Ok(run);
            }
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
        let deadline = narrowed_deadline(
            execution_start + Duration::from_millis(rule.settings.wall_time_ms),
            absolute_deadline,
        );
        let mut exit_observed_within_budget = false;
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
            // Nonblocking syscalls can still return after a scheduler/kernel
            // stall. Classification uses the time AFTER each observation, never
            // just the clock sampled before entering it.
            if cancel.load(Ordering::Relaxed) {
                break RunReason::CancelRequested;
            }
            if Instant::now() >= deadline {
                break RunReason::DeadlineExceeded;
            }
            if stdout.truncated || stderr.truncated {
                break RunReason::CaptureLimit;
            }
            let finished = owned.finished();
            let observed_at = Instant::now();
            if cancel.load(Ordering::Relaxed) {
                break RunReason::CancelRequested;
            }
            if observed_at >= deadline {
                break RunReason::DeadlineExceeded;
            }
            match finished {
                Ok(true) => {
                    exit_observed_within_budget = true;
                    break RunReason::Exited;
                }
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
        // Cleanup's fixed budget includes its ownership observation and signals.
        // It must not restart after a delayed kill_owned()/waitid observation.
        let cleanup = narrowed_deadline(
            Instant::now() + Duration::from_millis(rule.settings.cleanup_time_ms),
            absolute_deadline,
        );
        stdin.take();
        owned.kill_owned();
        let mut status = None;
        let mut cleanup_completed_within_budget = false;
        loop {
            if Instant::now() >= cleanup {
                break;
            }
            let mut progress = false;
            if !owned.reaped {
                let finished = owned.finished().unwrap_or(false);
                if Instant::now() >= cleanup {
                    break;
                }
                if finished {
                    status = owned.reap().ok();
                    progress = true;
                }
                if Instant::now() >= cleanup {
                    break;
                }
            }
            if let Ok(p) = stdout.pump() {
                progress |= p;
            }
            if Instant::now() >= cleanup {
                break;
            }
            if let Ok(p) = stderr.pump() {
                progress |= p;
            }
            // A late EOF/reap is an actual observation, but not proof that the
            // approved cleanup budget was met. Retain its facts without PASS.
            if Instant::now() >= cleanup {
                break;
            }
            if owned.reaped && stdout.pipe.is_none() && stderr.pipe.is_none() {
                cleanup_completed_within_budget = true;
                break;
            }
            if !progress {
                let _ = poll_pipes(stdout.fd(), stderr.fd(), None, cleanup);
            }
        }
        if (stdout.truncated || stderr.truncated) && reason == RunReason::Exited {
            reason = RunReason::CaptureLimit;
        }
        if (!cleanup_completed_within_budget || !owned.reaped || !stdout.eof || !stderr.eof)
            && reason == RunReason::Exited
        {
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
        // This is the completion boundary for cancellation. Artifact/Git reads
        // do not extend the process/cleanup budgets, and must not hide a stop
        // request received while the observations were being checked.
        let cancellation_observed =
            cancel.load(Ordering::Relaxed) || reason == RunReason::CancelRequested;
        if cancellation_observed && reason == RunReason::Exited {
            reason = RunReason::CancelRequested;
        }
        if absolute_deadline.is_some_and(|deadline| Instant::now() >= deadline)
            && reason == RunReason::Exited
        {
            reason = RunReason::DeadlineExceeded;
        }
        let mut run = initial(rule, reason);
        run.view.process_started = true;
        run.view.exit_observed_within_budget = exit_observed_within_budget;
        run.view.cleanup_completed_within_budget = cleanup_completed_within_budget;
        run.view.cancellation_observed = cancellation_observed;
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
            && exit_observed_within_budget
            && cleanup_completed_within_budget
            && !cancellation_observed
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

/// Readonly, non-authoritative observation from an actual stopped CommandRun.
/// It never changes PASS and cannot authorize a retry, adoption or a new target.
/// The host must independently prove current policy/lineage/descendant/fences.
pub struct CommandTargetDriftObservation {
    contract_digest: String,
    target_ref: String,
    from_head: String,
    to_head: String,
    candidate: String,
    tree: String,
    pins: Vec<DriftPin>,
    tree_witness: DriftTree,
}
impl CommandTargetDriftObservation {
    pub fn contract_digest(&self) -> &str {
        &self.contract_digest
    }
    pub fn target_ref(&self) -> &str {
        &self.target_ref
    }
    pub fn from_head(&self) -> &str {
        &self.from_head
    }
    pub fn to_head(&self) -> &str {
        &self.to_head
    }
    pub fn candidate(&self) -> &str {
        &self.candidate
    }
    pub fn tree(&self) -> &str {
        &self.tree
    }
    /// Metadata-only final seal, not a fresh correctness/authority decision.
    pub fn recheck(&self, cancel: &AtomicBool, deadline: Instant) -> Result<(), CommandError> {
        let control = crate::adoption::AdoptionControl::for_request(cancel, deadline);
        for pin in &self.pins {
            pin.recheck(&control)?;
        }
        self.tree_witness.recheck(&control)?;
        drift_check(&control)
    }
}
#[derive(PartialEq, Eq)]
struct DriftStamp {
    dev: u64,
    ino: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    bytes: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}
fn drift_stamp(m: &std::fs::Metadata) -> DriftStamp {
    DriftStamp {
        dev: m.dev(),
        ino: m.ino(),
        mode: m.mode(),
        uid: m.uid(),
        gid: m.gid(),
        links: m.nlink(),
        bytes: m.len(),
        mtime: m.mtime(),
        mtime_nsec: m.mtime_nsec(),
        ctime: m.ctime(),
        ctime_nsec: m.ctime_nsec(),
    }
}
fn drift_stat_number<T, U>(value: T) -> Result<U, CommandError>
where
    U: TryFrom<T>,
{
    U::try_from(value).map_err(|_| CommandError::InputChanged)
}
fn drift_stat_stamp(m: &libc::stat) -> Result<DriftStamp, CommandError> {
    Ok(DriftStamp {
        dev: drift_stat_number(m.st_dev)?,
        ino: drift_stat_number(m.st_ino)?,
        mode: drift_stat_number(m.st_mode)?,
        uid: m.st_uid,
        gid: m.st_gid,
        links: drift_stat_number(m.st_nlink)?,
        bytes: drift_stat_number(m.st_size)?,
        mtime: drift_stat_number(m.st_mtime)?,
        mtime_nsec: drift_stat_number(m.st_mtime_nsec)?,
        ctime: drift_stat_number(m.st_ctime)?,
        ctime_nsec: drift_stat_number(m.st_ctime_nsec)?,
    })
}
fn drift_check(c: &crate::adoption::AdoptionControl<'_>) -> Result<(), CommandError> {
    c.check().map_err(|_| CommandError::Io)
}
fn drift_root(
    path: &std::path::Path,
    c: &crate::adoption::AdoptionControl<'_>,
) -> Result<File, CommandError> {
    use std::{
        ffi::CString,
        os::{
            fd::FromRawFd,
            unix::{ffi::OsStrExt, fs::OpenOptionsExt},
        },
        path::Component,
    };
    drift_check(c)?;
    if !path.is_absolute()
        || path.as_os_str().len() > 4096
        || path
            .components()
            .any(|v| !matches!(v, Component::RootDir | Component::Normal(_)))
    {
        return Err(CommandError::InvalidContract);
    }
    let mut fd = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open("/")
        .map_err(|_| CommandError::InputChanged)?;
    drift_check(c)?;
    for component in path.components() {
        let Component::Normal(name) = component else {
            continue;
        };
        drift_check(c)?;
        let name = CString::new(name.as_bytes()).map_err(|_| CommandError::InvalidContract)?;
        let raw = unsafe {
            libc::openat(
                fd.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY
                    | libc::O_DIRECTORY
                    | libc::O_NOFOLLOW
                    | libc::O_CLOEXEC
                    | libc::O_NONBLOCK,
            )
        };
        if raw < 0 {
            return Err(CommandError::InputChanged);
        }
        fd = unsafe { File::from_raw_fd(raw) };
        drift_check(c)?;
    }
    Ok(fd)
}
struct DriftPin {
    parent: File,
    parent_path: std::path::PathBuf,
    parent_identity: (u64, u64),
    name: std::ffi::CString,
    held: Option<File>,
    stamp: DriftStamp,
}
impl DriftPin {
    fn capture(
        path: &std::path::Path,
        c: &crate::adoption::AdoptionControl<'_>,
        directory: bool,
    ) -> Result<Self, CommandError> {
        use std::{
            ffi::CString,
            os::{fd::FromRawFd, unix::ffi::OsStrExt},
        };
        let parent_path = if path == std::path::Path::new("/") && directory {
            path.to_owned()
        } else {
            path.parent()
                .ok_or(CommandError::InvalidContract)?
                .to_owned()
        };
        let parent = drift_root(&parent_path, c)?;
        let pm = parent.metadata().map_err(|_| CommandError::InputChanged)?;
        drift_check(c)?;
        let name = CString::new(if path == std::path::Path::new("/") && directory {
            b".".as_slice()
        } else {
            path.file_name()
                .ok_or(CommandError::InvalidContract)?
                .as_bytes()
        })
        .map_err(|_| CommandError::InvalidContract)?;
        let raw = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY
                    | libc::O_NOFOLLOW
                    | libc::O_CLOEXEC
                    | libc::O_NONBLOCK
                    | if directory { libc::O_DIRECTORY } else { 0 },
            )
        };
        let held = if raw >= 0 {
            Some(unsafe { File::from_raw_fd(raw) })
        } else if std::io::Error::last_os_error().kind() == ErrorKind::NotFound {
            None
        } else {
            return Err(CommandError::InputChanged);
        };
        drift_check(c)?;
        let stamp = if let Some(f) = &held {
            let m = f.metadata().map_err(|_| CommandError::InputChanged)?;
            if (directory && !m.is_dir()) || (!directory && (!m.is_file() || m.nlink() != 1)) {
                return Err(CommandError::InputChanged);
            }
            drift_stamp(&m)
        } else {
            drift_stamp(&pm)
        };
        let pin = Self {
            parent,
            parent_path,
            parent_identity: (pm.dev(), pm.ino()),
            name,
            held,
            stamp,
        };
        pin.recheck(c)?;
        Ok(pin)
    }
    fn recheck(&self, c: &crate::adoption::AdoptionControl<'_>) -> Result<(), CommandError> {
        drift_check(c)?;
        let current = drift_root(&self.parent_path, c)?;
        let pm = current.metadata().map_err(|_| CommandError::InputChanged)?;
        let held_parent = self
            .parent
            .metadata()
            .map_err(|_| CommandError::InputChanged)?;
        if (pm.dev(), pm.ino()) != self.parent_identity
            || (held_parent.dev(), held_parent.ino()) != self.parent_identity
        {
            return Err(CommandError::InputChanged);
        }
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::fstatat(
                current.as_raw_fd(),
                self.name.as_ptr(),
                &mut stat,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        let missing = result < 0 && std::io::Error::last_os_error().kind() == ErrorKind::NotFound;
        match &self.held {
            Some(held) => {
                if result < 0
                    || drift_stamp(&held.metadata().map_err(|_| CommandError::InputChanged)?)
                        != self.stamp
                    || drift_stat_stamp(&stat)? != self.stamp
                {
                    return Err(CommandError::InputChanged);
                }
            }
            None => {
                if !missing || drift_stamp(&pm) != self.stamp {
                    return Err(CommandError::InputChanged);
                }
            }
        }
        drift_check(c)
    }
}
// Complete bounded candidate data-tree mutation witness. No content/Git read
// runs on its final seal. Unsupported symlink/special/multilink entries fail
// this optional recovery path without changing ordinary validation semantics.
struct DriftTree {
    root: std::path::PathBuf,
    entries: Vec<(std::path::PathBuf, DriftStamp, bool)>,
}
impl DriftTree {
    fn capture(
        root: &std::path::Path,
        c: &crate::adoption::AdoptionControl<'_>,
    ) -> Result<Self, CommandError> {
        use std::{
            ffi::CStr,
            os::{fd::IntoRawFd, unix::ffi::OsStringExt},
        };
        struct Directory(*mut libc::DIR);
        impl Drop for Directory {
            fn drop(&mut self) {
                unsafe {
                    libc::closedir(self.0);
                }
            }
        }
        fn walk(
            tree: &mut DriftTree,
            relative: &std::path::Path,
            c: &crate::adoption::AdoptionControl<'_>,
            dirs: &mut usize,
            bytes: &mut usize,
        ) -> Result<(), CommandError> {
            drift_check(c)?;
            *dirs = dirs.checked_add(1).ok_or(CommandError::InvalidContract)?;
            if *dirs > 256 || relative.components().count() > 128 {
                return Err(CommandError::InvalidContract);
            }
            let path = tree.root.join(relative);
            let pin = DriftPin::capture(&path, c, true)?;
            tree.entries.push((relative.into(), pin.stamp, true));
            let fd = drift_root(&path, c)?.into_raw_fd();
            let stream = unsafe { libc::fdopendir(fd) };
            if stream.is_null() {
                unsafe {
                    libc::close(fd);
                }
                return Err(CommandError::InputChanged);
            }
            let stream = Directory(stream);
            loop {
                drift_check(c)?;
                #[cfg(target_os = "linux")]
                unsafe {
                    *libc::__errno_location() = 0;
                }
                #[cfg(target_os = "macos")]
                unsafe {
                    *libc::__error() = 0;
                }
                let entry = unsafe { libc::readdir(stream.0) };
                if entry.is_null() {
                    if std::io::Error::last_os_error().raw_os_error() != Some(0) {
                        return Err(CommandError::InputChanged);
                    }
                    break;
                }
                let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
                if name == b"."
                    || name == b".."
                    || (relative.as_os_str().is_empty() && name == b".git")
                {
                    continue;
                }
                let child = relative.join(std::ffi::OsString::from_vec(name.to_vec()));
                *bytes = bytes
                    .checked_add(child.as_os_str().len())
                    .ok_or(CommandError::InvalidContract)?;
                if *bytes > 1_048_576 || tree.entries.len() >= 8192 {
                    return Err(CommandError::InvalidContract);
                }

                let pin = DriftPin::capture(&tree.root.join(&child), c, false);
                // Opening a directory without O_DIRECTORY is safe; determine
                // its type from the nofollow descriptor, never lstat through a
                // potentially replaced/symlink ancestor.
                let is_directory = match pin {
                    Ok(pin) => {
                        tree.entries.push((child.clone(), pin.stamp, false));
                        false
                    }
                    Err(CommandError::InputChanged) => {
                        let directory = DriftPin::capture(&tree.root.join(&child), c, true)?;
                        if directory.held.is_none() {
                            return Err(CommandError::InputChanged);
                        }
                        true
                    }
                    Err(error) => return Err(error),
                };
                if is_directory {
                    walk(tree, &child, c, dirs, bytes)?;
                }
                drift_check(c)?;
            }
            Ok(())
        }
        let mut tree = Self {
            root: root.into(),
            entries: Vec::new(),
        };
        walk(&mut tree, std::path::Path::new(""), c, &mut 0, &mut 0)?;
        tree.recheck(c)?;
        Ok(tree)
    }
    fn recheck(&self, c: &crate::adoption::AdoptionControl<'_>) -> Result<(), CommandError> {
        for (relative, expected, directory) in &self.entries {
            drift_check(c)?;
            let pin = DriftPin::capture(&self.root.join(relative), c, *directory)?;
            if pin.held.is_none() || pin.stamp != *expected {
                return Err(CommandError::InputChanged);
            }
        }
        drift_check(c)
    }
}

fn drift_artifact(
    view: &ArtifactContractView,
    evidence: &crate::validation::ArtifactEvidence,
    c: &crate::adoption::AdoptionControl<'_>,
    keep_bytes: bool,
) -> Result<(Vec<DriftPin>, Vec<u8>), CommandError> {
    use sha2::{Digest, Sha256};
    use std::path::Component;
    drift_check(c)?;
    let relative = &view.requirement.relative_path;
    if relative.as_os_str().is_empty()
        || relative.as_os_str().len() > 4096
        || relative
            .components()
            .any(|v| !matches!(v, Component::Normal(_)))
        || view.requirement.max_bytes > MAX_PROGRAM
    {
        return Err(CommandError::InvalidContract);
    }
    let root_pin = DriftPin::capture(&view.canonical_root, c, true)?;
    let root = drift_root(&view.canonical_root, c)?;
    let m = root.metadata().map_err(|_| CommandError::InputChanged)?;
    drift_check(c)?;
    if (m.dev(), m.ino()) != (view.root_device, view.root_inode)
        || evidence.artifact_id != view.requirement.artifact_id
        || evidence.relative_path != *relative
        || evidence.root_device != view.root_device
        || evidence.root_inode != view.root_inode
        || evidence.expected_sha256 != view.requirement.expected_sha256
        || evidence.state != crate::validation::CheckState::Passed
    {
        return Err(CommandError::InputChanged);
    }
    // Parent traversal is nofollow at every component. The leaf owner remains
    // alive through the final Actor seal; all reads/hash chunks use this clock.
    let path = view.canonical_root.join(relative);
    let mut pin = DriftPin::capture(&path, c, false)?;
    let f = pin.held.as_mut().ok_or(CommandError::InputChanged)?;
    let mut digest = Sha256::new();
    let mut count = 0u64;
    let mut bytes = Vec::new();
    let mut chunk = [0; 65536];
    loop {
        drift_check(c)?;
        let n = match f.read(&mut chunk) {
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return Err(CommandError::InputChanged),
        };
        drift_check(c)?;
        if n == 0 {
            break;
        }
        count = count
            .checked_add(u64::try_from(n).map_err(|_| CommandError::InvalidContract)?)
            .ok_or(CommandError::InvalidContract)?;
        if count > view.requirement.max_bytes {
            return Err(CommandError::InputChanged);
        }
        digest.update(&chunk[..n]);
        if keep_bytes {
            let retain = (4usize.saturating_sub(bytes.len())).min(n);
            bytes.extend_from_slice(&chunk[..retain]);
        }
        drift_check(c)?;
    }
    if evidence.size != Some(count)
        || evidence.observed_sha256.as_deref() != Some(format!("{:x}", digest.finalize()).as_str())
    {
        return Err(CommandError::InputChanged);
    }
    pin.recheck(c)?;
    root_pin.recheck(c)?;
    Ok((vec![root_pin, pin], bytes))
}
fn drift_git(
    root: &std::path::Path,
    args: &[&str],
    c: &crate::adoption::AdoptionControl<'_>,
) -> Result<std::process::Output, CommandError> {
    drift_check(c)?;
    let result = crate::controlled_output(crate::git_command(root).args(args), c)
        .map_err(|_| CommandError::CwdChanged)?;
    drift_check(c)?;
    if result.interrupted || !result.io.io_complete() {
        return Err(CommandError::CwdChanged);
    }
    Ok(result.output)
}
fn drift_git_text(
    root: &std::path::Path,
    args: &[&str],
    c: &crate::adoption::AdoptionControl<'_>,
) -> Result<String, CommandError> {
    let result = drift_git(root, args, c)?;
    if !result.status.success() {
        return Err(CommandError::CwdChanged);
    }
    let text = String::from_utf8(result.stdout).map_err(|_| CommandError::CwdChanged)?;
    Ok(text.trim().to_owned())
}
#[cfg(target_os = "macos")]
fn drift_protected_program_view(
    view: &CommandContractView,
    bytes: &[u8],
    control: &crate::adoption::AdoptionControl<'_>,
) -> Result<ProtectedProgramObservation, CommandError> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt, path::Path};
    drift_check(control)?;
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
    // The exact approved SHA was already verified in controlled chunks.
    drift_check(control)?;
    let path = Path::new(&view.argv0);
    if std::fs::canonicalize(path).map_err(|_| CommandError::ProgramChanged)? != path {
        return Err(CommandError::EnforcementUnavailable);
    }
    drift_check(control)?;
    // Check every canonical ancestor including /. Mode bits alone miss ACLs;
    // access(W_OK) uses the same real/effective identity required above.
    for (index, component) in path.ancestors().enumerate() {
        drift_check(control)?;
        let metadata =
            std::fs::symlink_metadata(component).map_err(|_| CommandError::ProgramChanged)?;
        drift_check(control)?;
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
        drift_check(control)?;
        if !matches!(
            error,
            Some(libc::EACCES) | Some(libc::EROFS) | Some(libc::EPERM)
        ) {
            return Err(CommandError::EnforcementUnavailable);
        }
    }
    drift_check(control)?;
    let metadata = std::fs::symlink_metadata(path).map_err(|_| CommandError::ProgramChanged)?;
    drift_check(control)?;
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
impl CommandRun {
    pub fn observe_stopped_target_drift(
        &self,
        cancel: &AtomicBool,
        deadline: Instant,
    ) -> Result<Option<CommandTargetDriftObservation>, CommandError> {
        let c = crate::adoption::AdoptionControl::for_request(cancel, deadline);
        drift_check(&c)?;
        let v = &self.view;
        let contract = &v.contract;
        if self.pending.is_some()
            || !v.process_started
            || v.reason != RunReason::ArtifactContractFailed
            || !v.exit_observed_within_budget
            || !v.cleanup_completed_within_budget
            || v.cancellation_observed
            || !v.cleanup.owned_leader_reaped
            || !v.cleanup.all_capture_pipes_observed_eof
            || !v.stdout.eof_observed
            || !v.stderr.eof_observed
            || v.stdout.truncated
            || v.stderr.truncated
            || !v.main_image_verified
            || !v
                .exit_code
                .is_some_and(|code| contract.expected_exit_codes.contains(&code))
            || v.exit_signal.is_some()
            || execution_identity() != contract.execution_identity
        {
            return Ok(None);
        }
        let VersionBinding::Git {
            target_ref,
            expected_target_head,
            integration_candidate,
            tree,
            ..
        } = &contract.file_contract.subject.version
        else {
            return Ok(None);
        };
        let files = self
            .files
            .as_ref()
            .ok_or(CommandError::InputChanged)?
            .view();
        if !files.passed
            || files.policy_contract != contract.file_contract
            || v.file_integrity.as_ref() != Some(&files)
            || files.inputs.len() != contract.file_contract.inputs.len()
            || files.outputs.len() != contract.file_contract.outputs.len()
            || files.inputs.len() + files.outputs.len() > 64
        {
            return Ok(None);
        }
        let mut pins = Vec::new();
        let mut program_bytes = Vec::new();
        let mut total = 0u64;
        for (template, evidence) in contract
            .file_contract
            .inputs
            .iter()
            .zip(&files.inputs)
            .chain(contract.file_contract.outputs.iter().zip(&files.outputs))
        {
            total = total
                .checked_add(template.requirement.max_bytes)
                .ok_or(CommandError::InvalidContract)?;
            if total > MAX_ARTIFACT_TOTAL + MAX_PROGRAM + MAX_SCRIPT {
                return Err(CommandError::InvalidContract);
            }
            let is_program = template == &contract.program;
            let (artifact_pins, bytes) = drift_artifact(template, evidence, &c, is_program)?;
            pins.extend(artifact_pins);
            if is_program {
                program_bytes = bytes;
            }
        }
        if program_bytes.is_empty() {
            return Err(CommandError::ProgramChanged);
        }
        #[cfg(target_os = "macos")]
        for (index, ancestor) in std::path::Path::new(&contract.argv0)
            .ancestors()
            .enumerate()
        {
            if index >= 128 {
                return Err(CommandError::InvalidContract);
            }
            pins.push(DriftPin::capture(ancestor, &c, index > 0)?);
        }
        #[cfg(target_os = "macos")]
        if v.protected_program.as_ref()
            != Some(&drift_protected_program_view(contract, &program_bytes, &c)?)
        {
            return Err(CommandError::ProgramChanged);
        }
        let cwd = &contract.cwd;
        let WorkspaceKind::Git {
            common_dir,
            head,
            dirty: false,
        } = &cwd.kind
        else {
            return Ok(None);
        };
        if head != integration_candidate {
            return Err(CommandError::CwdChanged);
        }
        let admin = std::path::PathBuf::from(drift_git_text(
            &cwd.canonical_root,
            &["rev-parse", "--path-format=absolute", "--absolute-git-dir"],
            &c,
        )?);
        if !admin.is_absolute() || !admin.starts_with(common_dir) {
            return Err(CommandError::CwdChanged);
        }
        // Capture before inspect; never pair an old Git result with newer pins.
        for path in [
            admin.join("HEAD"),
            admin.join("index"),
            admin.join("commondir"),
            admin.join("config.worktree"),
            common_dir.join("config"),
            common_dir.join("packed-refs"),
            common_dir.join(target_ref),
        ] {
            pins.push(DriftPin::capture(&path, &c, false)?);
        }
        let git_entry = cwd.canonical_root.join(".git");
        drift_check(&c)?;
        let entry = std::fs::symlink_metadata(&git_entry).map_err(|_| CommandError::CwdChanged)?;
        drift_check(&c)?;
        pins.push(DriftPin::capture(&git_entry, &c, entry.is_dir())?);
        let tree_witness = DriftTree::capture(&cwd.canonical_root, &c)?;
        let now = crate::inspect_controlled(&cwd.canonical_root, &c)
            .map_err(|_| CommandError::CwdChanged)?;
        if now != *cwd {
            return Err(CommandError::CwdChanged);
        }
        if drift_git_text(
            &cwd.canonical_root,
            &["rev-parse", "--verify", "HEAD^{tree}"],
            &c,
        )? != *tree
        {
            return Err(CommandError::CwdChanged);
        }
        let symbolic = drift_git(
            &cwd.canonical_root,
            &["symbolic-ref", "--quiet", target_ref],
            &c,
        )?;
        if symbolic.status.code() != Some(1) || !symbolic.stdout.is_empty() {
            return Err(CommandError::CwdChanged);
        }
        let to = drift_git_text(
            &cwd.canonical_root,
            &["rev-parse", "--verify", target_ref],
            &c,
        )?;
        if !crate::is_oid(&to)
            || drift_git_text(&cwd.canonical_root, &["cat-file", "-t", &to], &c)? != "commit"
        {
            return Err(CommandError::CwdChanged);
        }
        if !crate::is_oid(&to) || to == *expected_target_head {
            return Ok(None);
        }
        let observation = CommandTargetDriftObservation {
            contract_digest: v.contract_digest.clone(),
            target_ref: target_ref.clone(),
            from_head: expected_target_head.clone(),
            to_head: to,
            candidate: integration_candidate.clone(),
            tree: tree.clone(),
            pins,
            tree_witness,
        };
        observation.recheck(cancel, deadline)?;
        Ok(Some(observation))
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod absolute_deadline_tests {
    use super::*;
    use crate::validation::ArtifactRequirement;
    use std::{fs, os::unix::fs::PermissionsExt, path::Path};

    fn fixture() -> (tempfile::TempDir, ApprovedCommandRule) {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(root.join("input"), b"fixed input").unwrap();
        fs::write(root.join("out"), b"fixed output").unwrap();
        let executable = Path::new("/usr/bin/touch")
            .canonicalize()
            .or_else(|_| Path::new("/bin/touch").canonicalize())
            .unwrap();
        let program = BoundArtifact::from_approved_root(
            executable.parent().unwrap(),
            ArtifactRequirement {
                artifact_id: "program".into(),
                relative_path: executable.file_name().unwrap().into(),
                max_bytes: MAX_PROGRAM,
                expected_sha256: Some(sha256(&fs::read(&executable).unwrap())),
            },
        )
        .unwrap();
        let artifact = |name: &str| {
            BoundArtifact::from_approved_root(
                &root,
                ArtifactRequirement {
                    artifact_id: name.into(),
                    relative_path: name.into(),
                    max_bytes: 4096,
                    expected_sha256: Some(sha256(&fs::read(root.join(name)).unwrap())),
                },
            )
            .unwrap()
        };
        let rule = ApprovedCommandRule::from_verified_policy(
            "approval".into(),
            "revision".into(),
            ValidationSubject {
                task_id: "task".into(),
                attempt_id: "producer".into(),
                spec_revision: 1,
                input_revision: "input-v1".into(),
                version: VersionBinding::Data {
                    dataset_revision: "dataset-v1".into(),
                },
            },
            program,
            crate::inspect(&root).unwrap(),
            vec![artifact("input")],
            vec![artifact("out")],
            CommandSettings {
                mode: CommandMode::NativeExecutable,
                arguments: vec!["started".into()],
                environment: BTreeMap::new(),
                expected_exit_codes: vec![0],
                wall_time_ms: 100,
                cleanup_time_ms: 50,
                stdout_limit: 4096,
                stderr_limit: 4096,
                require: RequiredEnforcement::default(),
            },
        )
        .unwrap();
        (directory, rule)
    }

    #[test]
    fn retained_unknown_leader_final_cleanup_really_waits_without_backfilling_pass() {
        let (_directory, rule) = fixture();
        let child = Command::new("/bin/sh")
            .args(["-c", "exec /bin/sleep 30"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap();
        let pid = child.id();
        let mut run = initial(&rule, RunReason::CleanupUnknown);
        run.view.process_started = true;
        run.view.cleanup.all_capture_pipes_observed_eof = false;
        run.pending = Some(OwnedChild {
            child,
            reaped: false,
            group_signal_sent: false,
        });
        run.stop_and_reap_owned_leader().unwrap();
        assert!(run.pending.is_none());
        assert!(run.view.cleanup.owned_leader_reaped);
        assert!(run.view.exit_signal.is_some() || run.view.exit_code.is_some());
        assert_eq!(run.view.reason, RunReason::CleanupUnknown);
        assert!(!run.view.passed);
        assert!(!run.view.exit_observed_within_budget);
        assert!(!run.view.cleanup_completed_within_budget);
        assert!(!run.view.cleanup.all_capture_pipes_observed_eof);
        #[cfg(target_os = "linux")]
        assert!(!std::path::Path::new("/proc").join(pid.to_string()).exists());
        #[cfg(not(target_os = "linux"))]
        let _ = pid;
        let digest = run.view.digest.clone();
        run.stop_and_reap_owned_leader().unwrap();
        assert_eq!(run.view.digest, digest);
    }

    #[test]
    fn valid_initial_host_budget_expiring_inside_preflight_or_image_preparation_never_spawns() {
        for stage in ["preflight", "prepared_image"] {
            let (directory, rule) = fixture();
            let deadline = Instant::now() + Duration::from_millis(500);
            assert!(
                deadline.saturating_duration_since(Instant::now())
                    > Duration::from_millis(
                        rule.settings.wall_time_ms + rule.settings.cleanup_time_ms
                    )
            );
            let until = deadline + Duration::from_millis(10);
            if stage == "preflight" {
                preflight_test_control::WAIT_UNTIL.with(|slot| slot.set(Some(until)));
            } else {
                preflight_test_control::PRESPAWN_WAIT_UNTIL.with(|slot| slot.set(Some(until)));
            }
            let run = run_approved_command_with_deadline(&rule, &AtomicBool::new(false), deadline)
                .unwrap();
            if stage == "preflight" {
                assert!(preflight_test_control::WAIT_UNTIL
                    .with(|slot| slot.get())
                    .is_none());
            } else {
                assert!(
                    preflight_test_control::PRESPAWN_WAIT_UNTIL
                        .with(|slot| slot.get())
                        .is_none(),
                    "must reach the prepared-image checkpoint"
                );
            }
            assert!(Instant::now() >= deadline);
            assert_eq!(run.view.reason, RunReason::DeadlineExceeded, "{stage}");
            assert!(!run.view.process_started, "{stage}");
            assert_eq!(run.view.main_image, "not_started");
            assert!(!run.view.passed);
            assert!(run.pending.is_none());
            assert!(run.files.is_none());
            assert!(
                !directory.path().join("started").exists(),
                "{stage}: executable must not run"
            );
        }
    }

    #[test]
    fn expired_or_cancelled_host_entry_is_not_started_and_legacy_entry_has_no_host_deadline() {
        let (directory, rule) = fixture();
        let run =
            run_approved_command_with_deadline(&rule, &AtomicBool::new(false), Instant::now())
                .unwrap();
        assert_eq!(run.view.reason, RunReason::DeadlineExceeded);
        assert!(!run.view.process_started);
        assert!(!directory.path().join("started").exists());
        let run = run_approved_command_with_deadline(
            &rule,
            &AtomicBool::new(true),
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(run.view.reason, RunReason::CancelledBeforeSpawn);
        assert!(!run.view.process_started);
        assert!(!directory.path().join("started").exists());
        preflight_test_control::WAIT_UNTIL
            .with(|slot| slot.set(Some(Instant::now() + Duration::from_millis(200))));
        let run = run_approved_command(&rule, &AtomicBool::new(false)).unwrap();
        assert!(run.view.process_started);
        assert!(run.view.passed);
        assert!(directory.path().join("started").exists());
        assert_eq!(run.view.contract.wall_time_ms, 100);
        assert_eq!(run.view.contract.cleanup_time_ms, 50);
    }
    #[test]
    fn drift_seal_rejects_checked_ancestor_mode_change_with_leaf_and_root_unchanged() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let ancestor = root.join("ancestor");
        fs::create_dir(&ancestor).unwrap();
        fs::write(ancestor.join("leaf"), b"unchanged leaf").unwrap();
        let root_before = drift_stamp(&fs::metadata(&root).unwrap());
        let leaf_before = drift_stamp(&fs::metadata(ancestor.join("leaf")).unwrap());
        let cancel = AtomicBool::new(false);
        let control = crate::adoption::AdoptionControl::for_request(
            &cancel,
            Instant::now() + Duration::from_secs(1),
        );
        let pin = DriftPin::capture(&ancestor, &control, true).unwrap();
        pin.recheck(&control).unwrap();
        let old = fs::metadata(&ancestor).unwrap().mode() & 0o7777;
        fs::set_permissions(
            &ancestor,
            fs::Permissions::from_mode(if old == 0o750 { 0o700 } else { 0o750 }),
        )
        .unwrap();
        assert!(drift_stamp(&fs::metadata(&root).unwrap()) == root_before);
        assert!(drift_stamp(&fs::metadata(ancestor.join("leaf")).unwrap()) == leaf_before);
        assert!(pin.recheck(&control).is_err());
    }
}
