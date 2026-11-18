//! Admission for a staged managed installation. The caller resolves current
//! records, live approval and verifier provenance from trusted stores, and
//! identifies the task and verification host authoritatively.
use crate::connections::{
    BindingError, ConfigurationSource, ExecutionBinding, ExecutionMode, IdentityReference,
    InstallationRecord, InstallationSource, NamedConnection, ProtocolRoute, ProviderKind,
    RuntimeEnvironment, SupportManifest,
};
use crate::installation::{verify_executable, BinaryDigest, InstallationError};
use std::{fmt, fs, path::Path};

#[derive(Debug, Clone, Copy)]
pub enum UpdateTask<'a> {
    New {
        task_id: &'a str,
        attempt_id: &'a str,
        input_revision: &'a str,
        mode: ExecutionMode,
    },
    AlreadyExists,
    OutcomeUnknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateScope<'a> {
    pub connection_id: &'a str,
    pub current_connection_revision: u64,
    pub provider: ProviderKind,
    pub route: ProtocolRoute,
    pub execution_mode: ExecutionMode,
    pub host_id: &'a str,
    pub identity_ref: &'a str,
    pub environment_id: &'a str,
    pub configuration: &'a ConfigurationSource,
    pub config_snapshot_sha256: &'a BinaryDigest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateInstallationPin<'a> {
    pub installation_id: &'a str,
    pub installation_approval_ref: &'a str,
    pub executable: &'a Path,
    pub binary_sha256: &'a BinaryDigest,
    pub version: &'a str,
    pub support_manifest_id: &'a str,
}

impl UpdateInstallationPin<'_> {
    fn matches(&self, installation: &InstallationRecord, support: &SupportManifest) -> bool {
        self.installation_id == installation.id
            && self.installation_approval_ref == installation.approval_reference
            && self.executable == installation.canonical_executable
            && *self.binary_sha256 == installation.binary_sha256
            && self.version == support.provider_version
            && self.support_manifest_id == support.id
    }
}

/// Exact transition resolved from the caller's approved update policy. The
/// resolver supplies the managed root and requirements independently of the
/// staged installation, and withholds this value if approval is unavailable.
#[derive(Debug)]
pub struct ApprovedManagedUpdate<'a> {
    pub approval_ref: &'a str,
    pub scope: UpdateScope<'a>,
    pub managed_root: &'a Path,
    pub current: UpdateInstallationPin<'a>,
    pub candidate: UpdateInstallationPin<'a>,
    pub required_capabilities: &'a [&'a str],
    pub valid_until_unix_ms: u64,
    pub max_evidence_age_ms: u64,
}

/// A compatibility observation resolved by the caller from its verifier. Its
/// scope and program pin identify what was actually tested on the bound host.
#[derive(Debug)]
pub struct ManagedUpdateCompatibility<'a> {
    pub scope: UpdateScope<'a>,
    pub tested_installation: UpdateInstallationPin<'a>,
    pub support: &'a SupportManifest,
}

#[derive(Debug)]
pub struct ManagedUpdateRequest<'a> {
    pub task: UpdateTask<'a>,
    pub connection: &'a NamedConnection,
    pub current: &'a InstallationRecord,
    pub current_support: &'a SupportManifest,
    pub staged: &'a InstallationRecord,
    pub environment: &'a RuntimeEnvironment,
    pub identity: &'a IdentityReference,
    pub verification_host_id: &'a str,
    pub now_unix_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedUpdateError {
    ExistingTask,
    OutcomeUnknown,
    PolicyUnavailable,
    InvalidPolicy,
    PolicyMismatch,
    ExternalInstallation,
    InstallationsNotIndependent,
    CompatibilityMismatch,
    StaleEvidence,
    Binding(BindingError),
    Verification(InstallationError),
}

impl fmt::Display for ManagedUpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "managed update admission: {self:?}")
    }
}

impl std::error::Error for ManagedUpdateError {}

/// Returns an unregistered connection revision and binding for a new task.
/// Registration must compare against the same current revision and approval
/// while keeping prior revisions and files addressable. Dispatch must recheck
/// its pinned digest at launch. This operation reads executable bytes locally;
/// the caller must run it on the verified host.
pub fn admit_managed_update(
    request: &ManagedUpdateRequest<'_>,
    approved: Option<&ApprovedManagedUpdate<'_>>,
    compatibility: &ManagedUpdateCompatibility<'_>,
) -> Result<(NamedConnection, ExecutionBinding), ManagedUpdateError> {
    use ManagedUpdateError as Error;
    let UpdateTask::New {
        task_id,
        attempt_id,
        input_revision,
        mode,
    } = request.task
    else {
        return Err(match request.task {
            UpdateTask::OutcomeUnknown => Error::OutcomeUnknown,
            _ => Error::ExistingTask,
        });
    };
    let approved = approved.ok_or(Error::PolicyUnavailable)?;
    let now = request.now_unix_ms;
    if approved.approval_ref.trim().is_empty()
        || approved.approval_ref.len() > 512
        || approved.approval_ref.chars().any(char::is_control)
        || now == 0
        || approved.valid_until_unix_ms <= now
        || approved.max_evidence_age_ms == 0
        || approved.required_capabilities.is_empty()
        || approved.required_capabilities.len() > 128
    {
        return Err(Error::InvalidPolicy);
    }

    let current = request.current;
    let staged = request.staged;
    if current.source != InstallationSource::BatonManaged
        || staged.source != InstallationSource::BatonManaged
    {
        return Err(Error::ExternalInstallation);
    }

    let connection = request.connection;
    let environment = request.environment;
    let scope = UpdateScope {
        connection_id: &connection.id,
        current_connection_revision: connection.revision,
        provider: connection.provider,
        route: connection.protocol_route,
        execution_mode: mode,
        host_id: &connection.host_id,
        identity_ref: &connection.identity_ref,
        environment_id: &environment.id,
        configuration: &environment.configuration,
        config_snapshot_sha256: &environment.config_snapshot_sha256,
    };
    let support = compatibility.support;
    if scope != approved.scope
        || request.verification_host_id != scope.host_id
        || !approved.current.matches(current, request.current_support)
        || !approved.candidate.matches(staged, support)
    {
        return Err(Error::PolicyMismatch);
    }
    if compatibility.scope != scope || compatibility.tested_installation != approved.candidate {
        return Err(Error::CompatibilityMismatch);
    }
    if staged.id == current.id
        || staged.canonical_executable == current.canonical_executable
        || staged.binary_sha256 == current.binary_sha256
        || support.id == request.current_support.id
        || support.provider_version == request.current_support.provider_version
    {
        return Err(Error::InstallationsNotIndependent);
    }

    connection
        .bind_execution(
            task_id,
            attempt_id,
            input_revision,
            mode,
            current,
            environment,
            request.identity,
            request.current_support,
            &[],
        )
        .map_err(Error::Binding)?;

    let mut next = connection.clone();
    next.revision = next.revision.checked_add(1).ok_or(Error::InvalidPolicy)?;
    next.installation_id.clone_from(&staged.id);
    next.support_manifest_ref.clone_from(&support.id);
    let binding = next
        .bind_execution(
            task_id,
            attempt_id,
            input_revision,
            mode,
            staged,
            environment,
            request.identity,
            support,
            approved.required_capabilities,
        )
        .map_err(Error::Binding)?;

    let probe_time = staged.version_probe.observed_at_unix_ms;
    if probe_time > now
        || now - probe_time > approved.max_evidence_age_ms
        || support
            .capabilities
            .iter()
            .any(|item| item.observed_at_unix_ms < probe_time || item.observed_at_unix_ms > now)
    {
        return Err(Error::StaleEvidence);
    }

    verify_managed_files(approved.managed_root, current, staged)?;
    Ok((next, binding))
}

fn verify_managed_files(
    root: &Path,
    current: &InstallationRecord,
    staged: &InstallationRecord,
) -> Result<(), ManagedUpdateError> {
    use ManagedUpdateError as Error;
    let current_path = &current.canonical_executable;
    let staged_path = &staged.canonical_executable;
    if !root.is_absolute()
        || root.parent().is_none()
        || !current_path.starts_with(root)
        || !staged_path.starts_with(root)
        || fs::canonicalize(root).ok().as_deref() != Some(root)
        || !root.is_dir()
    {
        return Err(Error::ExternalInstallation);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let current =
            fs::metadata(current_path).map_err(|_| Error::Verification(InstallationError::Io))?;
        let staged =
            fs::metadata(staged_path).map_err(|_| Error::Verification(InstallationError::Io))?;
        if (current.dev(), current.ino()) == (staged.dev(), staged.ino())
            || current.nlink() != 1
            || staged.nlink() != 1
        {
            return Err(Error::InstallationsNotIndependent);
        }
    }
    verify_executable(current_path, &current.binary_sha256).map_err(Error::Verification)?;
    verify_executable(staged_path, &staged.binary_sha256).map_err(Error::Verification)
}
