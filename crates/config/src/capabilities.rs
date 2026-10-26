//! Static capability inventories and evidence-aware snapshots. Source content is
//! untrusted data; caller-supplied precedence/evidence never grants authority.
//! Only trusted adapter/registry code may provide roots, policies and observations.
pub mod native;

use super::{
    error, io_error, open_dir_at, sha256, validate_relative, ConfigError, ErrorCode, MAX_BYTES,
};
use rustix::fs::{self as rfs, FileType, Mode, OFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, ConfigError>;
const MAX_CAPABILITIES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderRevision {
    pub provider_id: String,
    pub version: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Managed,
    Project,
    Native,
    Plugin,
    Organization,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceDescriptor {
    pub source_id: String,
    pub kind: SourceKind,
    /// Opaque owner-selected locator; no personal absolute paths are exported.
    pub locator_ref: String,
    pub provider: ProviderRevision,
    /// Registry-supplied organization constraint, never derived from descriptions.
    pub mandatory: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    Skill,
    Mcp,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "snake_case", deny_unknown_fields)]
pub enum McpTarget {
    Stdio {
        executable_ref: String,
        arguments_ref: Option<String>,
    },
    Https {
        endpoint_ref: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillMetadata {
    pub content_ref: String,
    #[serde(default)]
    pub dependency_refs: Vec<String>,
    #[serde(default)]
    pub script_refs: Vec<String>,
    #[serde(default)]
    pub compatible_with: Vec<ProviderRevision>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpMetadata {
    pub target: McpTarget,
    #[serde(default)]
    pub credential_refs: Vec<String>,
    pub tools_metadata_ref: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "metadata",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum CapabilityDefinition {
    Skill(SkillMetadata),
    Mcp(McpMetadata),
}
impl CapabilityDefinition {
    fn kind(&self) -> CapabilityKind {
        match self {
            Self::Skill(_) => CapabilityKind::Skill,
            Self::Mcp(_) => CapabilityKind::Mcp,
        }
    }
}
#[derive(Clone, Deserialize)]
struct Inventory {
    schema_version: u32,
    provider: ProviderRevision,
    capabilities: Vec<Entry>,
    #[serde(flatten)]
    unknown: BTreeMap<String, Value>,
}
#[derive(Clone, Deserialize)]
struct Entry {
    id: String,
    name: String,
    enabled: bool,
    definition: CapabilityDefinition,
    #[serde(flatten)]
    unknown: BTreeMap<String, Value>,
}
/// Root is pinned read-only; no automatic discovery or personal/global scanning.
/// The trusted registry must approve both the root and each selected relative file.
pub struct SourceRoot {
    root: File,
    identity: (u64, u64),
}
impl SourceRoot {
    pub fn open_approved_private(path: &Path) -> Result<Self> {
        if !path.is_absolute() || io_error(path.canonicalize())? != path {
            return Err(error(ErrorCode::InvalidPath));
        }
        let root = open_dir_at(rfs::CWD, path)?;
        let meta = io_error(root.metadata())?;
        Ok(Self {
            root,
            identity: (meta.dev(), meta.ino()),
        })
    }
    pub fn read_selected(
        &self,
        descriptor: SourceDescriptor,
        relative: &Path,
    ) -> Result<SourceSnapshot> {
        validate_descriptor(&descriptor)?;
        let raw = self.read(relative)?;
        let inventory: Inventory =
            serde_json::from_slice(&raw).map_err(|_| error(ErrorCode::Conflict))?;
        if inventory.schema_version != 1
            || inventory.provider != descriptor.provider
            || inventory.capabilities.len() > MAX_CAPABILITIES
        {
            return Err(error(ErrorCode::Conflict));
        }
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        for entry in &inventory.capabilities {
            if !valid_ref(&entry.id)
                || !valid_label(&entry.name)
                || !ids.insert(&entry.id)
                || !names.insert((entry.definition.kind(), &entry.name))
            {
                return Err(error(ErrorCode::Conflict));
            }
            validate_definition(&entry.definition)?;
        }
        Ok(SourceSnapshot {
            descriptor,
            relative_path: relative.to_owned(),
            root_identity: self.identity,
            digest: sha256(&raw),
            raw,
            inventory,
        })
    }
    pub fn verify_unchanged(&self, source: &SourceSnapshot) -> Result<bool> {
        if self.identity != source.root_identity {
            return Err(error(ErrorCode::Conflict));
        }
        Ok(sha256(&self.read(&source.relative_path)?) == source.digest)
    }
    fn read(&self, relative: &Path) -> Result<Vec<u8>> {
        validate_relative(relative)?;
        let parts: Vec<_> = relative.components().collect();
        let mut parent = io_error(self.root.try_clone())?;
        for part in &parts[..parts.len() - 1] {
            parent = open_dir_at(&parent, Path::new(part.as_os_str()))?;
        }
        let fd = rfs::openat(
            &parent,
            parts[parts.len() - 1].as_os_str(),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| error(ErrorCode::PermissionDenied))?;
        let stat = io_error(rfs::fstat(&fd))?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
            || stat.st_uid != rustix::process::geteuid().as_raw()
            || stat.st_nlink != 1
            || stat.st_mode & 0o077 != 0
        {
            return Err(error(ErrorCode::PermissionDenied));
        }
        let mut bytes = Vec::new();
        io_error(
            File::from(fd)
                .take((MAX_BYTES + 1) as u64)
                .read_to_end(&mut bytes),
        )?;
        if bytes.len() > MAX_BYTES {
            return Err(error(ErrorCode::ResourceLimit));
        }
        Ok(bytes)
    }
}
/// Private original bytes are retained unchanged; safe previews expose neither
/// arbitrary unknown values nor MCP argument/credential values.
#[derive(Clone)]
pub struct SourceSnapshot {
    descriptor: SourceDescriptor,
    relative_path: PathBuf,
    root_identity: (u64, u64),
    digest: String,
    raw: Vec<u8>,
    inventory: Inventory,
}
impl SourceSnapshot {
    pub fn descriptor(&self) -> &SourceDescriptor {
        &self.descriptor
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn preserves_original_hash(&self) -> bool {
        sha256(&self.raw) == self.digest
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeRule {
    RejectDuplicates,
    ReplaceWholeCapability,
}
/// Policy is provider/version specific and must be selected by a trusted adapter.
/// Order is explicit lowest-to-highest source ID, with no built-in global order.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolutionPolicy {
    pub provider: ProviderRevision,
    pub rule_revision: String,
    pub evidence_ref: String,
    pub lowest_to_highest: Vec<String>,
    pub merge_rule: MergeRule,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Compatibility {
    DeclaredCompatible,
    DeclaredIncompatible,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidatePreview {
    pub source_id: String,
    pub source_kind: SourceKind,
    pub locator_ref: String,
    pub source_digest: String,
    pub capability_id: String,
    pub kind: CapabilityKind,
    pub name: String,
    pub enabled: bool,
    pub winner: bool,
    pub replaced_by_source: Option<String>,
    pub definition_digest: String,
    pub unknown_field_names: Vec<String>,
    pub compatibility: Compatibility,
    pub skill: Option<SkillMetadata>,
    pub mcp: Option<McpMetadata>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotPreview {
    pub schema_version: u32,
    pub digest: String,
    pub provider: ProviderRevision,
    pub rule_revision: String,
    pub resolution_evidence_ref: String,
    pub inferred_resolution: bool,
    pub sources: Vec<SourceSummary>,
    pub candidates: Vec<CandidatePreview>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSummary {
    pub source_id: String,
    pub source_digest: String,
    pub unknown_field_names: Vec<String>,
}
pub struct CapabilitySnapshot {
    policy: ResolutionPolicy,
    sources: Vec<SourceSnapshot>,
    preview: SnapshotPreview,
}
impl CapabilitySnapshot {
    pub fn resolve(policy: ResolutionPolicy, mut sources: Vec<SourceSnapshot>) -> Result<Self> {
        if !valid_provider(&policy.provider)
            || !valid_ref(&policy.rule_revision)
            || !valid_ref(&policy.evidence_ref)
            || sources.is_empty()
            || sources.len() > 64
        {
            return Err(error(ErrorCode::Conflict));
        }
        let source_ids: BTreeSet<_> = sources
            .iter()
            .map(|source| source.descriptor.source_id.as_str())
            .collect();
        let precedence: BTreeSet<_> = policy
            .lowest_to_highest
            .iter()
            .map(String::as_str)
            .collect();
        if source_ids.len() != sources.len()
            || precedence.len() != policy.lowest_to_highest.len()
            || source_ids != precedence
            || sources
                .iter()
                .any(|source| source.descriptor.provider != policy.provider)
        {
            return Err(error(ErrorCode::Conflict));
        }
        sources.sort_by_key(|source| {
            policy
                .lowest_to_highest
                .iter()
                .position(|id| id == &source.descriptor.source_id)
                .unwrap()
        });
        let mut candidates: Vec<CandidatePreview> = Vec::new();
        let mut winners: BTreeMap<(CapabilityKind, String), usize> = BTreeMap::new();
        for source in &sources {
            for entry in &source.inventory.capabilities {
                let key = (entry.definition.kind(), entry.name.clone());
                if let Some(previous) = winners.get(&key) {
                    let previous = &candidates[*previous];
                    let mandatory = sources
                        .iter()
                        .find(|s| s.descriptor.source_id == previous.source_id)
                        .is_some_and(|s| s.descriptor.mandatory);
                    if mandatory || matches!(policy.merge_rule, MergeRule::RejectDuplicates) {
                        return Err(error(ErrorCode::Conflict));
                    }
                }
                let definition_digest = sha256(&io_error(serde_json::to_vec(&entry.definition))?);
                let mut candidate = CandidatePreview {
                    source_id: source.descriptor.source_id.clone(),
                    source_kind: source.descriptor.kind,
                    locator_ref: source.descriptor.locator_ref.clone(),
                    source_digest: source.digest.clone(),
                    capability_id: entry.id.clone(),
                    kind: entry.definition.kind(),
                    name: entry.name.clone(),
                    enabled: entry.enabled,
                    winner: true,
                    replaced_by_source: None,
                    definition_digest,
                    unknown_field_names: entry.unknown.keys().cloned().collect(),
                    compatibility: Compatibility::Unknown,
                    skill: None,
                    mcp: None,
                };
                match &entry.definition {
                    CapabilityDefinition::Skill(metadata) => {
                        candidate.compatibility = if metadata.compatible_with.is_empty() {
                            Compatibility::Unknown
                        } else if metadata.compatible_with.contains(&policy.provider) {
                            Compatibility::DeclaredCompatible
                        } else {
                            Compatibility::DeclaredIncompatible
                        };
                        candidate.skill = Some(metadata.clone());
                    }
                    CapabilityDefinition::Mcp(metadata) => candidate.mcp = Some(metadata.clone()),
                }
                if let Some(previous) = winners.insert(key, candidates.len()) {
                    candidates[previous].winner = false;
                    candidates[previous].replaced_by_source =
                        Some(source.descriptor.source_id.clone());
                }
                candidates.push(candidate);
            }
        }
        let summaries: Vec<_> = sources
            .iter()
            .map(|source| SourceSummary {
                source_id: source.descriptor.source_id.clone(),
                source_digest: source.digest.clone(),
                unknown_field_names: source.inventory.unknown.keys().cloned().collect(),
            })
            .collect();
        let digest = sha256(&io_error(serde_json::to_vec(
            &json!({"schema_version":1,"policy":policy,"sources":summaries,"descriptors":sources.iter().map(|source| &source.descriptor).collect::<Vec<_>>()}),
        ))?);
        let preview = SnapshotPreview {
            schema_version: 1,
            digest,
            provider: policy.provider.clone(),
            rule_revision: policy.rule_revision.clone(),
            resolution_evidence_ref: policy.evidence_ref.clone(),
            inferred_resolution: true,
            sources: summaries,
            candidates,
        };
        Ok(Self {
            policy,
            sources,
            preview,
        })
    }
    pub fn digest(&self) -> &str {
        &self.preview.digest
    }
    pub fn preview(&self) -> SnapshotPreview {
        self.preview.clone()
    }
    pub fn sources(&self) -> &[SourceSnapshot] {
        &self.sources
    }
    pub fn assess(
        &self,
        source_id: &str,
        capability_id: &str,
        binding: &SessionBinding,
        observations: &[Observation],
        now_unix: u64,
    ) -> Result<EvidenceAssessment> {
        if !valid_ref(&binding.session_id) {
            return Err(error(ErrorCode::Conflict));
        }
        let candidate = self
            .preview
            .candidates
            .iter()
            .find(|candidate| {
                candidate.source_id == source_id && candidate.capability_id == capability_id
            })
            .ok_or_else(|| error(ErrorCode::Conflict))?;
        let mut assessment = EvidenceAssessment {
            configured: true,
            enabled: candidate.enabled && candidate.winner,
            diagnostic: EvidenceState::NotObserved,
            available_to_session: EvidenceState::NotObserved,
            content_loaded: EvidenceState::NotObserved,
            execution_used: EvidenceState::NotObserved,
            rejected_observations: 0,
        };
        for observation in observations {
            if observation.source_id != source_id || observation.capability_id != capability_id {
                continue;
            }
            let state = if observation.provider != self.policy.provider
                || observation.snapshot_digest != self.digest()
                || observation.source_digest != candidate.source_digest
                || !valid_ref(&observation.evidence_ref)
                || observation.observed_at_unix > now_unix
                || observation.observed_at_unix >= observation.expires_at_unix
            {
                EvidenceState::Mismatch
            } else if now_unix >= observation.expires_at_unix {
                EvidenceState::Stale
            } else {
                EvidenceState::Observed
            };
            let diagnostic = observation.kind == ObservationKind::DiagnosticSucceeded;
            let origin_matches = if diagnostic {
                observation.origin == ObservationOrigin::IndependentDiagnostic
                    && observation.session_id.is_none()
            } else {
                observation.origin == ObservationOrigin::NativeRuntime
                    && observation.session_id.as_deref().is_some_and(valid_ref)
                    && observation.session_id.as_deref() == Some(binding.session_id.as_str())
                    && binding.provider == self.policy.provider
                    && binding.config_snapshot_digest == self.digest()
            };
            let state = if origin_matches {
                state
            } else {
                EvidenceState::Mismatch
            };
            if state == EvidenceState::Mismatch {
                assessment.rejected_observations += 1;
            }
            let target = match observation.kind {
                ObservationKind::DiagnosticSucceeded => &mut assessment.diagnostic,
                ObservationKind::AvailableToSession => &mut assessment.available_to_session,
                ObservationKind::ContentLoaded => &mut assessment.content_loaded,
                ObservationKind::ExecutionUsed => &mut assessment.execution_used,
            };
            if state.priority() > target.priority() {
                *target = state;
            }
        }
        Ok(assessment)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionBinding {
    /// Baton-global identity from the trusted registry, never a provider-native
    /// session ID that may be reused across connections or accounts.
    pub session_id: String,
    pub provider: ProviderRevision,
    pub config_snapshot_digest: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationKind {
    DiagnosticSucceeded,
    AvailableToSession,
    ContentLoaded,
    ExecutionUsed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationOrigin {
    IndependentDiagnostic,
    NativeRuntime,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub source_id: String,
    pub capability_id: String,
    pub kind: ObservationKind,
    pub origin: ObservationOrigin,
    pub provider: ProviderRevision,
    pub snapshot_digest: String,
    pub source_digest: String,
    pub session_id: Option<String>,
    pub evidence_ref: String,
    pub observed_at_unix: u64,
    pub expires_at_unix: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    NotObserved,
    Mismatch,
    Stale,
    Observed,
}
impl EvidenceState {
    fn priority(self) -> u8 {
        match self {
            Self::NotObserved => 0,
            Self::Mismatch => 1,
            Self::Stale => 2,
            Self::Observed => 3,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceAssessment {
    pub configured: bool,
    pub enabled: bool,
    pub diagnostic: EvidenceState,
    pub available_to_session: EvidenceState,
    pub content_loaded: EvidenceState,
    pub execution_used: EvidenceState,
    pub rejected_observations: usize,
}
fn validate_descriptor(descriptor: &SourceDescriptor) -> Result<()> {
    if !valid_ref(&descriptor.source_id)
        || !valid_ref(&descriptor.locator_ref)
        || !valid_provider(&descriptor.provider)
        || (descriptor.mandatory && descriptor.kind != SourceKind::Organization)
    {
        return Err(error(ErrorCode::Conflict));
    }
    Ok(())
}
fn valid_label(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
fn valid_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-/".contains(&byte))
}
fn valid_provider(provider: &ProviderRevision) -> bool {
    valid_ref(&provider.provider_id) && valid_ref(&provider.version)
}
fn validate_definition(definition: &CapabilityDefinition) -> Result<()> {
    let refs: Vec<&str> = match definition {
        CapabilityDefinition::Skill(skill) => {
            if skill
                .compatible_with
                .iter()
                .any(|provider| !valid_provider(provider))
            {
                return Err(error(ErrorCode::Conflict));
            }
            std::iter::once(skill.content_ref.as_str())
                .chain(skill.dependency_refs.iter().map(String::as_str))
                .chain(skill.script_refs.iter().map(String::as_str))
                .collect()
        }
        CapabilityDefinition::Mcp(mcp) => {
            match &mcp.target {
                McpTarget::Stdio {
                    executable_ref,
                    arguments_ref,
                } => {
                    if !valid_ref(executable_ref)
                        || arguments_ref
                            .as_deref()
                            .is_some_and(|reference| !valid_ref(reference))
                    {
                        return Err(error(ErrorCode::Conflict));
                    }
                }
                McpTarget::Https { endpoint_ref } => {
                    if !valid_ref(endpoint_ref) {
                        return Err(error(ErrorCode::Conflict));
                    }
                }
            }
            mcp.credential_refs
                .iter()
                .map(String::as_str)
                .chain(mcp.tools_metadata_ref.iter().map(String::as_str))
                .collect()
        }
    };
    if refs.len() > 128 || refs.iter().any(|reference| !valid_ref(reference)) {
        return Err(error(ErrorCode::Conflict));
    }
    Ok(())
}
