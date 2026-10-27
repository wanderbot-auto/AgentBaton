//! Deterministic artifact validation. Trusted host code resolves grants and roots;
//! this library cannot authenticate caller-supplied approval labels. No repository
//! validation commands or artifact contents are executed.
use crate::{open_root_without_symlinks, WorkspaceError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum ValidationError {
    Workspace(WorkspaceError),
    InvalidContract,
    StaleEvidence,
    ValidationFailed,
    ApprovalMismatch,
}
impl From<WorkspaceError> for ValidationError {
    fn from(error: WorkspaceError) -> Self {
        Self::Workspace(error)
    }
}
impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "validation rejected: {:?}", self)
    }
}
impl std::error::Error for ValidationError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum VersionBinding {
    Git {
        target_ref: String,
        expected_target_head: String,
        source_candidate: String,
        integration_candidate: String,
        tree: String,
    },
    Data {
        dataset_revision: String,
    },
    WorkspaceArtifacts {
        workspace_id: String,
        base_revision: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationSubject {
    pub task_id: String,
    pub attempt_id: String,
    pub spec_revision: u64,
    pub input_revision: String,
    pub version: VersionBinding,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRequirement {
    pub artifact_id: String,
    pub relative_path: PathBuf,
    pub max_bytes: u64,
    pub expected_sha256: Option<String>,
}
/// Immutable approved root mapping from the host registry, never a renderer path.
#[derive(Clone)]
pub struct BoundArtifact {
    root: PathBuf,
    identity: (u64, u64),
    requirement: ArtifactRequirement,
}
impl BoundArtifact {
    pub fn from_approved_root(
        root: &Path,
        requirement: ArtifactRequirement,
    ) -> Result<Self, ValidationError> {
        if root.to_str().is_none()
            || !root.is_absolute()
            || fs::canonicalize(root).map_err(WorkspaceError::Io)? != root
            || !valid_id(&requirement.artifact_id)
            || requirement.max_bytes == 0
            || requirement.max_bytes > 67_108_864
            || !safe_relative(&requirement.relative_path)
            || requirement
                .expected_sha256
                .as_deref()
                .is_some_and(|hash| !valid_hash(hash))
        {
            return Err(ValidationError::InvalidContract);
        }
        let metadata = fs::metadata(root).map_err(WorkspaceError::Io)?;
        if !metadata.is_dir() {
            return Err(ValidationError::InvalidContract);
        }
        Ok(Self {
            root: root.into(),
            identity: (metadata.dev(), metadata.ino()),
            requirement,
        })
    }
    pub fn contract_view(&self) -> ArtifactContractView {
        ArtifactContractView {
            canonical_root: self.root.clone(),
            root_device: self.identity.0,
            root_inode: self.identity.1,
            requirement: self.requirement.clone(),
        }
    }
    /// Reconstruct only from the trusted persisted owner-approved policy record;
    /// a deserialized contract view alone is not authority to read any path.
    pub fn from_verified_contract(view: &ArtifactContractView) -> Result<Self, ValidationError> {
        let binding = Self::from_approved_root(&view.canonical_root, view.requirement.clone())?;
        if binding.identity != (view.root_device, view.root_inode) {
            return Err(ValidationError::StaleEvidence);
        }
        Ok(binding)
    }
    fn read(&self) -> Result<Vec<u8>, WorkspaceError> {
        read_pinned_artifact(
            &self.root,
            &self.requirement.relative_path,
            self.requirement.max_bytes,
            self.identity,
        )
    }
}
fn read_pinned_artifact(
    root: &Path,
    relative: &Path,
    max_bytes: u64,
    expected_identity: (u64, u64),
) -> Result<Vec<u8>, WorkspaceError> {
    let directory = open_root_without_symlinks(root)?;
    read_opened_artifact(directory, root, relative, max_bytes, expected_identity)
}
fn read_opened_artifact(
    mut directory: std::fs::File,
    root: &Path,
    relative: &Path,
    max_bytes: u64,
    expected_identity: (u64, u64),
) -> Result<Vec<u8>, WorkspaceError> {
    use std::{
        ffi::CString,
        fs::File,
        io::Read,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::ffi::OsStrExt,
        },
    };
    let opened = directory.metadata()?;
    if (opened.dev(), opened.ino()) != expected_identity {
        return Err(WorkspaceError::InvalidPath);
    }
    let parts: Vec<_> = relative.components().collect();
    for (index, part) in parts.iter().enumerate() {
        let std::path::Component::Normal(name) = part else {
            return Err(WorkspaceError::InvalidPath);
        };
        let name = CString::new(name.as_bytes()).map_err(|_| WorkspaceError::InvalidPath)?;
        let last = index + 1 == parts.len();
        let flags = libc::O_RDONLY
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | libc::O_NONBLOCK
            | if last { 0 } else { libc::O_DIRECTORY };
        // SAFETY: both the pinned directory descriptor and NUL-terminated name live through openat.
        let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: openat transferred one newly owned descriptor.
        let file = unsafe { File::from_raw_fd(fd) };
        if !last {
            directory = file;
            continue;
        }
        let before = file.metadata()?;
        if !before.is_file() || before.nlink() != 1 {
            return Err(WorkspaceError::NotRegularFile);
        }
        if before.len() > max_bytes {
            return Err(WorkspaceError::TooLarge);
        }
        let mut bytes = Vec::new();
        let mut reader = file.take(max_bytes.checked_add(1).ok_or(WorkspaceError::TooLarge)?);
        reader.read_to_end(&mut bytes)?;
        let after = reader.get_ref().metadata()?;
        if bytes.len() as u64 > max_bytes {
            return Err(WorkspaceError::TooLarge);
        }
        if after.nlink() != 1
            || before.len() != after.len()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
        {
            return Err(WorkspaceError::InvalidPath);
        }
        let current_root = fs::metadata(root)?;
        if (current_root.dev(), current_root.ino()) != expected_identity {
            return Err(WorkspaceError::InvalidPath);
        }
        return Ok(bytes);
    }
    Err(WorkspaceError::InvalidPath)
}

/// Serializable proposal/evidence only; this is not an authorization token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactContractView {
    pub canonical_root: PathBuf,
    pub root_device: u64,
    pub root_inode: u64,
    pub requirement: ArtifactRequirement,
}
/// Host-native owner proposal/persisted policy DTO. Never trust an Agent-supplied
/// view as approval: derive scope from authenticated native owner/registry state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationPolicyContractView {
    pub validator_id: String,
    pub subject: ValidationSubject,
    pub inputs: Vec<ArtifactContractView>,
    pub outputs: Vec<ArtifactContractView>,
    pub analysis: Option<AnalysisReproducibility>,
}
impl ValidationPolicyContractView {
    /// Deterministic proposal binding, not a signature or authority proof.
    pub fn digest(&self) -> String {
        policy_contract_digest(self)
    }
}
fn contract_view(
    subject: &ValidationSubject,
    inputs: &[BoundArtifact],
    outputs: &[BoundArtifact],
    analysis: Option<AnalysisReproducibility>,
) -> ValidationPolicyContractView {
    let mut inputs: Vec<_> = inputs.iter().map(BoundArtifact::contract_view).collect();
    let mut outputs: Vec<_> = outputs.iter().map(BoundArtifact::contract_view).collect();
    inputs.sort_by(|left, right| {
        left.requirement
            .artifact_id
            .cmp(&right.requirement.artifact_id)
    });
    outputs.sort_by(|left, right| {
        left.requirement
            .artifact_id
            .cmp(&right.requirement.artifact_id)
    });
    ValidationPolicyContractView {
        validator_id: "builtin.artifact-integrity.v1".into(),
        subject: subject.clone(),
        inputs,
        outputs,
        analysis,
    }
}

/// Construct only after the daemon validates the persisted policy/actor grant.
/// This typed token prevents accidental mixing; it is not itself authentication.
#[derive(Clone)]
pub struct ApprovedRule {
    approval_id: String,
    policy_revision: String,
    subject: ValidationSubject,
    required_outputs: Vec<String>,
    contract: ValidationPolicyContractView,
}
impl ApprovedRule {
    pub fn from_verified_policy(
        approval_id: String,
        policy_revision: String,
        subject: ValidationSubject,
        inputs: &[BoundArtifact],
        outputs: &[BoundArtifact],
        analysis: Option<AnalysisReproducibility>,
    ) -> Result<Self, ValidationError> {
        validate_subject(&subject)?;
        let contract = contract_view(&subject, inputs, outputs, analysis);
        let required_outputs: Vec<_> = contract
            .outputs
            .iter()
            .map(|output| output.requirement.artifact_id.clone())
            .collect();
        if !valid_id(&approval_id)
            || !valid_id(&policy_revision)
            || required_outputs.is_empty()
            || required_outputs.len() > 256
            || inputs.len() > 256
            || required_outputs.windows(2).any(|pair| pair[0] == pair[1])
            || contract
                .inputs
                .windows(2)
                .any(|pair| pair[0].requirement.artifact_id == pair[1].requirement.artifact_id)
            || inputs
                .iter()
                .any(|input| input.requirement.expected_sha256.is_none())
        {
            return Err(ValidationError::InvalidContract);
        }
        if let Some(analysis) = &contract.analysis {
            validate_analysis(analysis, &required_outputs, inputs.len())?;
            let script = contract
                .outputs
                .iter()
                .find(|output| output.requirement.artifact_id == analysis.script_artifact_id)
                .unwrap();
            let environment = contract
                .outputs
                .iter()
                .find(|output| output.requirement.artifact_id == analysis.environment_artifact_id)
                .unwrap();
            if script.canonical_root == environment.canonical_root
                && script.requirement.relative_path == environment.requirement.relative_path
            {
                return Err(ValidationError::InvalidContract);
            }
        }
        Ok(Self {
            approval_id,
            policy_revision,
            subject,
            required_outputs,
            contract,
        })
    }
    pub fn contract_view(&self) -> ValidationPolicyContractView {
        self.contract.clone()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AnalysisScope {
    Full,
    Sample { rows: u64, limitation: String },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisReproducibility {
    pub script_artifact_id: String,
    pub environment_artifact_id: String,
    pub units: BTreeMap<String, String>,
    pub timezone: String,
    pub seed: Option<u64>,
    pub scope: AnalysisScope,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    Passed,
    Missing,
    UnsafeOrUnreadable,
    TooLarge,
    DigestMismatch,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactEvidence {
    pub artifact_id: String,
    pub relative_path: PathBuf,
    pub root_device: u64,
    pub root_inode: u64,
    pub expected_sha256: Option<String>,
    pub observed_sha256: Option<String>,
    pub size: Option<u64>,
    pub state: CheckState,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationView {
    pub schema_version: u32,
    pub digest: String,
    pub validator_id: String,
    pub subject: ValidationSubject,
    pub approval_id: String,
    pub policy_revision: String,
    pub policy_contract: ValidationPolicyContractView,
    pub inputs: Vec<ArtifactEvidence>,
    pub outputs: Vec<ArtifactEvidence>,
    pub analysis: Option<AnalysisReproducibility>,
    pub passed: bool,
}
pub struct ValidationEvidence {
    view: ValidationView,
    inputs: Vec<BoundArtifact>,
    outputs: Vec<BoundArtifact>,
}
impl ValidationEvidence {
    pub fn view(&self) -> ValidationView {
        self.view.clone()
    }
    /// Re-read fixed input/output subjects. A prior pass is not reusable after drift.
    pub fn verify_current(&self) -> Result<(), ValidationError> {
        let inputs: Vec<_> = self.inputs.iter().map(check_artifact).collect();
        let outputs: Vec<_> = self.outputs.iter().map(check_artifact).collect();
        if inputs != self.view.inputs || outputs != self.view.outputs {
            return Err(ValidationError::StaleEvidence);
        }
        Ok(())
    }
    pub fn accept(&self, approval: &ApprovedRule) -> Result<Acceptance, ValidationError> {
        if self.view.policy_contract != approval.contract
            || self.view.subject != approval.subject
            || self.view.approval_id != approval.approval_id
            || self.view.policy_revision != approval.policy_revision
            || self
                .view
                .outputs
                .iter()
                .map(|output| output.artifact_id.clone())
                .collect::<Vec<_>>()
                != approval.required_outputs
        {
            return Err(ValidationError::ApprovalMismatch);
        }
        self.verify_current()?;
        if !self.view.passed {
            return Err(ValidationError::ValidationFailed);
        }
        Ok(Acceptance {
            validation_digest: self.view.digest.clone(),
            approval_id: approval.approval_id.clone(),
            policy_revision: approval.policy_revision.clone(),
            subject: approval.subject.clone(),
        })
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Acceptance {
    validation_digest: String,
    approval_id: String,
    policy_revision: String,
    subject: ValidationSubject,
}
impl Acceptance {
    pub fn validation_digest(&self) -> &str {
        &self.validation_digest
    }
    pub fn subject(&self) -> &ValidationSubject {
        &self.subject
    }
    pub(crate) fn matches(&self, evidence: &ValidationEvidence) -> bool {
        self.validation_digest == evidence.view.digest
            && self.subject == evidence.view.subject
            && self.approval_id == evidence.view.approval_id
            && self.policy_revision == evidence.view.policy_revision
            && evidence.view.passed
    }
}
/// Built-in file integrity validation only. Full code/numerical correctness needs
/// independently authorized deterministic validators; Agent self-report is absent.
pub fn validate_artifacts(
    approval: &ApprovedRule,
    mut inputs: Vec<BoundArtifact>,
    mut outputs: Vec<BoundArtifact>,
    analysis: Option<AnalysisReproducibility>,
) -> Result<ValidationEvidence, ValidationError> {
    inputs.sort_by(|left, right| {
        left.requirement
            .artifact_id
            .cmp(&right.requirement.artifact_id)
    });
    outputs.sort_by(|left, right| {
        left.requirement
            .artifact_id
            .cmp(&right.requirement.artifact_id)
    });
    if contract_view(&approval.subject, &inputs, &outputs, analysis.clone()) != approval.contract {
        return Err(ValidationError::ApprovalMismatch);
    }
    if inputs.len() > 256
        || outputs.len() > 256
        || inputs
            .windows(2)
            .any(|pair| pair[0].requirement.artifact_id == pair[1].requirement.artifact_id)
        || inputs
            .iter()
            .any(|input| input.requirement.expected_sha256.is_none())
        || outputs
            .iter()
            .map(|output| output.requirement.artifact_id.clone())
            .collect::<Vec<_>>()
            != approval.required_outputs
    {
        return Err(ValidationError::InvalidContract);
    }
    if let Some(analysis) = &analysis {
        validate_analysis(analysis, &approval.required_outputs, inputs.len())?;
    }
    let input_evidence: Vec<_> = inputs.iter().map(check_artifact).collect();
    let output_evidence: Vec<_> = outputs.iter().map(check_artifact).collect();
    let passed = input_evidence
        .iter()
        .chain(&output_evidence)
        .all(|item| item.state == CheckState::Passed);
    let mut view = ValidationView {
        schema_version: 1,
        digest: String::new(),
        validator_id: "builtin.artifact-integrity.v1".into(),
        subject: approval.subject.clone(),
        approval_id: approval.approval_id.clone(),
        policy_revision: approval.policy_revision.clone(),
        policy_contract: approval.contract.clone(),
        inputs: input_evidence,
        outputs: output_evidence,
        analysis,
        passed,
    };
    view.digest = evidence_digest(&view);
    Ok(ValidationEvidence {
        view,
        inputs,
        outputs,
    })
}
fn check_artifact(artifact: &BoundArtifact) -> ArtifactEvidence {
    let mut evidence = ArtifactEvidence {
        artifact_id: artifact.requirement.artifact_id.clone(),
        relative_path: artifact.requirement.relative_path.clone(),
        root_device: artifact.identity.0,
        root_inode: artifact.identity.1,
        expected_sha256: artifact.requirement.expected_sha256.clone(),
        observed_sha256: None,
        size: None,
        state: CheckState::UnsafeOrUnreadable,
    };
    match artifact.read() {
        Ok(bytes) => {
            let hash = sha256(&bytes);
            evidence.size = Some(bytes.len() as u64);
            evidence.state = if artifact
                .requirement
                .expected_sha256
                .as_ref()
                .is_some_and(|expected| *expected != hash)
            {
                CheckState::DigestMismatch
            } else {
                CheckState::Passed
            };
            evidence.observed_sha256 = Some(hash);
        }
        Err(WorkspaceError::TooLarge) => evidence.state = CheckState::TooLarge,
        Err(WorkspaceError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            evidence.state = CheckState::Missing
        }
        Err(_) => {}
    }
    evidence
}
fn validate_subject(subject: &ValidationSubject) -> Result<(), ValidationError> {
    if !valid_id(&subject.task_id)
        || !valid_id(&subject.attempt_id)
        || subject.spec_revision == 0
        || !valid_id(&subject.input_revision)
    {
        return Err(ValidationError::InvalidContract);
    }
    match &subject.version {
        VersionBinding::Git {
            target_ref,
            expected_target_head,
            source_candidate,
            integration_candidate,
            tree,
        } => {
            if !target_ref.starts_with("refs/heads/")
                || [
                    expected_target_head,
                    source_candidate,
                    integration_candidate,
                    tree,
                ]
                .iter()
                .any(|oid| !crate::is_oid(oid))
            {
                return Err(ValidationError::InvalidContract);
            }
        }
        VersionBinding::Data { dataset_revision } => {
            if !valid_id(dataset_revision) {
                return Err(ValidationError::InvalidContract);
            }
        }
        VersionBinding::WorkspaceArtifacts {
            workspace_id,
            base_revision,
        } => {
            if !valid_id(workspace_id) || !valid_id(base_revision) {
                return Err(ValidationError::InvalidContract);
            }
        }
    }
    Ok(())
}
fn validate_analysis(
    analysis: &AnalysisReproducibility,
    required: &[String],
    input_count: usize,
) -> Result<(), ValidationError> {
    if input_count == 0
        || !required.contains(&analysis.script_artifact_id)
        || !required.contains(&analysis.environment_artifact_id)
        || analysis.script_artifact_id == analysis.environment_artifact_id
        || analysis.units.is_empty()
        || analysis.units.len() > 64
        || analysis
            .units
            .iter()
            .any(|(name, unit)| !valid_id(name) || unit.is_empty() || unit.len() > 128)
        || !valid_id(&analysis.timezone)
    {
        return Err(ValidationError::InvalidContract);
    }
    if let AnalysisScope::Sample { rows, limitation } = &analysis.scope {
        if *rows == 0 || limitation.is_empty() || limitation.len() > 4096 {
            return Err(ValidationError::InvalidContract);
        }
    }
    Ok(())
}
fn safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path.to_str().is_some()
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}
pub(crate) fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-/".contains(&byte))
}
pub(crate) fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn evidence_digest(view: &ValidationView) -> String {
    let mut digest = Sha256::new();
    let mut add = |value: &str| {
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value.as_bytes());
    };
    add("agentbaton.validation.v1");
    add(&view.validator_id);
    add(&view.subject.task_id);
    add(&view.subject.attempt_id);
    add(&view.subject.spec_revision.to_string());
    add(&view.subject.input_revision);
    match &view.subject.version {
        VersionBinding::Git {
            target_ref,
            expected_target_head,
            source_candidate,
            integration_candidate,
            tree,
        } => {
            add("git");
            for value in [
                target_ref,
                expected_target_head,
                source_candidate,
                integration_candidate,
                tree,
            ] {
                add(value);
            }
        }
        VersionBinding::Data { dataset_revision } => {
            add("data");
            add(dataset_revision);
        }
        VersionBinding::WorkspaceArtifacts {
            workspace_id,
            base_revision,
        } => {
            add("workspace_artifacts");
            add(workspace_id);
            add(base_revision);
        }
    }
    add(&view.approval_id);
    add(&view.policy_revision);
    add(&policy_contract_digest(&view.policy_contract));
    add(if view.passed { "passed" } else { "failed" });
    for (kind, items) in [("inputs", &view.inputs), ("outputs", &view.outputs)] {
        add(kind);
        add(&items.len().to_string());
        for item in items {
            add(&item.artifact_id);
            add(&item.relative_path.to_string_lossy());
            add(&item.root_device.to_string());
            add(&item.root_inode.to_string());
            add(item.expected_sha256.as_deref().unwrap_or("none"));
            add(item.observed_sha256.as_deref().unwrap_or("none"));
            add(&format!("{:?}", item.state));
            add(&item
                .size
                .map_or_else(|| "none".into(), |size| size.to_string()));
        }
    }
    if let Some(analysis) = &view.analysis {
        add("analysis");
        add(&analysis.script_artifact_id);
        add(&analysis.environment_artifact_id);
        add(&analysis.units.len().to_string());
        for (name, unit) in &analysis.units {
            add(name);
            add(unit);
        }
        add(&analysis.timezone);
        add(&format!("{:?}", analysis.seed));
        match &analysis.scope {
            AnalysisScope::Full => add("full"),
            AnalysisScope::Sample { rows, limitation } => {
                add("sample");
                add(&rows.to_string());
                add(limitation);
            }
        }
    } else {
        add("no_analysis");
    }
    format!("{:x}", digest.finalize())
}

fn policy_contract_digest(contract: &ValidationPolicyContractView) -> String {
    let mut digest = Sha256::new();
    let mut add = |value: &str| {
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value.as_bytes());
    };
    add("agentbaton.validation-policy-contract.v1");
    add(&contract.validator_id);
    for (kind, items) in [("inputs", &contract.inputs), ("outputs", &contract.outputs)] {
        add(kind);
        add(&items.len().to_string());
        for item in items {
            add(&item.canonical_root.to_string_lossy());
            add(&item.root_device.to_string());
            add(&item.root_inode.to_string());
            add(&item.requirement.artifact_id);
            add(&item.requirement.relative_path.to_string_lossy());
            add(&item.requirement.max_bytes.to_string());
            add(item
                .requirement
                .expected_sha256
                .as_deref()
                .unwrap_or("none"));
        }
    }
    add(&contract.subject.task_id);
    add(&contract.subject.attempt_id);
    add(&contract.subject.spec_revision.to_string());
    add(&contract.subject.input_revision);
    match &contract.subject.version {
        VersionBinding::Git {
            target_ref,
            expected_target_head,
            source_candidate,
            integration_candidate,
            tree,
        } => {
            add("git");
            for value in [
                target_ref,
                expected_target_head,
                source_candidate,
                integration_candidate,
                tree,
            ] {
                add(value);
            }
        }
        VersionBinding::Data { dataset_revision } => {
            add("data");
            add(dataset_revision);
        }
        VersionBinding::WorkspaceArtifacts {
            workspace_id,
            base_revision,
        } => {
            add("workspace_artifacts");
            add(workspace_id);
            add(base_revision);
        }
    }
    if let Some(analysis) = &contract.analysis {
        add("analysis");
        add(&analysis.script_artifact_id);
        add(&analysis.environment_artifact_id);
        add(&analysis.units.len().to_string());
        for (name, unit) in &analysis.units {
            add(name);
            add(unit);
        }
        add(&analysis.timezone);
        add(&format!("{:?}", analysis.seed));
        match &analysis.scope {
            AnalysisScope::Full => add("full"),
            AnalysisScope::Sample { rows, limitation } => {
                add("sample");
                add(&rows.to_string());
                add(limitation);
            }
        }
    } else {
        add("no_analysis");
    }
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
mod root_binding_tests {
    use super::*;
    #[test]
    fn opened_root_descriptor_is_checked_instead_of_trusting_an_earlier_path_stat() {
        let owner = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let approved = owner.path().canonicalize().unwrap();
        let replacement = other.path().canonicalize().unwrap();
        fs::write(approved.join("artifact"), b"authorized").unwrap();
        fs::write(replacement.join("artifact"), b"unauthorized").unwrap();
        let metadata = fs::metadata(&approved).unwrap();
        let expected = (metadata.dev(), metadata.ino());
        // Model the exact stat/open race: a path stat yielded the approved
        // identity, but the opener returned a different held directory descriptor.
        let wrong_descriptor = open_root_without_symlinks(&replacement).unwrap();
        assert!(matches!(
            read_opened_artifact(
                wrong_descriptor,
                &approved,
                Path::new("artifact"),
                1024,
                expected
            ),
            Err(WorkspaceError::InvalidPath)
        ));
        let approved_descriptor = open_root_without_symlinks(&approved).unwrap();
        assert_eq!(
            read_opened_artifact(
                approved_descriptor,
                &approved,
                Path::new("artifact"),
                1024,
                expected
            )
            .unwrap(),
            b"authorized"
        );
    }
}
