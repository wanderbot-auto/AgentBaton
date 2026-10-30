//! Exact-head local Git candidate/adoption primitives. These never push, create
//! PRs, deploy or bypass remote protection. Approval must be resolved by trusted
//! host policy; labels supplied through this Rust API do not authenticate actors.
use crate::validation::{
    valid_hash, valid_id, Acceptance, ValidationError, ValidationEvidence, VersionBinding,
};
use crate::{
    bounded_output, check_adoption, git_command, inspect, is_oid, WorkspaceError,
    WorkspaceIdentity, WorkspaceKind,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum AdoptionError {
    Workspace(WorkspaceError),
    Validation(ValidationError),
    InvalidGrant,
    Conflict,
    StaleHead,
    TargetCheckedOut,
    UnsafeMergeDriver,
    SubjectMismatch,
}
impl From<WorkspaceError> for AdoptionError {
    fn from(error: WorkspaceError) -> Self {
        Self::Workspace(error)
    }
}
impl From<ValidationError> for AdoptionError {
    fn from(error: ValidationError) -> Self {
        Self::Validation(error)
    }
}
impl std::fmt::Display for AdoptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "integration rejected: {:?}", self)
    }
}
impl std::error::Error for AdoptionError {}

/// Observed IO facts, not process-tree containment, permission or an acceptance.
#[derive(Clone, Debug, Serialize)]
pub struct ControlledIntegrationIo {
    pub(crate) elapsed_ms: u64,
    pub(crate) stdout_retained_bytes: Option<u64>,
    pub(crate) stderr_retained_bytes: Option<u64>,
    pub(crate) stdout_eof: bool,
    pub(crate) stderr_eof: bool,
    pub(crate) owned_leader_reaped: Option<bool>,
    pub(crate) owned_group_signal_attempted: Option<bool>,
    pub(crate) interrupted: bool,
    pub(crate) read_failed: Option<bool>,
}
impl ControlledIntegrationIo {
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms
    }
    pub fn stdout_retained_bytes(&self) -> Option<u64> {
        self.stdout_retained_bytes
    }
    pub fn stderr_retained_bytes(&self) -> Option<u64> {
        self.stderr_retained_bytes
    }
    pub fn stdout_eof(&self) -> bool {
        self.stdout_eof
    }
    pub fn stderr_eof(&self) -> bool {
        self.stderr_eof
    }
    pub fn owned_leader_reaped(&self) -> Option<bool> {
        self.owned_leader_reaped
    }
    pub fn owned_group_signal_attempted(&self) -> Option<bool> {
        self.owned_group_signal_attempted
    }
    pub fn interrupted(&self) -> bool {
        self.interrupted
    }
    pub fn read_failed(&self) -> Option<bool> {
        self.read_failed
    }
    /// Only direct-child/stream receipt completeness, never an OS tree sandbox.
    pub fn io_complete(&self) -> bool {
        self.owned_leader_reaped == Some(true)
            && self.stdout_eof
            && self.stderr_eof
            && self.read_failed == Some(false)
    }
}
/// Keeps historical ref completion separate from resource/stream uncertainty.
/// This result cannot be deserialized into any authority or owned-stop proof.
pub struct ControlledIntegrationResult {
    record: IntegrationRecord,
    io: ControlledIntegrationIo,
}
impl ControlledIntegrationResult {
    pub fn record(&self) -> &IntegrationRecord {
        &self.record
    }
    pub fn io(&self) -> &ControlledIntegrationIo {
        &self.io
    }
}

/// Trusted host cancellation/deadline only; this is not an approval token.
/// It cannot be deserialized and never refreshes any caller clock.
pub struct AdoptionControl<'a> {
    cancel: &'a std::sync::atomic::AtomicBool,
    deadline: std::time::Instant,
    started: std::time::Instant,
    preparation: std::sync::Arc<std::sync::Mutex<PreparationFacts>>,
}
/// Private resource-controller provenance, never permission or a wire token.
#[derive(Clone)]
pub(crate) struct PreparationEpoch {
    cancel_identity: usize,
    started: std::time::Instant,
    facts: std::sync::Arc<std::sync::Mutex<PreparationFacts>>,
}
#[derive(Default)]
struct PreparationFacts {
    git_calls_attempted: u64,
    git_calls_complete: u64,
    stdout_lower_bound: u64,
    stderr_lower_bound: u64,
    effects_may_have_started: bool,
    interrupted: bool,
    overflow: bool,
}
/// Observed scalar preparation facts, not permission, acceptance or an atomic
/// filesystem snapshot. Missing complete IO remains None, never zero.
#[derive(Clone, Debug, Serialize)]
pub struct ControlledPreparationIo {
    elapsed_ms: u64,
    git_calls_attempted: u64,
    git_calls_complete: u64,
    git_stdout_observed_bytes: Option<u64>,
    git_stderr_observed_bytes: Option<u64>,
    effects_may_have_started: bool,
    interrupted: bool,
    io_complete: bool,
}
impl ControlledPreparationIo {
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms
    }
    pub fn git_stdout_observed_bytes(&self) -> Option<u64> {
        self.git_stdout_observed_bytes
    }
    pub fn git_stderr_observed_bytes(&self) -> Option<u64> {
        self.git_stderr_observed_bytes
    }
    pub fn effects_may_have_started(&self) -> bool {
        self.effects_may_have_started
    }
    pub fn io_complete(&self) -> bool {
        self.io_complete
    }
    pub fn interrupted(&self) -> bool {
        self.interrupted
    }
}

impl<'a> AdoptionControl<'a> {
    pub fn for_request(
        cancel: &'a std::sync::atomic::AtomicBool,
        deadline: std::time::Instant,
    ) -> Self {
        Self {
            cancel,
            deadline,
            started: std::time::Instant::now(),
            preparation: std::sync::Arc::new(std::sync::Mutex::new(PreparationFacts::default())),
        }
    }
    pub(crate) fn check(&self) -> Result<(), WorkspaceError> {
        if self.cancel.load(std::sync::atomic::Ordering::Acquire)
            || std::time::Instant::now() >= self.deadline
        {
            self.note_interrupted();
            Err(WorkspaceError::ResourceLimit)
        } else {
            Ok(())
        }
    }
    pub(crate) fn deadline(&self) -> std::time::Instant {
        self.deadline
    }
    pub(crate) fn cancel_identity(&self) -> usize {
        std::ptr::from_ref(self.cancel) as usize
    }
    pub fn narrowed(&self, deadline: std::time::Instant) -> Self {
        Self {
            cancel: self.cancel,
            deadline: self.deadline.min(deadline),
            started: self.started,
            preparation: self.preparation.clone(),
        }
    }
    pub(crate) fn epoch(&self) -> PreparationEpoch {
        PreparationEpoch {
            cancel_identity: self.cancel_identity(),
            started: self.started,
            facts: self.preparation.clone(),
        }
    }
    pub(crate) fn matches_epoch(&self, epoch: &PreparationEpoch) -> bool {
        self.cancel_identity() == epoch.cancel_identity
            && self.started == epoch.started
            && std::sync::Arc::ptr_eq(&self.preparation, &epoch.facts)
    }
    pub(crate) fn note_effects(&self) {
        self.preparation
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .effects_may_have_started = true;
    }
    pub(crate) fn note_git_attempt(&self) {
        let mut facts = self.preparation.lock().unwrap_or_else(|p| p.into_inner());
        match facts.git_calls_attempted.checked_add(1) {
            Some(n) => facts.git_calls_attempted = n,
            None => facts.overflow = true,
        }
    }
    pub(crate) fn note_git_io(&self, io: &ControlledIntegrationIo) {
        let mut facts = self.preparation.lock().unwrap_or_else(|p| p.into_inner());
        facts.interrupted |= io.interrupted;
        if io.io_complete() {
            match facts.git_calls_complete.checked_add(1) {
                Some(n) => facts.git_calls_complete = n,
                None => facts.overflow = true,
            }
        }
        for (is_stdout, bytes) in [
            (true, io.stdout_retained_bytes),
            (false, io.stderr_retained_bytes),
        ] {
            if let Some(bytes) = bytes {
                let before = if is_stdout {
                    facts.stdout_lower_bound
                } else {
                    facts.stderr_lower_bound
                };
                match before.checked_add(bytes) {
                    Some(total) => {
                        if is_stdout {
                            facts.stdout_lower_bound = total
                        } else {
                            facts.stderr_lower_bound = total
                        }
                    }
                    None => facts.overflow = true,
                }
            }
        }
    }
    pub(crate) fn note_interrupted(&self) {
        self.preparation
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .interrupted = true;
    }
    pub fn preparation_io(&self) -> ControlledPreparationIo {
        let poisoned = self.preparation.is_poisoned();
        let facts = self.preparation.lock().unwrap_or_else(|p| p.into_inner());
        let complete =
            !poisoned && !facts.overflow && facts.git_calls_attempted == facts.git_calls_complete;
        ControlledPreparationIo {
            elapsed_ms: u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX),
            git_calls_attempted: facts.git_calls_attempted,
            git_calls_complete: facts.git_calls_complete,
            git_stdout_observed_bytes: (complete && facts.git_calls_attempted > 0)
                .then_some(facts.stdout_lower_bound),
            git_stderr_observed_bytes: (complete && facts.git_calls_attempted > 0)
                .then_some(facts.stderr_lower_bound),
            effects_may_have_started: facts.effects_may_have_started,
            interrupted: facts.interrupted,
            io_complete: complete,
        }
    }
}
fn controlled_check(control: Option<&AdoptionControl<'_>>) -> Result<(), AdoptionError> {
    if let Some(c) = control {
        c.check()?;
    }
    Ok(())
}
fn bound_output(
    grant: &AuthorizedIntegration,
    args: &[&str],
    control: Option<&AdoptionControl<'_>>,
) -> Result<crate::ControlledGitOutput, WorkspaceError> {
    let mut command = bound_git_command(grant);
    match control {
        Some(c) => crate::controlled_output(command.args(args), c),
        None => {
            let start = std::time::Instant::now();
            let output = bounded_output(command.args(args))?;
            let io = ControlledIntegrationIo {
                elapsed_ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
                stdout_retained_bytes: Some(output.stdout.len() as u64),
                stderr_retained_bytes: Some(output.stderr.len() as u64),
                stdout_eof: true,
                stderr_eof: true,
                owned_leader_reaped: Some(true),
                owned_group_signal_attempted: Some(false),
                interrupted: false,
                read_failed: Some(false),
            };
            Ok(crate::ControlledGitOutput {
                output,
                interrupted: false,
                io,
            })
        }
    }
}
fn bound_text_controlled(
    grant: &AuthorizedIntegration,
    args: &[&str],
    control: Option<&AdoptionControl<'_>>,
) -> Result<String, AdoptionError> {
    controlled_check(control)?;
    let result = bound_output(grant, args, control)?;
    controlled_check(control)?;
    if result.interrupted || !result.output.status.success() {
        return Err(AdoptionError::Conflict);
    }
    String::from_utf8(result.output.stdout)
        .map(|s| s.trim().to_owned())
        .map_err(|_| AdoptionError::Conflict)
}
fn inspect_bound(
    root: &Path,
    control: Option<&AdoptionControl<'_>>,
) -> Result<WorkspaceIdentity, WorkspaceError> {
    match control {
        Some(c) => crate::inspect_controlled(root, c),
        None => inspect(root),
    }
}

pub struct AuthorizedIntegration {
    root: PathBuf,
    identity: (u64, u64),
    common_dir: PathBuf,
    common_identity: (u64, u64),
    target_ref: String,
    expected_target_head: String,
    source_candidate: String,
    grant_ref: String,
    commit_time_unix: u64,
    preparation_host_deadline: Option<std::time::Instant>,
    preparation_deadline: Option<std::time::Instant>,
    preparation_epoch: Option<PreparationEpoch>,
}
impl AuthorizedIntegration {
    /// Host policy must verify the grant's actor, project/repository, destination
    /// branch and exact candidate before calling. No main/remote authority inferred.
    pub fn from_verified_grant(
        root: &WorkspaceIdentity,
        target_ref: String,
        expected_target_head: String,
        source_candidate: String,
        grant_ref: String,
        commit_time_unix: u64,
    ) -> Result<Self, AdoptionError> {
        Self::from_verified_grant_inner(
            root,
            target_ref,
            expected_target_head,
            source_candidate,
            grant_ref,
            commit_time_unix,
            None,
        )
    }
    pub fn from_verified_grant_controlled(
        root: &WorkspaceIdentity,
        target_ref: String,
        expected_target_head: String,
        source_candidate: String,
        grant_ref: String,
        commit_time_unix: u64,
        control: &AdoptionControl<'_>,
    ) -> Result<Self, AdoptionError> {
        let narrowed =
            control.narrowed(std::time::Instant::now() + std::time::Duration::from_secs(15));
        let mut authorized = Self::from_verified_grant_inner(
            root,
            target_ref,
            expected_target_head,
            source_candidate,
            grant_ref,
            commit_time_unix,
            Some(&narrowed),
        )?;
        authorized.preparation_host_deadline = Some(control.deadline());
        Ok(authorized)
    }
    fn from_verified_grant_inner(
        root: &WorkspaceIdentity,
        target_ref: String,
        expected_target_head: String,
        source_candidate: String,
        grant_ref: String,
        commit_time_unix: u64,
        control: Option<&AdoptionControl<'_>>,
    ) -> Result<Self, AdoptionError> {
        controlled_check(control)?;
        if !matches!(root.kind, WorkspaceKind::Git { .. })
            || !target_ref.starts_with("refs/heads/")
            || !is_oid(&expected_target_head)
            || !is_oid(&source_candidate)
            || !valid_id(&grant_ref)
            || commit_time_unix > 4_294_967_295
        {
            return Err(AdoptionError::InvalidGrant);
        }
        let current = inspect_bound(&root.canonical_root, control)?;
        let WorkspaceKind::Git {
            common_dir: approved_common,
            ..
        } = &root.kind
        else {
            return Err(AdoptionError::InvalidGrant);
        };
        let WorkspaceKind::Git {
            common_dir: current_common,
            ..
        } = &current.kind
        else {
            return Err(AdoptionError::InvalidGrant);
        };
        if (current.device, current.inode) != (root.device, root.inode)
            || approved_common != current_common
        {
            return Err(AdoptionError::SubjectMismatch);
        }
        let common_metadata = fs::metadata(approved_common).map_err(WorkspaceError::Io)?;
        if !common_metadata.is_dir() {
            return Err(AdoptionError::SubjectMismatch);
        }
        controlled_check(control)?;
        let valid = crate::output_with_control(
            git_command(&root.canonical_root).args(["check-ref-format", target_ref.as_str()]),
            control,
        )?;
        if !valid.status.success() {
            return Err(AdoptionError::InvalidGrant);
        }
        Ok(Self {
            root: root.canonical_root.clone(),
            identity: (root.device, root.inode),
            common_dir: approved_common.clone(),
            common_identity: (common_metadata.dev(), common_metadata.ino()),
            target_ref,
            expected_target_head,
            source_candidate,
            grant_ref,
            commit_time_unix,
            preparation_host_deadline: control.map(|c| c.deadline()),
            preparation_deadline: control.map(|c| c.deadline()),
            preparation_epoch: control.map(AdoptionControl::epoch),
        })
    }
}
pub struct PreparedCandidate {
    authorization: AuthorizedIntegration,
    workspace: WorkspaceIdentity,
    integration_candidate: String,
    tree: String,
}
impl PreparedCandidate {
    pub fn workspace(&self) -> &WorkspaceIdentity {
        &self.workspace
    }
    pub fn version_binding(&self) -> VersionBinding {
        VersionBinding::Git {
            target_ref: self.authorization.target_ref.clone(),
            expected_target_head: self.authorization.expected_target_head.clone(),
            source_candidate: self.authorization.source_candidate.clone(),
            integration_candidate: self.integration_candidate.clone(),
            tree: self.tree.clone(),
        }
    }
    pub fn candidate_commit(&self) -> &str {
        &self.integration_candidate
    }
    pub fn expected_target_head(&self) -> &str {
        &self.authorization.expected_target_head
    }
}
fn bound_git_command(grant: &AuthorizedIntegration) -> std::process::Command {
    let mut command = git_command(&grant.root);
    // Never rediscover .git for mutations after checking the approved selector.
    command
        .arg("--git-dir")
        .arg(&grant.common_dir)
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_ALLOW_PROTOCOL", "")
        .args(["-c", "protocol.allow=never"]);
    command
}
fn create_bound_isolated(
    grant: &AuthorizedIntegration,
    managed_root: &Path,
    name: &str,
    commit: &str,
    control: Option<&AdoptionControl<'_>>,
) -> Result<WorkspaceIdentity, AdoptionError> {
    controlled_check(control)?;
    if name.is_empty()
        || name.starts_with('.')
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
        || !is_oid(commit)
    {
        return Err(WorkspaceError::InvalidPath.into());
    }
    let canonical = fs::canonicalize(managed_root).map_err(WorkspaceError::Io)?;
    if canonical != managed_root
        || !canonical.is_dir()
        || canonical.starts_with(&grant.root)
        || canonical.starts_with(&grant.common_dir)
    {
        return Err(WorkspaceError::InvalidPath.into());
    }
    let destination = canonical.join(name);
    match fs::symlink_metadata(&destination) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Err(WorkspaceError::InvalidPath.into()),
    }
    verify_repository_controlled(grant, control)?;
    controlled_check(control)?;
    if let Some(control) = control {
        control.note_effects();
    }
    let output = crate::output_with_control(
        bound_git_command(grant).args([
            std::ffi::OsStr::new("worktree"),
            std::ffi::OsStr::new("add"),
            std::ffi::OsStr::new("--detach"),
            std::ffi::OsStr::new("--"),
            destination.as_os_str(),
            std::ffi::OsStr::new(commit),
        ]),
        control,
    )?;
    if !output.status.success() {
        return Err(AdoptionError::Conflict);
    }
    Ok(inspect_bound(&destination, control)?)
}
fn verify_repository_controlled(
    grant: &AuthorizedIntegration,
    control: Option<&AdoptionControl<'_>>,
) -> Result<WorkspaceIdentity, AdoptionError> {
    controlled_check(control)?;
    let current = inspect_bound(&grant.root, control)?;
    if (current.device, current.inode) != grant.identity {
        return Err(AdoptionError::SubjectMismatch);
    }
    let WorkspaceKind::Git { common_dir, .. } = &current.kind else {
        return Err(AdoptionError::SubjectMismatch);
    };
    if common_dir != &grant.common_dir {
        return Err(AdoptionError::SubjectMismatch);
    }
    let common_metadata = fs::metadata(common_dir).map_err(WorkspaceError::Io)?;
    if (common_metadata.dev(), common_metadata.ino()) != grant.common_identity {
        return Err(AdoptionError::SubjectMismatch);
    }
    if matches!(current.kind, WorkspaceKind::Git { dirty: true, .. }) {
        return Err(WorkspaceError::Dirty.into());
    }
    reject_bound_unsafe_config_controlled(grant, control)?;
    let actual = bound_text_controlled(
        grant,
        &[
            "rev-parse",
            "--verify",
            &format!("{}^{{commit}}", grant.target_ref),
        ],
        control,
    )?;
    if actual != grant.expected_target_head {
        return Err(AdoptionError::StaleHead);
    }
    controlled_check(control)?;
    Ok(current)
}
fn reject_bound_unsafe_config_controlled(
    grant: &AuthorizedIntegration,
    control: Option<&AdoptionControl<'_>>,
) -> Result<(), AdoptionError> {
    // Counterfactual checkout contexts can enable includeIf rules absent from
    // the linked root. Refuse indirection rather than audit only one context.
    for (pattern, merge) in [
        ("^include(if\\..*)?\\.path$", false),
        ("^merge\\.", true),
        ("^filter\\.", false),
        (
            "^(extensions\\.partialclone|remote\\..*\\.promisor)$",
            false,
        ),
    ] {
        controlled_check(control)?;
        let result = bound_output(
            grant,
            &["config", "--no-includes", "--get-regexp", pattern],
            control,
        )?;
        controlled_check(control)?;
        let config = result.output;
        if result.interrupted || config.status.success() || config.status.code() != Some(1) {
            return Err(if merge {
                AdoptionError::UnsafeMergeDriver
            } else {
                AdoptionError::Workspace(WorkspaceError::UnsafeRepository)
            });
        }
    }
    Ok(())
}
fn reject_candidate_gitlinks(
    grant: &AuthorizedIntegration,
    candidate: &str,
    control: Option<&AdoptionControl<'_>>,
) -> Result<(), AdoptionError> {
    let tree = crate::output_with_control(
        bound_git_command(grant).args(["ls-tree", "-r", "-z", candidate]),
        control,
    )?;
    if !tree.status.success() {
        return Err(AdoptionError::Conflict);
    }
    if tree
        .stdout
        .split(|byte| *byte == 0)
        .any(|entry| entry.starts_with(b"160000 "))
    {
        return Err(AdoptionError::Conflict);
    }
    Ok(())
}
/// Merge tree objects without touching the user's index/worktree, create a fixed
/// integration commit and check it out only under the approved isolated root.
/// Configured custom merge drivers/checkout filters are rejected before merge.
pub fn prepare_candidate(
    grant: AuthorizedIntegration,
    managed_root: &Path,
    name: &str,
) -> Result<PreparedCandidate, AdoptionError> {
    prepare_candidate_inner(grant, managed_root, name, None)
}
/// Original construction deadline/cancel continues through candidate creation.
/// On Err, control.preparation_io preserves possible private object/worktree IO.
pub fn prepare_candidate_controlled(
    grant: AuthorizedIntegration,
    managed_root: &Path,
    name: &str,
    control: &AdoptionControl<'_>,
) -> Result<PreparedCandidate, AdoptionError> {
    if grant
        .preparation_host_deadline
        .is_none_or(|original| control.deadline() > original)
        || !grant
            .preparation_epoch
            .as_ref()
            .is_some_and(|epoch| control.matches_epoch(epoch))
    {
        return Err(AdoptionError::InvalidGrant);
    }
    let original = grant
        .preparation_deadline
        .ok_or(AdoptionError::InvalidGrant)?;
    let narrowed = control.narrowed(original);
    prepare_candidate_inner(grant, managed_root, name, Some(&narrowed))
}
fn prepare_candidate_inner(
    grant: AuthorizedIntegration,
    managed_root: &Path,
    name: &str,
    control: Option<&AdoptionControl<'_>>,
) -> Result<PreparedCandidate, AdoptionError> {
    verify_repository_controlled(&grant, control)?;
    reject_bound_unsafe_config_controlled(&grant, control)?;
    let source = bound_text_controlled(
        &grant,
        &[
            "rev-parse",
            "--verify",
            &format!("{}^{{commit}}", grant.source_candidate),
        ],
        control,
    )?;
    if source != grant.source_candidate {
        return Err(AdoptionError::SubjectMismatch);
    }
    reject_candidate_gitlinks(&grant, &grant.source_candidate, control)?;
    controlled_check(control)?;
    if let Some(control) = control {
        control.note_effects();
    }
    let merge = crate::output_with_control(
        bound_git_command(&grant).args([
            "-c",
            "merge.renormalize=false",
            "merge-tree",
            "--write-tree",
            grant.expected_target_head.as_str(),
            grant.source_candidate.as_str(),
        ]),
        control,
    )?;
    if !merge.status.success() {
        return Err(AdoptionError::Conflict);
    }
    let tree = String::from_utf8(merge.stdout)
        .map_err(|_| AdoptionError::Conflict)?
        .lines()
        .next()
        .ok_or(AdoptionError::Conflict)?
        .to_owned();
    if !is_oid(&tree) {
        return Err(AdoptionError::Conflict);
    }
    let mut commit = bound_git_command(&grant);
    let date = format!("@{} +0000", grant.commit_time_unix);
    commit
        .env("GIT_AUTHOR_NAME", "AgentBaton")
        .env("GIT_AUTHOR_EMAIL", "development@agentbaton.invalid")
        .env("GIT_COMMITTER_NAME", "AgentBaton")
        .env("GIT_COMMITTER_EMAIL", "development@agentbaton.invalid")
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .args([
            "commit-tree",
            tree.as_str(),
            "-p",
            grant.expected_target_head.as_str(),
        ]);
    if grant.source_candidate != grant.expected_target_head {
        commit.args(["-p", grant.source_candidate.as_str()]);
    }
    commit.args(["-m", "AgentBaton isolated integration candidate"]);
    verify_repository_controlled(&grant, control)?;
    controlled_check(control)?;
    if let Some(control) = control {
        control.note_effects();
    }
    let written = crate::output_with_control(&mut commit, control)?;
    if !written.status.success() {
        return Err(AdoptionError::Conflict);
    }
    let integration_candidate = String::from_utf8(written.stdout)
        .map_err(|_| AdoptionError::Conflict)?
        .trim()
        .to_owned();
    if !is_oid(&integration_candidate) {
        return Err(AdoptionError::Conflict);
    }
    // The target may have advanced during object preparation. Do not silently
    // validate against that changed target, even though user files were untouched.
    verify_repository_controlled(&grant, control)?;
    let workspace =
        create_bound_isolated(&grant, managed_root, name, &integration_candidate, control)?;
    verify_repository_controlled(&grant, control)?;
    Ok(PreparedCandidate {
        authorization: grant,
        workspace,
        integration_candidate,
        tree,
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationState {
    ConfirmedLocalRef,
    Rejected,
    OutcomeUnknown,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationRecord {
    pub grant_ref: String,
    pub target_ref: String,
    pub expected_target_head: String,
    pub candidate_commit: String,
    pub validation_digest: String,
    pub observed_head: Option<String>,
    pub state: IntegrationState,
}
fn checked_out(
    grant: &AuthorizedIntegration,
    control: Option<&AdoptionControl<'_>>,
) -> Result<bool, AdoptionError> {
    controlled_check(control)?;
    let result = bound_output(grant, &["worktree", "list", "--porcelain", "-z"], control)?;
    controlled_check(control)?;
    if result.interrupted {
        return Err(WorkspaceError::ResourceLimit.into());
    }
    let worktrees = result.output;
    if !worktrees.status.success() {
        return Err(AdoptionError::Conflict);
    }
    let expected = format!("branch {}", grant.target_ref);
    Ok(worktrees
        .stdout
        .split(|byte| *byte == 0)
        .any(|field| field == expected.as_bytes()))
}
/// Local compare-and-swap only, and only for a branch not checked out in any
/// worktree. Refusing checked-out targets preserves user index/worktree contents.
/// Callers must fence their own writers; this is not an OS sandbox or remote PR.
pub fn adopt_local_reference(
    candidate: &PreparedCandidate,
    evidence: &ValidationEvidence,
    acceptance: &Acceptance,
) -> Result<IntegrationRecord, AdoptionError> {
    adopt_inner(candidate, evidence, acceptance, None).map(|result| result.record)
}
/// Same independent authority and evidence guards, with one original host clock.
/// Post-write uncertainty is returned as a record, never a not-started error.
pub fn adopt_local_reference_controlled(
    candidate: &PreparedCandidate,
    evidence: &ValidationEvidence,
    acceptance: &Acceptance,
    control: &AdoptionControl<'_>,
) -> Result<ControlledIntegrationResult, AdoptionError> {
    adopt_inner(candidate, evidence, acceptance, Some(control))
}
fn adopt_inner(
    candidate: &PreparedCandidate,
    evidence: &ValidationEvidence,
    acceptance: &Acceptance,
    control: Option<&AdoptionControl<'_>>,
) -> Result<ControlledIntegrationResult, AdoptionError> {
    let started = std::time::Instant::now();
    let grant = &candidate.authorization;
    verify_repository_controlled(grant, control)?;
    if candidate.version_binding() != acceptance.subject().version || !acceptance.matches(evidence)
    {
        return Err(AdoptionError::SubjectMismatch);
    }
    controlled_check(control)?;
    evidence.verify_current()?;
    controlled_check(control)?;
    let current = match control {
        None => check_adoption(
            &candidate.workspace.canonical_root,
            &candidate.integration_candidate,
        )?,
        Some(c) => {
            let observed = crate::inspect_controlled(&candidate.workspace.canonical_root, c)?;
            match &observed.kind {
                WorkspaceKind::Git { head, dirty, .. }
                    if head == &candidate.integration_candidate && !dirty => {}
                _ => return Err(AdoptionError::SubjectMismatch),
            }
            observed
        }
    };
    if (current.device, current.inode) != (candidate.workspace.device, candidate.workspace.inode) {
        return Err(AdoptionError::SubjectMismatch);
    }
    let WorkspaceKind::Git { common_dir, .. } = &current.kind else {
        return Err(AdoptionError::SubjectMismatch);
    };
    if common_dir != &grant.common_dir {
        return Err(AdoptionError::SubjectMismatch);
    }

    if checked_out(grant, control)? {
        return Err(AdoptionError::TargetCheckedOut);
    }
    verify_repository_controlled(grant, control)?;
    controlled_check(control)?;
    let result = bound_output(
        grant,
        &[
            "update-ref",
            "-m",
            "AgentBaton validated local adoption",
            grant.target_ref.as_str(),
            candidate.integration_candidate.as_str(),
            grant.expected_target_head.as_str(),
        ],
        control,
    );
    // A joined exit0 is an actual update-ref completion receipt. Cancellation
    // after that completion does not erase history or fabricate a stopped-no-CAS.
    #[cfg(test)]
    controlled_completion_hook::apply(result.as_ref().is_ok_and(|r| r.output.status.success()));
    let confirmed_controlled =
        control.is_some() && result.as_ref().is_ok_and(|r| r.output.status.success());
    let observed = bound_text_controlled(
        grant,
        &["rev-parse", "--verify", grant.target_ref.as_str()],
        control,
    )
    .ok();
    let mut io = result
        .as_ref()
        .map(|r| r.io.clone())
        .unwrap_or(ControlledIntegrationIo {
            elapsed_ms: 0,
            stdout_retained_bytes: None,
            stderr_retained_bytes: None,
            stdout_eof: false,
            stderr_eof: false,
            owned_leader_reaped: None,
            owned_group_signal_attempted: None,
            interrupted: true,
            read_failed: None,
        });
    io.elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    io.interrupted |= control.is_some_and(|c| c.check().is_err());
    let state = if confirmed_controlled {
        IntegrationState::ConfirmedLocalRef
    } else {
        match result {
            Ok(output)
                if !output.interrupted
                    && output.output.status.success()
                    && observed.as_deref() == Some(&candidate.integration_candidate) =>
            {
                IntegrationState::ConfirmedLocalRef
            }
            Ok(output)
                if !output.interrupted
                    && !output.output.status.success()
                    && observed.as_deref() == Some(&grant.expected_target_head) =>
            {
                IntegrationState::Rejected
            }
            _ => IntegrationState::OutcomeUnknown,
        }
    };
    Ok(ControlledIntegrationResult {
        record: IntegrationRecord {
            grant_ref: grant.grant_ref.clone(),
            target_ref: grant.target_ref.clone(),
            expected_target_head: grant.expected_target_head.clone(),
            candidate_commit: candidate.integration_candidate.clone(),
            validation_digest: acceptance.validation_digest().into(),
            observed_head: observed,
            state,
        },
        io,
    })
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentClass {
    Development,
    Production,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseSubject {
    pub artifact_sha256: String,
    pub commit: String,
    pub environment_id: String,
    pub class: EnvironmentClass,
}
pub struct ConfirmedReleaseGrant {
    approval_ref: String,
    subject: ReleaseSubject,
    expires_at_unix: u64,
}
impl ConfirmedReleaseGrant {
    /// Trusted host must verify an explicit human confirmation for this exact
    /// subject. An accepted task or a development grant cannot create this token.
    pub fn from_verified_human_confirmation(
        approval_ref: String,
        subject: ReleaseSubject,
        expires_at_unix: u64,
    ) -> Result<Self, AdoptionError> {
        if !valid_id(&approval_ref)
            || !valid_hash(&subject.artifact_sha256)
            || !is_oid(&subject.commit)
            || !valid_id(&subject.environment_id)
            || expires_at_unix == 0
        {
            return Err(AdoptionError::InvalidGrant);
        }
        Ok(Self {
            approval_ref,
            subject,
            expires_at_unix,
        })
    }
    pub fn verify(&self, subject: &ReleaseSubject, now_unix: u64) -> Result<&str, AdoptionError> {
        if self.subject != *subject || now_unix >= self.expires_at_unix {
            return Err(AdoptionError::SubjectMismatch);
        }
        Ok(&self.approval_ref)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryEffectState {
    NotDispatched,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentDeliveryIntent {
    pub approval_ref: String,
    pub commit: String,
    pub destination_ref: String,
    pub environment_id: String,
    pub effect_state: DeliveryEffectState,
}
/// Records a bounded future development action. This performs no push/PR/deploy
/// and never labels an unexecuted or unknown external side effect successful.
pub fn plan_development_delivery(
    approval_ref: String,
    commit: String,
    destination_ref: String,
    environment_id: String,
    class: EnvironmentClass,
) -> Result<DevelopmentDeliveryIntent, AdoptionError> {
    if class != EnvironmentClass::Development
        || !valid_id(&approval_ref)
        || !is_oid(&commit)
        || !destination_ref.starts_with("refs/heads/")
        || !valid_id(&environment_id)
    {
        return Err(AdoptionError::InvalidGrant);
    }
    Ok(DevelopmentDeliveryIntent {
        approval_ref,
        commit,
        destination_ref,
        environment_id,
        effect_state: DeliveryEffectState::NotDispatched,
    })
}

#[cfg(test)]
mod controlled_completion_hook {
    type Hook = Box<dyn FnOnce(bool)>;
    std::thread_local! {static HOOK:std::cell::RefCell<Option<Hook>>=std::cell::RefCell::new(None);}
    pub(super) struct Guard;
    pub(super) fn install(hook: impl FnOnce(bool) + 'static) -> Guard {
        HOOK.with(|slot| {
            assert!(slot.borrow().is_none());
            *slot.borrow_mut() = Some(Box::new(hook));
        });
        Guard
    }
    pub(super) fn apply(joined_success: bool) {
        let hook = HOOK.with(|slot| slot.borrow_mut().take());
        if let Some(hook) = hook {
            hook(joined_success);
        }
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            HOOK.with(|slot| {
                slot.borrow_mut().take();
            });
        }
    }
}
#[cfg(test)]
mod controlled_completion_tests {
    use super::*;
    use crate::validation::{
        validate_artifacts, ApprovedRule, ArtifactRequirement, BoundArtifact, ValidationSubject,
    };
    use std::{
        process::Command,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::{Duration, Instant},
    };
    fn git(root: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap().trim().into()
    }
    fn completed_cas_remains_historical_success(late: bool) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let managed_directory = tempfile::tempdir().unwrap();
        let managed = managed_directory.path().canonicalize().unwrap();
        git(&root, &["init", "-b", "source"]);
        git(&root, &["config", "user.name", "Fixture"]);
        git(&root, &["config", "user.email", "fixture@example.invalid"]);
        fs::write(root.join("file.txt"), b"base\n").unwrap();
        git(&root, &["add", "file.txt"]);
        git(&root, &["commit", "-m", "base"]);
        let base = git(&root, &["rev-parse", "HEAD"]);
        git(&root, &["branch", "integration"]);
        fs::write(root.join("file.txt"), b"candidate\n").unwrap();
        git(&root, &["commit", "-am", "candidate"]);
        let source = git(&root, &["rev-parse", "HEAD"]);
        let grant = AuthorizedIntegration::from_verified_grant(
            &inspect(&root).unwrap(),
            "refs/heads/integration".into(),
            base,
            source,
            "owner:controlled-fixture".into(),
            1_700_000_000,
        )
        .unwrap();
        let candidate = prepare_candidate(grant, &managed, "candidate").unwrap();
        let outputs = vec![BoundArtifact::from_approved_root(
            &candidate.workspace().canonical_root,
            ArtifactRequirement {
                artifact_id: "file".into(),
                relative_path: "file.txt".into(),
                max_bytes: 4096,
                expected_sha256: None,
            },
        )
        .unwrap()];
        let rule = ApprovedRule::from_verified_policy(
            "owner:controlled-fixture".into(),
            "policy:v1".into(),
            ValidationSubject {
                task_id: "task".into(),
                attempt_id: "attempt".into(),
                spec_revision: 1,
                input_revision: "base".into(),
                version: candidate.version_binding(),
            },
            &[],
            &outputs,
            None,
        )
        .unwrap();
        let evidence = validate_artifacts(&rule, vec![], outputs, None).unwrap();
        let acceptance = evidence.accept(&rule).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let applied = Arc::new(AtomicBool::new(false));
        let deadline = Instant::now() + Duration::from_secs(5);
        let signal = cancel.clone();
        let proof = applied.clone();
        let _guard = controlled_completion_hook::install(move |joined_success| {
            assert!(
                joined_success,
                "the fixture only acts after the real update-ref child/readers joined with exit0"
            );
            if late {
                while Instant::now() < deadline {
                    std::thread::park_timeout(deadline.saturating_duration_since(Instant::now()));
                }
            }
            signal.store(true, Ordering::Release);
            proof.store(true, Ordering::Release);
        });
        let control = AdoptionControl::for_request(&cancel, deadline);
        let result =
            adopt_local_reference_controlled(&candidate, &evidence, &acceptance, &control).unwrap();
        assert!(applied.load(Ordering::Acquire));
        assert_eq!(result.record().state, IntegrationState::ConfirmedLocalRef);
        assert!(
            result.record().observed_head.is_none(),
            "cancelled post-write observer must not invent a current ref observation"
        );
        assert_eq!(
            git(&root, &["rev-parse", "refs/heads/integration"]),
            candidate.candidate_commit()
        );
        assert!(result.io().io_complete());
        assert!(result.io().interrupted());
        if late {
            assert!(Instant::now() >= deadline);
            assert!(result.io().elapsed_ms() >= 5000);
        }
    }
    #[test]
    fn joined_update_ref_success_survives_subsequent_cancel() {
        completed_cas_remains_historical_success(false);
    }
    #[test]
    fn joined_update_ref_success_survives_late_receipt_handling() {
        completed_cas_remains_historical_success(true);
    }
}
