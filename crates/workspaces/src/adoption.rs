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
        if !matches!(root.kind, WorkspaceKind::Git { .. })
            || !target_ref.starts_with("refs/heads/")
            || !is_oid(&expected_target_head)
            || !is_oid(&source_candidate)
            || !valid_id(&grant_ref)
            || commit_time_unix > 4_294_967_295
        {
            return Err(AdoptionError::InvalidGrant);
        }
        let current = inspect(&root.canonical_root)?;
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
        let valid = bounded_output(
            git_command(&root.canonical_root).args(["check-ref-format", target_ref.as_str()]),
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
fn bound_git_text(grant: &AuthorizedIntegration, args: &[&str]) -> Result<String, AdoptionError> {
    let output = bounded_output(bound_git_command(grant).args(args))?;
    if !output.status.success() {
        return Err(AdoptionError::Conflict);
    }
    String::from_utf8(output.stdout)
        .map(|text| text.trim().to_owned())
        .map_err(|_| AdoptionError::Conflict)
}
fn create_bound_isolated(
    grant: &AuthorizedIntegration,
    managed_root: &Path,
    name: &str,
    commit: &str,
) -> Result<WorkspaceIdentity, AdoptionError> {
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
    verify_repository(grant)?;
    let output = bounded_output(bound_git_command(grant).args([
        std::ffi::OsStr::new("worktree"),
        std::ffi::OsStr::new("add"),
        std::ffi::OsStr::new("--detach"),
        std::ffi::OsStr::new("--"),
        destination.as_os_str(),
        std::ffi::OsStr::new(commit),
    ]))?;
    if !output.status.success() {
        return Err(AdoptionError::Conflict);
    }
    Ok(inspect(&destination)?)
}
fn verify_repository(grant: &AuthorizedIntegration) -> Result<WorkspaceIdentity, AdoptionError> {
    let current = inspect(&grant.root)?;
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
    reject_bound_unsafe_config(grant)?;
    let actual = bound_git_text(
        grant,
        &[
            "rev-parse",
            "--verify",
            &format!("{}^{{commit}}", grant.target_ref),
        ],
    )?;
    if actual != grant.expected_target_head {
        return Err(AdoptionError::StaleHead);
    }
    Ok(current)
}
fn reject_bound_unsafe_config(grant: &AuthorizedIntegration) -> Result<(), AdoptionError> {
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
        let config = bounded_output(bound_git_command(grant).args([
            "config",
            "--no-includes",
            "--get-regexp",
            pattern,
        ]))?;
        if config.status.success() || config.status.code() != Some(1) {
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
) -> Result<(), AdoptionError> {
    let tree = bounded_output(bound_git_command(grant).args(["ls-tree", "-r", "-z", candidate]))?;
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
    verify_repository(&grant)?;
    reject_bound_unsafe_config(&grant)?;
    let source = bound_git_text(
        &grant,
        &[
            "rev-parse",
            "--verify",
            &format!("{}^{{commit}}", grant.source_candidate),
        ],
    )?;
    if source != grant.source_candidate {
        return Err(AdoptionError::SubjectMismatch);
    }
    reject_candidate_gitlinks(&grant, &grant.source_candidate)?;
    let merge = bounded_output(bound_git_command(&grant).args([
        "-c",
        "merge.renormalize=false",
        "merge-tree",
        "--write-tree",
        grant.expected_target_head.as_str(),
        grant.source_candidate.as_str(),
    ]))?;
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
    verify_repository(&grant)?;
    let written = bounded_output(&mut commit)?;
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
    verify_repository(&grant)?;
    let workspace = create_bound_isolated(&grant, managed_root, name, &integration_candidate)?;
    verify_repository(&grant)?;
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
fn checked_out(grant: &AuthorizedIntegration) -> Result<bool, AdoptionError> {
    let worktrees =
        bounded_output(bound_git_command(grant).args(["worktree", "list", "--porcelain", "-z"]))?;
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
    let grant = &candidate.authorization;
    verify_repository(grant)?;
    if candidate.version_binding() != acceptance.subject().version || !acceptance.matches(evidence)
    {
        return Err(AdoptionError::SubjectMismatch);
    }
    evidence.verify_current()?;
    let current = check_adoption(
        &candidate.workspace.canonical_root,
        &candidate.integration_candidate,
    )?;
    if (current.device, current.inode) != (candidate.workspace.device, candidate.workspace.inode) {
        return Err(AdoptionError::SubjectMismatch);
    }
    let WorkspaceKind::Git { common_dir, .. } = &current.kind else {
        return Err(AdoptionError::SubjectMismatch);
    };
    if common_dir != &grant.common_dir {
        return Err(AdoptionError::SubjectMismatch);
    }

    if checked_out(grant)? {
        return Err(AdoptionError::TargetCheckedOut);
    }
    verify_repository(grant)?;
    let result = bounded_output(bound_git_command(grant).args([
        "update-ref",
        "-m",
        "AgentBaton validated local adoption",
        grant.target_ref.as_str(),
        candidate.integration_candidate.as_str(),
        grant.expected_target_head.as_str(),
    ]));
    let observed =
        bound_git_text(grant, &["rev-parse", "--verify", grant.target_ref.as_str()]).ok();
    let state = match result {
        Ok(output)
            if output.status.success()
                && observed.as_deref() == Some(&candidate.integration_candidate) =>
        {
            IntegrationState::ConfirmedLocalRef
        }
        Ok(output)
            if !output.status.success()
                && observed.as_deref() == Some(&grant.expected_target_head) =>
        {
            IntegrationState::Rejected
        }
        _ => IntegrationState::OutcomeUnknown,
    };
    Ok(IntegrationRecord {
        grant_ref: grant.grant_ref.clone(),
        target_ref: grant.target_ref.clone(),
        expected_target_head: grant.expected_target_head.clone(),
        candidate_commit: candidate.integration_candidate.clone(),
        validation_digest: acceptance.validation_digest().into(),
        observed_head: observed,
        state,
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
