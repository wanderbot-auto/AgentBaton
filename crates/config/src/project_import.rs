//! Read-only project configuration review and approved managed snapshots.
//! Project source files do not confer authority. The trusted caller resolves
//! project identity, current input revisions, non-secret text classification,
//! eligible registered MCP definitions, and the original approval.
use crate::{
    capabilities::{
        native::{
            DiscoveryLimits, InstructionOnlySkillMetadata, NativeFormat, NativeSourceRoot,
            NativeTransport, ParseStatus, SkillDialect, SkillDiscovery,
        },
        ProviderRevision, SourceDescriptor, SourceKind,
    },
    error, io_error, load_report, open_dir_at, open_private, outcome_unknown, read_at,
    runtime::{
        ManagedMcpServer, ManagedMcpTransport, NonSecretText, RuntimePlan, RuntimeProvider,
        SelectedSkill, SourceProvenance,
    },
    save_report, sha256, write_new, Edit, ErrorCode, FileReport, FileState, LockGuard, ManagedRoot,
    Phase, Report, Result, MAX_BYTES,
};
use rustix::fs::{self as rfs, AtFlags, FileType, Mode, OFlags};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const SNAPSHOT: &str = "project-config.json";
const VERSION: u32 = 1;
const MAX_CANDIDATES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectImportScope {
    pub project_id: String,
    pub project_revision: u64,
    pub input_revision: String,
    pub root: PathBuf,
    pub root_device: u64,
    pub root_inode: u64,
}

/// This table defines the selectable project-relative surfaces of this slice.
/// It is not a claim about a Provider's native search order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectSource {
    AgentSkills,
    CodexSkills,
    CodexConfig,
    ClaudeSkills,
    ClaudeMcp,
    ClaudeSettings,
    ClaudeLocalSettings,
}

enum SourceType {
    Skills(SkillDialect),
    Document(NativeFormat),
}

impl ProjectSource {
    fn locator(self) -> (&'static str, &'static str, SourceType) {
        use ProjectSource as Source;
        match self {
            Source::AgentSkills => (
                "agent-skills",
                ".agents/skills",
                SourceType::Skills(SkillDialect::Codex),
            ),
            Source::CodexSkills => (
                "codex-skills",
                ".codex/skills",
                SourceType::Skills(SkillDialect::Codex),
            ),
            Source::CodexConfig => (
                "codex-config",
                ".codex/config.toml",
                SourceType::Document(NativeFormat::CodexToml),
            ),
            Source::ClaudeSkills => (
                "claude-skills",
                ".claude/skills",
                SourceType::Skills(SkillDialect::Claude),
            ),
            Source::ClaudeMcp => (
                "claude-mcp",
                ".mcp.json",
                SourceType::Document(NativeFormat::ClaudeMcpJson),
            ),
            Source::ClaudeSettings => (
                "claude-settings",
                ".claude/settings.json",
                SourceType::Document(NativeFormat::ClaudeSettingsJson),
            ),
            Source::ClaudeLocalSettings => (
                "claude-local-settings",
                ".claude/settings.local.json",
                SourceType::Document(NativeFormat::ClaudeSettingsJson),
            ),
        }
    }

    fn runtime_provider(self) -> RuntimeProvider {
        match self {
            Self::AgentSkills | Self::CodexSkills | Self::CodexConfig => RuntimeProvider::Codex,
            _ => RuntimeProvider::ClaudeAcp,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectImportPolicy {
    pub revision: String,
    pub provider: ProviderRevision,
    /// All of these locations, including absent ones, are part of the review.
    pub sources: Vec<ProjectSource>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectCapabilityKind {
    Skill,
    Mcp,
    Hook,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectCapabilityKey {
    pub source: ProjectSource,
    pub kind: ProjectCapabilityKind,
    pub locator: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectCapabilityAdmission {
    ReviewedInstruction,
    RegisteredMcpOnly,
    Unsupported,
    ExcludedSensitiveFields,
    DisabledDeclaration,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectCapabilityPreview {
    pub key: ProjectCapabilityKey,
    pub name: String,
    pub source_sha256: String,
    pub admission: ProjectCapabilityAdmission,
    /// Number of competing declarations with this kind and name.
    pub conflict_count: usize,
    pub runtime_loading_known: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSourcePreview {
    pub source: ProjectSource,
    pub relative_path: String,
    pub status: Option<ParseStatus>,
    pub sha256: Option<String>,
    pub unknown_fields: usize,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectImportInspection {
    pub sources: Vec<ProjectSourcePreview>,
    pub candidates: Vec<ProjectCapabilityPreview>,
}

pub enum ProjectImportChoice<'a> {
    /// The trusted caller classified the exact reviewed document as non-secret.
    Skill {
        key: &'a ProjectCapabilityKey,
        nonsecret_sha256: &'a str,
    },
    /// A separately authorized, non-secret managed definition. No project
    /// command, argument, endpoint, environment value or hook is copied.
    RegisteredMcp {
        key: &'a ProjectCapabilityKey,
        definition: &'a ManagedMcpServer,
    },
}

pub struct ProjectImportRequest<'a> {
    /// A canonical UUID reserved by the registry for this confirmation.
    pub proposal_id: &'a str,
    pub scope: &'a ProjectImportScope,
    pub policy: &'a ProjectImportPolicy,
    pub managed_project_root: &'a Path,
    pub choices: &'a [ProjectImportChoice<'a>],
}

/// Serializable review metadata, not an authorization capability. Only the
/// trusted registry can resolve a live approval for this exact binding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectImportBinding {
    pub proposal_id: String,
    pub project_id: String,
    pub managed_root_device: u64,
    pub managed_root_inode: u64,
    pub snapshot_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProjectImportReview {
    pub binding: ProjectImportBinding,
    pub scope: ProjectImportScope,
    pub policy: ProjectImportPolicy,
    pub inspection: ProjectImportInspection,
    pub selections: Vec<ProjectImportSelectionPreview>,
    pub runtime_plan_sha256: String,
    pub new_runtime_required: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectImportSelectionPreview {
    pub capability: ProjectCapabilityKey,
    pub managed_provenance: SourceProvenance,
}

pub struct ConfirmedProjectImport<'a> {
    pub approval_ref: &'a str,
    pub binding: &'a ProjectImportBinding,
}

pub enum ProjectImportDecision<'a> {
    Skip,
    Cancel,
    Confirm(ConfirmedProjectImport<'a>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectImportState {
    Skipped,
    Cancelled,
    NotApplied,
    Applied,
    NeedsReconciliation,
}

struct Inspection {
    preview: ProjectImportInspection,
    skills: BTreeMap<ProjectCapabilityKey, (InstructionOnlySkillMetadata, String)>,
    mcp: BTreeMap<ProjectCapabilityKey, NativeTransport>,
    skill_catalogs: BTreeMap<ProjectSource, SkillDiscovery>,
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.:-".contains(&b))
        && value != "."
        && value != ".."
}

fn canonical_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|id| !id.is_nil() && id.to_string() == value)
}

fn document_digest(value: &impl Serialize) -> Result<String> {
    Ok(sha256(&io_error(serde_json::to_vec(value))?))
}

fn pin(directory: &File) -> Result<(u64, u64)> {
    let meta = io_error(directory.metadata())?;
    Ok((meta.dev(), meta.ino()))
}

fn source_directory(
    dir: impl std::os::fd::AsFd,
    name: &Path,
) -> std::result::Result<File, rustix::io::Errno> {
    let fd = rfs::openat(
        dir,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    let stat = rfs::fstat(&fd)?;
    if stat.st_uid != rustix::process::geteuid().as_raw() || stat.st_mode & 0o022 != 0 {
        return Err(rustix::io::Errno::ACCESS);
    }
    Ok(File::from(fd))
}

fn open_source(scope: &ProjectImportScope) -> Result<File> {
    let path = &scope.root;
    if !identifier(&scope.project_id)
        || scope.project_revision == 0
        || !identifier(&scope.input_revision)
    {
        return Err(error(ErrorCode::Conflict));
    }
    if !path.is_absolute() || path.parent().is_none() || io_error(path.canonicalize())? != *path {
        return Err(error(ErrorCode::InvalidPath));
    }
    let root = source_directory(rfs::CWD, path).map_err(|_| error(ErrorCode::PermissionDenied))?;
    if pin(&root)? != (scope.root_device, scope.root_inode) {
        return Err(error(ErrorCode::Conflict));
    }
    Ok(root)
}

fn source_present(root: &File, relative: &Path, directory: bool) -> Result<bool> {
    // The paths come only from ProjectSource::locator. Distinguish missing
    // entries from unsafe paths or unreadable directories.
    let mut parent = io_error(root.try_clone())?;
    for part in relative
        .parent()
        .ok_or_else(|| error(ErrorCode::InvalidPath))?
        .components()
    {
        parent = match source_directory(&parent, Path::new(part.as_os_str())) {
            Ok(next) => next,
            Err(rustix::io::Errno::NOENT) => return Ok(false),
            Err(_) => return Err(error(ErrorCode::PermissionDenied)),
        };
    }
    match rfs::statat(
        &parent,
        relative.file_name().unwrap(),
        AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Err(rustix::io::Errno::NOENT) => Ok(false),
        Ok(stat)
            if FileType::from_raw_mode(stat.st_mode)
                == if directory {
                    FileType::Directory
                } else {
                    FileType::RegularFile
                } =>
        {
            Ok(true)
        }
        _ => Err(error(ErrorCode::PermissionDenied)),
    }
}

fn policy_provider(policy: &ProjectImportPolicy) -> Result<RuntimeProvider> {
    let provider = match policy.provider.provider_id.as_str() {
        "codex" => RuntimeProvider::Codex,
        "claude" | "claude-code" => RuntimeProvider::ClaudeAcp,
        _ => return Err(error(ErrorCode::Conflict)),
    };
    if !identifier(&policy.revision)
        || !identifier(&policy.provider.version)
        || policy.sources.is_empty()
        || policy.sources.len() > 4
        || policy.sources.iter().collect::<BTreeSet<_>>().len() != policy.sources.len()
        || policy
            .sources
            .iter()
            .any(|source| source.runtime_provider() != provider)
    {
        return Err(error(ErrorCode::Conflict));
    }
    Ok(provider)
}

fn inspect(scope: &ProjectImportScope, policy: &ProjectImportPolicy) -> Result<Inspection> {
    policy_provider(policy)?;
    let root = open_source(scope)?;
    let inspection = inspect_pinned(&root, policy)?;
    open_source(scope)?;
    Ok(inspection)
}

fn inspect_pinned(root: &File, policy: &ProjectImportPolicy) -> Result<Inspection> {
    let reader = NativeSourceRoot::from_selected_directory(io_error(root.try_clone())?)?;
    let mut inspection = Inspection {
        preview: ProjectImportInspection {
            sources: vec![],
            candidates: vec![],
        },
        skills: BTreeMap::new(),
        mcp: BTreeMap::new(),
        skill_catalogs: BTreeMap::new(),
    };
    for source in &policy.sources {
        let (source_id, relative, source_type) = source.locator();
        let path = Path::new(relative);
        let mut summary = ProjectSourcePreview {
            source: *source,
            relative_path: relative.into(),
            status: None,
            sha256: None,
            unknown_fields: 0,
            diagnostics: vec![],
        };
        if source_present(root, path, matches!(source_type, SourceType::Skills(_)))? {
            let descriptor = SourceDescriptor {
                source_id: source_id.into(),
                kind: SourceKind::Project,
                locator_ref: relative.into(),
                provider: policy.provider.clone(),
                mandatory: false,
            };
            match source_type {
                SourceType::Skills(dialect) => {
                    let discovery = reader.discover_skills(
                        descriptor,
                        path,
                        dialect,
                        DiscoveryLimits::default(),
                    )?;
                    summary.sha256 = Some(document_digest(&discovery)?);
                    summary.status = Some(if discovery.complete {
                        ParseStatus::Parsed
                    } else {
                        ParseStatus::Partial
                    });
                    summary
                        .diagnostics
                        .extend(discovery.diagnostics.iter().map(|d| d.code.clone()));
                    for skill in &discovery.skills {
                        let key = ProjectCapabilityKey {
                            source: *source,
                            kind: ProjectCapabilityKind::Skill,
                            locator: safe_name(&skill.directory_name),
                        };
                        let selected =
                            if skill.status == ParseStatus::Parsed && skill.scripts.is_empty() {
                                reader
                                    .read_instruction_only_skill(&path.join(&skill.directory_name))
                                    .ok()
                            } else {
                                None
                            };
                        let mut admission = ProjectCapabilityAdmission::Unsupported;
                        if let Some((metadata, instruction)) = selected {
                            if metadata.sha256 != skill.source_digest {
                                return Err(error(ErrorCode::Conflict));
                            }
                            admission = ProjectCapabilityAdmission::ReviewedInstruction;
                            inspection
                                .skills
                                .insert(key.clone(), (metadata, instruction));
                        }
                        summary.unknown_fields += skill.unknown_frontmatter_names.len();
                        inspection.preview.candidates.push(candidate(
                            key,
                            safe_name(skill.name.as_deref().unwrap_or(&skill.directory_name)),
                            skill.source_digest.clone(),
                            admission,
                        ));
                    }
                    inspection.skill_catalogs.insert(*source, discovery);
                }
                SourceType::Document(format) => {
                    let document = reader.read_native(descriptor, path, format)?;
                    if !reader.verify_unchanged(&document)? {
                        return Err(error(ErrorCode::Conflict));
                    }
                    let native = document.preview();
                    summary.status = Some(native.status);
                    summary.sha256 = Some(native.source_digest.clone());
                    summary.unknown_fields = native.unknown_field_names.len();
                    summary
                        .diagnostics
                        .extend(native.diagnostics.into_iter().map(|d| d.code));
                    for server in native.mcp_servers {
                        let key = ProjectCapabilityKey {
                            source: *source,
                            kind: ProjectCapabilityKind::Mcp,
                            locator: safe_name(&server.name),
                        };
                        let admission = if server.inline_values_present
                            || !server.credential_refs.is_empty()
                            || !server.environment_names.is_empty()
                            || !server.header_names.is_empty()
                        {
                            ProjectCapabilityAdmission::ExcludedSensitiveFields
                        } else if server.enabled_declaration == Some(false) {
                            ProjectCapabilityAdmission::DisabledDeclaration
                        } else if native.status != ParseStatus::Parsed
                            || !server.unknown_field_names.is_empty()
                            || !identifier(&server.name)
                            || !matches!(
                                server.transport,
                                NativeTransport::Stdio | NativeTransport::Http
                            )
                        {
                            ProjectCapabilityAdmission::Unsupported
                        } else {
                            inspection.mcp.insert(key.clone(), server.transport);
                            ProjectCapabilityAdmission::RegisteredMcpOnly
                        };
                        inspection.preview.candidates.push(candidate(
                            key,
                            safe_name(&server.name),
                            native.source_digest.clone(),
                            admission,
                        ));
                    }
                    if let Some(settings) = native.settings {
                        for event in settings.hook_event_names {
                            let name = safe_name(&event);
                            let key = ProjectCapabilityKey {
                                source: *source,
                                kind: ProjectCapabilityKind::Hook,
                                locator: name.clone(),
                            };
                            inspection.preview.candidates.push(candidate(
                                key,
                                name,
                                native.source_digest.clone(),
                                ProjectCapabilityAdmission::Unsupported,
                            ));
                        }
                    }
                }
            }
        }
        summary.diagnostics.sort();
        summary.diagnostics.dedup();
        inspection.preview.sources.push(summary);
        if inspection.preview.candidates.len() > MAX_CANDIDATES {
            return Err(error(ErrorCode::ResourceLimit));
        }
    }
    let mut groups = BTreeMap::<_, usize>::new();
    for item in &inspection.preview.candidates {
        *groups
            .entry((item.key.kind, item.name.clone()))
            .or_default() += 1;
    }
    for item in &mut inspection.preview.candidates {
        item.conflict_count = groups[&(item.key.kind, item.name.clone())] - 1;
    }
    Ok(inspection)
}

fn safe_name(value: &str) -> String {
    if identifier(value) {
        value.into()
    } else {
        format!("opaque:{}", sha256(value.as_bytes()))
    }
}

fn candidate(
    key: ProjectCapabilityKey,
    name: String,
    source_sha256: String,
    admission: ProjectCapabilityAdmission,
) -> ProjectCapabilityPreview {
    ProjectCapabilityPreview {
        key,
        name,
        source_sha256,
        admission,
        conflict_count: 0,
        runtime_loading_known: false,
    }
}

pub fn inspect_project_config(
    scope: &ProjectImportScope,
    policy: &ProjectImportPolicy,
) -> Result<ProjectImportInspection> {
    Ok(inspect(scope, policy)?.preview)
}

#[derive(Serialize)]
struct Snapshot<'a> {
    schema_version: u32,
    proposal_id: &'a str,
    scope: &'a ProjectImportScope,
    policy: &'a ProjectImportPolicy,
    inspection: &'a ProjectImportInspection,
    selections: &'a [ProjectImportSelectionPreview],
    runtime_plan: &'a RuntimePlan,
    runtime_plan_sha256: &'a str,
}

fn target_root(path: &Path) -> Result<File> {
    if !path.is_absolute() || path.parent().is_none() || io_error(path.canonicalize())? != path {
        return Err(error(ErrorCode::InvalidPath));
    }
    open_dir_at(rfs::CWD, path)
}

fn prepare(request: &ProjectImportRequest<'_>) -> Result<(ProjectImportReview, Vec<u8>)> {
    if !canonical_uuid(request.proposal_id) || request.choices.len() > 32 {
        return Err(error(ErrorCode::Conflict));
    }
    require_disjoint(&request.scope.root, request.managed_project_root)?;
    let target = target_root(request.managed_project_root)?;
    let (managed_root_device, managed_root_inode) = pin(&target)?;
    let inspection = inspect(request.scope, request.policy)?;
    let mut plan = RuntimePlan::empty(policy_provider(request.policy)?);
    let mut names = BTreeSet::new();
    let mut selections = Vec::new();
    for choice in request.choices {
        let key = match choice {
            ProjectImportChoice::Skill { key, .. }
            | ProjectImportChoice::RegisteredMcp { key, .. } => *key,
        };
        let item = inspection
            .preview
            .candidates
            .iter()
            .find(|item| item.key == *key)
            .ok_or_else(|| error(ErrorCode::Conflict))?;
        if !names.insert((key.kind, &item.name)) {
            return Err(error(ErrorCode::Conflict));
        }
        let provenance = SourceProvenance {
            source_ref: format!("project-import:{}", key.source.locator().0),
            revision: request.scope.input_revision.clone(),
            sha256: item.source_sha256.clone(),
        };
        let managed_provenance = match choice {
            ProjectImportChoice::Skill {
                nonsecret_sha256, ..
            } => {
                let (metadata, instruction) = inspection
                    .skills
                    .get(key)
                    .ok_or_else(|| error(ErrorCode::PermissionDenied))?;
                if *nonsecret_sha256 != item.source_sha256 {
                    return Err(error(ErrorCode::Conflict));
                }
                plan.skills.push(SelectedSkill {
                    directory_name: metadata.directory_name.clone(),
                    instruction: NonSecretText::approved(instruction.clone())?,
                    assets: vec![],
                    provenance: provenance.clone(),
                });
                provenance
            }
            ProjectImportChoice::RegisteredMcp { definition, .. } => {
                let expected = inspection
                    .mcp
                    .get(key)
                    .ok_or_else(|| error(ErrorCode::PermissionDenied))?;
                let transport = match &definition.transport {
                    ManagedMcpTransport::Stdio { .. } => NativeTransport::Stdio,
                    ManagedMcpTransport::Http { .. } => NativeTransport::Http,
                };
                if *expected != transport || definition.name != item.name {
                    return Err(error(ErrorCode::Conflict));
                }
                plan.mcp_servers.push((*definition).clone());
                plan.source_refs.push(provenance);
                definition.provenance.clone()
            }
        };
        selections.push(ProjectImportSelectionPreview {
            capability: key.clone(),
            managed_provenance,
        });
    }
    plan.skills
        .sort_by(|a, b| a.directory_name.cmp(&b.directory_name));
    plan.mcp_servers.sort_by(|a, b| a.name.cmp(&b.name));
    plan.source_refs
        .sort_by(|a, b| (&a.source_ref, &a.sha256).cmp(&(&b.source_ref, &b.sha256)));
    plan.source_refs.dedup();
    plan.validate()?;
    let runtime_plan_sha256 = plan.plan_digest()?;
    selections.sort_by(|a, b| a.capability.cmp(&b.capability));
    let bytes = io_error(serde_json::to_vec(&Snapshot {
        schema_version: VERSION,
        proposal_id: request.proposal_id,
        scope: request.scope,
        policy: request.policy,
        inspection: &inspection.preview,
        selections: &selections,
        runtime_plan: &plan,
        runtime_plan_sha256: &runtime_plan_sha256,
    }))?;
    if bytes.len() > MAX_BYTES {
        return Err(error(ErrorCode::ResourceLimit));
    }
    Ok((
        ProjectImportReview {
            binding: ProjectImportBinding {
                proposal_id: request.proposal_id.into(),
                project_id: request.scope.project_id.clone(),
                managed_root_device,
                managed_root_inode,
                snapshot_sha256: sha256(&bytes),
            },
            scope: request.scope.clone(),
            policy: request.policy.clone(),
            inspection: inspection.preview,
            selections,
            runtime_plan_sha256,
            new_runtime_required: true,
        },
        bytes,
    ))
}

pub fn prepare_project_import(request: &ProjectImportRequest<'_>) -> Result<ProjectImportReview> {
    Ok(prepare(request)?.0)
}

fn approved_target(path: &Path, approval: &ConfirmedProjectImport<'_>) -> Result<File> {
    let scope = approval.binding;
    if !identifier(approval.approval_ref)
        || !identifier(&scope.project_id)
        || !canonical_uuid(&scope.proposal_id)
        || !crate::valid_hash(&scope.snapshot_sha256)
    {
        return Err(error(ErrorCode::PermissionDenied));
    }
    let root = target_root(path)?;
    if pin(&root)? != (scope.managed_root_device, scope.managed_root_inode) {
        return Err(error(ErrorCode::Conflict));
    }
    Ok(root)
}

fn observed_snapshot(
    root: &File,
    relative: &Path,
    transaction_id: &str,
    digest: &str,
    valid_contents: impl FnOnce(&[u8]) -> bool,
) -> Result<(ProjectImportState, bool)> {
    use ProjectImportState as State;
    let lock = match rfs::statat(root, ".baton-config.lock", AtFlags::SYMLINK_NOFOLLOW) {
        Err(rustix::io::Errno::NOENT) => None,
        Ok(_) => match open_private(root, Path::new(".baton-config.lock"), false) {
            Ok(lock) => Some(lock),
            Err(_) => return Ok((State::NeedsReconciliation, true)),
        },
        Err(_) => return Ok((State::NeedsReconciliation, true)),
    };
    let _guard = match lock.as_ref().map(LockGuard::acquire).transpose() {
        Ok(guard) => guard,
        Err(_) => return Ok((State::NeedsReconciliation, true)),
    };
    let current = match read_at(root, relative) {
        Ok(bytes) => bytes,
        Err(_) => return Ok((State::NeedsReconciliation, true)),
    };
    let journal = (|| {
        match rfs::statat(root, ".baton-transactions", AtFlags::SYMLINK_NOFOLLOW) {
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Ok(_) if lock.is_none() => return Err(error(ErrorCode::Conflict)),
            Ok(_) => {}
            Err(_) => return Err(error(ErrorCode::Io)),
        }
        let journals = open_dir_at(root, Path::new(".baton-transactions"))?;
        match rfs::statat(&journals, transaction_id, AtFlags::SYMLINK_NOFOLLOW) {
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Ok(_) => {}
            Err(_) => return Err(error(ErrorCode::Io)),
        }
        let journal = open_dir_at(&journals, Path::new(transaction_id))?;
        load_report(&journal, transaction_id).map(Some)
    })();
    let report = match journal {
        Ok(None) if current.is_none() => return Ok((State::NotApplied, false)),
        Ok(Some(report)) => report,
        _ => return Ok((State::NeedsReconciliation, true)),
    };
    let [file] = report.files.as_slice() else {
        return Ok((State::NeedsReconciliation, true));
    };
    if (report.root_device, report.root_inode) != pin(root)?
        || file.relative_path != relative
        || file.before_sha256.is_some()
        || file.after_sha256 != digest
        || matches!(
            report.phase,
            Phase::RollingBack | Phase::RolledBack | Phase::Conflict
        )
    {
        return Ok((State::NeedsReconciliation, true));
    }
    match current.as_deref() {
        Some(bytes) if sha256(bytes) == digest => {
            if !valid_contents(bytes) {
                return Ok((State::NeedsReconciliation, true));
            }
            // A crash can leave an applying journal after publication. The
            // replacement was fsynced before rename. Stabilize the directory
            // entry before reporting that the exact snapshot is applied.
            if (report.phase == Phase::Applied && file.state == FileState::Written)
                || root.sync_all().is_ok()
            {
                Ok((State::Applied, true))
            } else {
                Ok((State::NeedsReconciliation, true))
            }
        }
        None if matches!(report.phase, Phase::Applying | Phase::PartialFailure)
            && matches!(file.state, FileState::Pending | FileState::NotApplied) =>
        {
            Ok((State::NotApplied, true))
        }
        _ => Ok((State::NeedsReconciliation, true)),
    }
}

fn observed_state(
    root: &File,
    binding: &ProjectImportBinding,
) -> Result<(ProjectImportState, bool)> {
    #[derive(Deserialize)]
    struct Header {
        schema_version: u32,
        proposal_id: String,
        scope: ProjectImportScope,
    }
    observed_snapshot(
        root,
        Path::new(SNAPSHOT),
        &binding.proposal_id,
        &binding.snapshot_sha256,
        |bytes| {
            serde_json::from_slice::<Header>(bytes).is_ok_and(|header| {
                header.schema_version == VERSION
                    && header.proposal_id == binding.proposal_id
                    && header.scope.project_id == binding.project_id
            })
        },
    )
}

/// Point-in-time reconciliation of the approved destination receipt and exact
/// bytes. No uncertain write is replayed; first-import snapshots are immutable.
pub fn recover_project_import(
    path: &Path,
    approval: &ConfirmedProjectImport<'_>,
) -> Result<ProjectImportState> {
    let root = approved_target(path, approval)?;
    Ok(observed_state(&root, approval.binding)?.0)
}

/// Read the persisted normalized plan by exact trusted approval after
/// confirmation or restart. A later runtime materializer and execution grant
/// are still required before a Provider may load the returned plan.
pub fn read_imported_project_plan(
    path: &Path,
    approval: &ConfirmedProjectImport<'_>,
) -> Result<Option<RuntimePlan>> {
    let root = approved_target(path, approval)?;
    Ok(read_approved_import(&root, approval.binding)?.map(|stored| stored.runtime_plan))
}

#[derive(Deserialize)]
struct StoredImport {
    schema_version: u32,
    proposal_id: String,
    scope: ProjectImportScope,
    policy: ProjectImportPolicy,
    inspection: ProjectImportInspection,
    selections: Vec<ProjectImportSelectionPreview>,
    runtime_plan_sha256: String,
    runtime_plan: RuntimePlan,
}

fn read_approved_import(
    root: &File,
    binding: &ProjectImportBinding,
) -> Result<Option<StoredImport>> {
    match observed_state(root, binding)?.0 {
        ProjectImportState::Applied => {}
        ProjectImportState::NotApplied => return Ok(None),
        _ => return Err(error(ErrorCode::OutcomeUnknown)),
    }
    let bytes = read_exact_snapshot(root, Path::new(SNAPSHOT), &binding.snapshot_sha256)?;
    let plan: StoredImport = io_error(serde_json::from_slice(&bytes))?;
    if plan.schema_version != VERSION
        || plan.proposal_id != binding.proposal_id
        || plan.scope.project_id != binding.project_id
        || plan.runtime_plan.plan_digest()? != plan.runtime_plan_sha256
    {
        return Err(error(ErrorCode::Conflict));
    }
    Ok(Some(plan))
}

fn read_exact_snapshot(root: &File, relative: &Path, digest: &str) -> Result<Vec<u8>> {
    let bytes = read_at(root, relative)?.ok_or_else(|| error(ErrorCode::Conflict))?;
    if sha256(&bytes) != digest {
        return Err(error(ErrorCode::Conflict));
    }
    Ok(bytes)
}

pub fn apply_project_import(
    request: &ProjectImportRequest<'_>,
    decision: ProjectImportDecision<'_>,
) -> Result<ProjectImportState> {
    use ProjectImportState as State;
    let approval = match decision {
        ProjectImportDecision::Skip => return Ok(State::Skipped),
        ProjectImportDecision::Cancel => return Ok(State::Cancelled),
        ProjectImportDecision::Confirm(approval) => approval,
    };
    let binding = approval.binding;
    if binding.project_id != request.scope.project_id || binding.proposal_id != request.proposal_id
    {
        return Err(error(ErrorCode::Conflict));
    }
    let target = approved_target(request.managed_project_root, &approval)?;
    let (state, started) = observed_state(&target, binding)?;
    if started {
        return Ok(state);
    }
    let (review, bytes) = prepare(request)?;
    if review.binding != *binding {
        return Err(error(ErrorCode::Conflict));
    }
    // Use the already verified destination descriptor. Opening a fresh path
    // here could initialize transaction files in a replacement directory.
    let managed = ManagedRoot::from_private_directory(target)?;
    let result = managed
        .plan(vec![Edit::nonsecret_replacement(SNAPSHOT, None, bytes)])
        .and_then(|mut change| {
            change.id.clone_from(&binding.proposal_id);
            managed.apply(change)
        });
    let (state, started) = observed_state(
        &approved_target(request.managed_project_root, &approval)?,
        binding,
    )?;
    if started || result.is_ok() {
        Ok(state)
    } else {
        result.map(|_| state)
    }
}

/// This rule admits changed bytes for explicitly selected instruction-only
/// skills. Deployment, executable definitions, and all other source structure
/// stay bound to the approved import.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfigUpdateRule {
    pub revision: String,
    pub instruction_keys: BTreeSet<ProjectCapabilityKey>,
}

/// A review binding to persist in the trusted approval registry. Deserializing
/// it from an untrusted caller does not establish approval.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfigUpdateBinding {
    pub initial_import: ProjectImportBinding,
    rule_sha256: String,
    source_shapes: BTreeMap<ProjectSource, String>,
}

pub struct ConfirmedProjectConfigUpdate<'a> {
    pub approval_ref: &'a str,
    pub binding: &'a ProjectConfigUpdateBinding,
    pub rule: &'a ProjectConfigUpdateRule,
}

/// Exact local version identity. The caller retains it before requesting
/// publication and supplies its accepted version as the next update's base.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfigVersionBinding {
    pub transaction_id: String,
    pub parent_sha256: String,
    pub snapshot_sha256: String,
}

pub struct ProjectConfigUpdateRequest<'a> {
    pub transaction_id: &'a str,
    pub managed_project_root: &'a Path,
    pub scope: &'a ProjectImportScope,
    pub policy: &'a ProjectImportPolicy,
    /// None selects the original approved import.
    pub base: Option<&'a ProjectConfigVersionBinding>,
    /// Trusted non-secret classifications for the exact new instruction bytes.
    pub classified_instructions: &'a BTreeMap<ProjectCapabilityKey, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason", content = "subject")]
pub enum ProjectConfigUpdateReviewReason {
    ScopeOrPolicyChanged,
    UnverifiableSource,
    StaleBase,
    UnresolvedConflict,
    ExecutableOrSettingsChanged(ProjectSource),
    CapabilityStructureChanged(ProjectSource),
    InstructionClassificationRequired(ProjectCapabilityKey),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectConfigUpdatePreview {
    Unchanged,
    ReviewRequired(ProjectConfigUpdateReviewReason),
    Ready(ProjectConfigVersionBinding),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredUpdate {
    schema_version: u32,
    transaction_id: String,
    parent_sha256: String,
    approval_sha256: String,
    source_state_sha256: String,
    runtime_plan_sha256: String,
    runtime_plan: RuntimePlan,
}

struct PreparedUpdate {
    binding: ProjectConfigVersionBinding,
    source_state_sha256: String,
    bytes: Vec<u8>,
}

enum EvaluatedUpdate {
    Ready(PreparedUpdate),
    Unchanged,
    Review(ProjectConfigUpdateReviewReason),
}

fn update_slot(parent_sha256: &str) -> PathBuf {
    PathBuf::from(format!("project-config-next-{parent_sha256}.json"))
}

fn valid_version(version: &ProjectConfigVersionBinding) -> bool {
    canonical_uuid(&version.transaction_id)
        && crate::valid_hash(&version.parent_sha256)
        && crate::valid_hash(&version.snapshot_sha256)
        && version.parent_sha256 != version.snapshot_sha256
}

fn validate_rule(rule: &ProjectConfigUpdateRule) -> Result<()> {
    if !identifier(&rule.revision)
        || rule.instruction_keys.is_empty()
        || rule.instruction_keys.len() > 32
        || rule
            .instruction_keys
            .iter()
            .any(|key| key.kind != ProjectCapabilityKind::Skill || !identifier(&key.locator))
    {
        return Err(error(ErrorCode::Conflict));
    }
    Ok(())
}

fn source_shapes(
    inspection: &Inspection,
    rule: &ProjectConfigUpdateRule,
) -> Result<BTreeMap<ProjectSource, String>> {
    inspection
        .preview
        .sources
        .iter()
        .map(|source| {
            let mut summary = source.clone();
            let mut catalog = inspection.skill_catalogs.get(&source.source).cloned();
            let mut candidates: Vec<_> = inspection
                .preview
                .candidates
                .iter()
                .filter(|item| item.key.source == source.source)
                .cloned()
                .collect();
            if let Some(catalog) = catalog.as_mut().filter(|_| {
                rule.instruction_keys
                    .iter()
                    .any(|key| key.source == source.source)
            }) {
                // The full catalog includes per-skill script digests, diagnostics
                // and metadata. Mask only allowed document hashes and the
                // aggregate that changes when the document length changes.
                summary.sha256 = None;
                catalog.inspected_bytes = 0;
                for skill in &mut catalog.skills {
                    let key = ProjectCapabilityKey {
                        source: source.source,
                        kind: ProjectCapabilityKind::Skill,
                        locator: safe_name(&skill.directory_name),
                    };
                    if rule.instruction_keys.contains(&key) {
                        skill.source_digest.clear();
                    }
                }
                for item in &mut candidates {
                    if rule.instruction_keys.contains(&item.key) {
                        item.source_sha256.clear();
                    }
                }
            }
            Ok((
                source.source,
                document_digest(&(summary, catalog, candidates))?,
            ))
        })
        .collect()
}

fn require_disjoint(source: &Path, target: &Path) -> Result<()> {
    if source.starts_with(target) || target.starts_with(source) {
        return Err(error(ErrorCode::PermissionDenied));
    }
    Ok(())
}

/// Derive the update rule's review binding while the source still matches the
/// exact approved first-import inspection. This operation is read-only.
pub fn prepare_project_config_update_binding(
    managed_project_root: &Path,
    first: &ConfirmedProjectImport<'_>,
    rule: &ProjectConfigUpdateRule,
) -> Result<ProjectConfigUpdateBinding> {
    validate_rule(rule)?;
    let root = approved_target(managed_project_root, first)?;
    let anchor =
        read_approved_import(&root, first.binding)?.ok_or_else(|| error(ErrorCode::Conflict))?;
    require_disjoint(&anchor.scope.root, managed_project_root)?;
    let inspection = inspect(&anchor.scope, &anchor.policy)?;
    if document_digest(&inspection.preview)? != document_digest(&anchor.inspection)? {
        return Err(error(ErrorCode::Conflict));
    }
    for key in &rule.instruction_keys {
        let complete = inspection.preview.sources.iter().any(|source| {
            source.source == key.source && source.status == Some(ParseStatus::Parsed)
        });
        let instruction_only = inspection
            .preview
            .candidates
            .iter()
            .filter(|candidate| candidate.key.source == key.source)
            .all(|candidate| {
                candidate.admission == ProjectCapabilityAdmission::ReviewedInstruction
            });
        if !complete
            || !instruction_only
            || !inspection.skills.contains_key(key)
            || !anchor
                .selections
                .iter()
                .any(|selection| selection.capability == *key)
        {
            return Err(error(ErrorCode::PermissionDenied));
        }
    }
    Ok(ProjectConfigUpdateBinding {
        initial_import: first.binding.clone(),
        rule_sha256: document_digest(rule)?,
        source_shapes: source_shapes(&inspection, rule)?,
    })
}

fn update_target(path: &Path, approval: &ConfirmedProjectConfigUpdate<'_>) -> Result<File> {
    validate_rule(approval.rule)?;
    if document_digest(approval.rule)? != approval.binding.rule_sha256 {
        return Err(error(ErrorCode::PermissionDenied));
    }
    approved_target(
        path,
        &ConfirmedProjectImport {
            approval_ref: approval.approval_ref,
            binding: &approval.binding.initial_import,
        },
    )
}

fn matches_update(
    record: &StoredUpdate,
    version: &ProjectConfigVersionBinding,
    approval_sha256: &str,
) -> bool {
    record.schema_version == VERSION
        && record.transaction_id == version.transaction_id
        && record.parent_sha256 == version.parent_sha256
        && record.approval_sha256 == approval_sha256
        && crate::valid_hash(&record.source_state_sha256)
        && record
            .runtime_plan
            .plan_digest()
            .is_ok_and(|digest| digest == record.runtime_plan_sha256)
}

fn observed_update(
    root: &File,
    approval: &ProjectConfigUpdateBinding,
    version: &ProjectConfigVersionBinding,
) -> Result<(ProjectImportState, bool)> {
    if !valid_version(version) || version.transaction_id == approval.initial_import.proposal_id {
        return Err(error(ErrorCode::Conflict));
    }
    let approved = document_digest(approval)?;
    observed_snapshot(
        root,
        &update_slot(&version.parent_sha256),
        &version.transaction_id,
        &version.snapshot_sha256,
        |bytes| {
            serde_json::from_slice::<StoredUpdate>(bytes)
                .is_ok_and(|record| matches_update(&record, version, &approved))
        },
    )
}

fn read_update(
    root: &File,
    approval: &ProjectConfigUpdateBinding,
    version: &ProjectConfigVersionBinding,
) -> Result<Option<StoredUpdate>> {
    match observed_update(root, approval, version)?.0 {
        ProjectImportState::Applied => {}
        ProjectImportState::NotApplied => return Ok(None),
        _ => return Err(error(ErrorCode::OutcomeUnknown)),
    }
    let bytes = read_exact_snapshot(
        root,
        &update_slot(&version.parent_sha256),
        &version.snapshot_sha256,
    )?;
    let record = io_error(serde_json::from_slice::<StoredUpdate>(&bytes))?;
    if !matches_update(&record, version, &document_digest(approval)?) {
        return Err(error(ErrorCode::Conflict));
    }
    Ok(Some(record))
}

fn same_source(left: &ProjectImportScope, right: &ProjectImportScope) -> bool {
    left.project_id == right.project_id
        && left.project_revision == right.project_revision
        && left.root == right.root
        && left.root_device == right.root_device
        && left.root_inode == right.root_inode
}

fn evaluate_update(
    root: &File,
    request: &ProjectConfigUpdateRequest<'_>,
    approval: &ConfirmedProjectConfigUpdate<'_>,
) -> Result<EvaluatedUpdate> {
    use ProjectConfigUpdateReviewReason as Reason;
    let first = &approval.binding.initial_import;
    if !canonical_uuid(request.transaction_id)
        || request.transaction_id == first.proposal_id
        || request.classified_instructions.len() > 32
    {
        return Err(error(ErrorCode::Conflict));
    }
    let anchor = read_approved_import(root, first)?.ok_or_else(|| error(ErrorCode::Conflict))?;
    require_disjoint(&anchor.scope.root, request.managed_project_root)?;
    if !same_source(request.scope, &anchor.scope)
        || document_digest(request.policy)? != document_digest(&anchor.policy)?
    {
        return Ok(EvaluatedUpdate::Review(Reason::ScopeOrPolicyChanged));
    }
    let (previous_sha256, previous_source, mut plan) = match request.base {
        Some(previous) => {
            if previous.transaction_id == request.transaction_id {
                return Err(error(ErrorCode::Conflict));
            }
            let previous_record = read_update(root, approval.binding, previous)?
                .ok_or_else(|| error(ErrorCode::Conflict))?;
            (
                &previous.snapshot_sha256,
                previous_record.source_state_sha256,
                previous_record.runtime_plan,
            )
        }
        None => (
            &first.snapshot_sha256,
            document_digest(&anchor.inspection)?,
            anchor.runtime_plan,
        ),
    };
    if read_at(root, &update_slot(previous_sha256))?.is_some() {
        return Ok(EvaluatedUpdate::Review(Reason::StaleBase));
    }
    let Ok(inspection) = inspect(request.scope, request.policy) else {
        return Ok(EvaluatedUpdate::Review(Reason::UnverifiableSource));
    };
    let source_state_sha256 = document_digest(&inspection.preview)?;
    if source_state_sha256 == previous_source {
        return Ok(EvaluatedUpdate::Unchanged);
    }
    if inspection
        .preview
        .candidates
        .iter()
        .any(|item| item.conflict_count != 0)
    {
        return Ok(EvaluatedUpdate::Review(Reason::UnresolvedConflict));
    }
    let current_shapes = source_shapes(&inspection, approval.rule)?;
    for (source, digest) in &current_shapes {
        if approval.binding.source_shapes.get(source) != Some(digest) {
            let reason = match source.locator().2 {
                SourceType::Skills(_) => Reason::CapabilityStructureChanged(*source),
                SourceType::Document(_) => Reason::ExecutableOrSettingsChanged(*source),
            };
            return Ok(EvaluatedUpdate::Review(reason));
        }
    }
    if current_shapes.len() != approval.binding.source_shapes.len() {
        return Ok(EvaluatedUpdate::Review(Reason::UnverifiableSource));
    }
    let mut changed = false;
    for key in &approval.rule.instruction_keys {
        let (metadata, instruction) = inspection
            .skills
            .get(key)
            .ok_or_else(|| error(ErrorCode::Conflict))?;
        let source_ref = format!("project-import:{}", key.source.locator().0);
        let slot = plan
            .skills
            .iter_mut()
            .find(|skill| {
                skill.directory_name == metadata.directory_name
                    && skill.provenance.source_ref == source_ref
            })
            .ok_or_else(|| error(ErrorCode::Conflict))?;
        if slot.provenance.sha256 == metadata.sha256 {
            continue;
        }
        if request.classified_instructions.get(key) != Some(&metadata.sha256) {
            return Ok(EvaluatedUpdate::Review(
                Reason::InstructionClassificationRequired(key.clone()),
            ));
        }
        slot.instruction = NonSecretText::approved(instruction.clone())?;
        slot.provenance
            .revision
            .clone_from(&request.scope.input_revision);
        slot.provenance.sha256.clone_from(&metadata.sha256);
        changed = true;
    }
    if !changed {
        // Never explain an unclassified observational difference as an
        // approved content edit.
        return Ok(EvaluatedUpdate::Review(Reason::UnverifiableSource));
    }
    let record = StoredUpdate {
        schema_version: VERSION,
        transaction_id: request.transaction_id.into(),
        parent_sha256: previous_sha256.clone(),
        approval_sha256: document_digest(approval.binding)?,
        source_state_sha256: source_state_sha256.clone(),
        runtime_plan_sha256: plan.plan_digest()?,
        runtime_plan: plan,
    };
    let bytes = io_error(serde_json::to_vec(&record))?;
    if bytes.len() > MAX_BYTES {
        return Err(error(ErrorCode::ResourceLimit));
    }
    Ok(EvaluatedUpdate::Ready(PreparedUpdate {
        binding: ProjectConfigVersionBinding {
            transaction_id: record.transaction_id,
            parent_sha256: record.parent_sha256,
            snapshot_sha256: sha256(&bytes),
        },
        source_state_sha256,
        bytes,
    }))
}

/// Read-only drift classification. Ready means the existing rule covers these
/// exact bytes. The trusted caller should retain the binding before applying.
pub fn prepare_project_config_update(
    request: &ProjectConfigUpdateRequest<'_>,
    approval: &ConfirmedProjectConfigUpdate<'_>,
) -> Result<ProjectConfigUpdatePreview> {
    let root = update_target(request.managed_project_root, approval)?;
    Ok(match evaluate_update(&root, request, approval)? {
        EvaluatedUpdate::Unchanged => ProjectConfigUpdatePreview::Unchanged,
        EvaluatedUpdate::Review(reason) => ProjectConfigUpdatePreview::ReviewRequired(reason),
        EvaluatedUpdate::Ready(update) => ProjectConfigUpdatePreview::Ready(update.binding),
    })
}

// Reuse the managed root's lock, transaction directory, report contract and
// durability routines. A per-parent slot needs an OS-level exclusive rename
// even against a non-cooperating writer that appears after revalidation.
fn publish_update(
    root: &ManagedRoot,
    update: PreparedUpdate,
    revalidate: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let _lock = LockGuard::acquire(&root.lock)?;
    let version = update.binding;
    let relative_path = update_slot(&version.parent_sha256);
    if read_at(&root.root, &relative_path)?.is_some() {
        return Err(error(ErrorCode::Conflict));
    }
    let transaction = root.create_transaction(&version.transaction_id)?;
    let mut report = Report {
        schema_version: 1,
        transaction_id: version.transaction_id,
        root_device: root.root_identity.0,
        root_inode: root.root_identity.1,
        phase: Phase::Applying,
        files: vec![FileReport {
            relative_path,
            before_sha256: None,
            after_sha256: version.snapshot_sha256,
            state: FileState::Pending,
        }],
    };
    save_report(&transaction, &report)?;
    write_new(&transaction, "pending-version", &update.bytes)?;
    revalidate()?;
    rfs::renameat_with(
        &transaction,
        "pending-version",
        &root.root,
        &report.files[0].relative_path,
        rfs::RenameFlags::NOREPLACE,
    )
    .map_err(|failure| {
        if failure == rustix::io::Errno::EXIST {
            error(ErrorCode::Conflict)
        } else {
            error(ErrorCode::Io)
        }
    })?;
    let post_publish = || outcome_unknown(&report.transaction_id);
    root.root.sync_all().map_err(|_| post_publish())?;
    transaction.sync_all().map_err(|_| post_publish())?;
    report.phase = Phase::Applied;
    report.files[0].state = FileState::Written;
    save_report(&transaction, &report).map_err(|_| outcome_unknown(&report.transaction_id))
}

/// Publish the exact prepared version through the already-approved rule.
/// An existing intent is reconciled by exact bytes and is never replayed.
pub fn apply_project_config_update(
    request: &ProjectConfigUpdateRequest<'_>,
    approval: &ConfirmedProjectConfigUpdate<'_>,
    expected: &ProjectConfigVersionBinding,
) -> Result<ProjectImportState> {
    if expected.transaction_id != request.transaction_id {
        return Err(error(ErrorCode::Conflict));
    }
    let target = update_target(request.managed_project_root, approval)?;
    let first = &approval.binding.initial_import;
    let anchor = read_approved_import(&target, first)?.ok_or_else(|| error(ErrorCode::Conflict))?;
    require_disjoint(&anchor.scope.root, request.managed_project_root)?;
    let parent = request
        .base
        .map_or(&first.snapshot_sha256, |base| &base.snapshot_sha256);
    if expected.parent_sha256 != *parent
        || !same_source(request.scope, &anchor.scope)
        || document_digest(request.policy)? != document_digest(&anchor.policy)?
    {
        return Err(error(ErrorCode::Conflict));
    }
    let (observed, started) = observed_update(&target, approval.binding, expected)?;
    if started {
        return Ok(observed);
    }
    let EvaluatedUpdate::Ready(update) = evaluate_update(&target, request, approval)? else {
        return Err(error(ErrorCode::Conflict));
    };
    if update.binding != *expected {
        return Err(error(ErrorCode::Conflict));
    }
    let observation = update.source_state_sha256.clone();
    let managed = ManagedRoot::from_private_directory(target)?;
    let result = publish_update(&managed, update, || {
        let root = update_target(request.managed_project_root, approval)?;
        let first = &approval.binding.initial_import;
        read_exact_snapshot(&root, Path::new(SNAPSHOT), &first.snapshot_sha256)?;
        if let Some(previous) = request.base {
            read_exact_snapshot(
                &root,
                &update_slot(&previous.parent_sha256),
                &previous.snapshot_sha256,
            )?;
        }
        let current = inspect(request.scope, request.policy)?;
        if document_digest(&current.preview)? != observation {
            return Err(error(ErrorCode::Conflict));
        }
        Ok(())
    });
    let Ok(root) = update_target(request.managed_project_root, approval) else {
        return Ok(ProjectImportState::NeedsReconciliation);
    };
    let (observed, started) = observed_update(&root, approval.binding, expected)?;
    if started || result.is_ok() {
        Ok(observed)
    } else {
        result.map(|_| observed)
    }
}

/// Reconcile the exact operation after restart or an ambiguous response.
pub fn recover_project_config_update(
    managed_project_root: &Path,
    approval: &ConfirmedProjectConfigUpdate<'_>,
    version: &ProjectConfigVersionBinding,
) -> Result<ProjectImportState> {
    let root = update_target(managed_project_root, approval)?;
    Ok(observed_update(&root, approval.binding, version)?.0)
}

/// Read an exact version for a new runtime or an existing version reference.
pub fn read_project_config_version(
    managed_project_root: &Path,
    approval: &ConfirmedProjectConfigUpdate<'_>,
    version: &ProjectConfigVersionBinding,
) -> Result<Option<RuntimePlan>> {
    let root = update_target(managed_project_root, approval)?;
    Ok(read_update(&root, approval.binding, version)?.map(|record| record.runtime_plan))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Ownership;
    use std::{fs, os::unix::fs::PermissionsExt};

    #[test]
    fn source_root_swap_does_not_redirect_native_or_skill_reads() {
        let private = tempfile::tempdir().unwrap();
        let original = private.path().join("project");
        let other = private.path().join("other");
        let parked = private.path().join("parked");
        let approved_skill = "---\nname: review\ndescription: Fixture\n---\nApproved text.\n";
        let substituted_skill = approved_skill.replace("Approved text.", "Substituted text.");
        for (path, server, skill) in [
            (&original, "approved-server", approved_skill),
            (&other, "substituted-server", &substituted_skill),
        ] {
            let skill_directory = path.join(".claude/skills/review");
            fs::create_dir_all(&skill_directory).unwrap();
            fs::write(skill_directory.join("SKILL.md"), skill).unwrap();
            let mcp = serde_json::json!({"mcpServers": { (server): {"command": "never-run"} }});
            fs::write(path.join(".mcp.json"), serde_json::to_vec(&mcp).unwrap()).unwrap();
        }
        let original = original.canonicalize().unwrap();
        let metadata = fs::metadata(&original).unwrap();
        let scope = ProjectImportScope {
            project_id: "root-swap-fixture".into(),
            project_revision: 1,
            input_revision: "fixture-input".into(),
            root: original.clone(),
            root_device: metadata.dev(),
            root_inode: metadata.ino(),
        };
        let policy = ProjectImportPolicy {
            revision: "fixture-policy".into(),
            provider: ProviderRevision {
                provider_id: "claude-code".into(),
                version: "fixture-v1".into(),
            },
            sources: vec![ProjectSource::ClaudeSkills, ProjectSource::ClaudeMcp],
        };
        let initial = document_digest(&inspect_project_config(&scope, &policy).unwrap()).unwrap();
        let pinned = open_source(&scope).unwrap();
        // Swap real directories after the approved descriptor is acquired. A
        // normal path open now reaches the other directory without symlinks.
        fs::rename(&original, &parked).unwrap();
        fs::rename(&other, &original).unwrap();
        assert!(fs::read_to_string(original.join(".mcp.json"))
            .unwrap()
            .contains("substituted-server"));
        assert_eq!(
            inspect_project_config(&scope, &policy).unwrap_err().code,
            ErrorCode::Conflict
        );

        let inspected = inspect_pinned(&pinned, &policy).unwrap();
        assert_eq!(document_digest(&inspected.preview).unwrap(), initial);
        let skill = inspected.skills.values().next().unwrap();
        assert_eq!(skill.1, approved_skill);
        assert!(inspected
            .preview
            .candidates
            .iter()
            .any(|item| item.name == "approved-server"));
        assert!(!inspected
            .preview
            .candidates
            .iter()
            .any(|item| item.name == "substituted-server"));

        fs::rename(&original, &other).unwrap();
        fs::rename(&parked, &original).unwrap();
        let restored = inspect_project_config(&scope, &policy).unwrap();
        assert_eq!(document_digest(&restored).unwrap(), initial);
    }

    #[test]
    fn exclusive_publication_preserves_a_file_created_after_the_destination_check() {
        let private = tempfile::tempdir().unwrap();
        let path = private.path().canonicalize().unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let managed = ManagedRoot::open(&path, Ownership::BatonManaged).unwrap();
        let bytes = b"owned fixture snapshot".to_vec();
        let version = ProjectConfigVersionBinding {
            transaction_id: Uuid::new_v4().to_string(),
            parent_sha256: sha256(b"approved parent"),
            snapshot_sha256: sha256(&bytes),
        };
        let slot = update_slot(&version.parent_sha256);
        let target = path.join(&slot);
        let err = publish_update(
            &managed,
            PreparedUpdate {
                binding: version.clone(),
                source_state_sha256: sha256(b"verified observation"),
                bytes,
            },
            || {
                write_new(
                    &managed.root,
                    slot.to_str().unwrap(),
                    b"unrelated newer occupant",
                )
            },
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::Conflict);
        assert_eq!(fs::read(target).unwrap(), b"unrelated newer occupant");
        assert_eq!(
            observed_snapshot(
                &managed.root,
                &slot,
                &version.transaction_id,
                &version.snapshot_sha256,
                |_| true,
            )
            .unwrap(),
            (ProjectImportState::NeedsReconciliation, true)
        );
    }

    #[test]
    fn recover_real_post_publication_manifest_failure_without_a_second_write() {
        let private = tempfile::tempdir().unwrap();
        let project = private.path().join("source");
        let managed_path = private.path().join("managed");
        for path in [&project, &managed_path] {
            fs::create_dir(path).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let project = project.canonicalize().unwrap();
        let managed_path = managed_path.canonicalize().unwrap();
        let meta = fs::metadata(&project).unwrap();
        let scope = ProjectImportScope {
            project_id: "post-publication-fixture".into(),
            project_revision: 1,
            input_revision: "fixture-input".into(),
            root: project,
            root_device: meta.dev(),
            root_inode: meta.ino(),
        };
        let policy = ProjectImportPolicy {
            revision: "fixture-policy".into(),
            provider: ProviderRevision {
                provider_id: "codex".into(),
                version: "fixture-v1".into(),
            },
            sources: vec![ProjectSource::AgentSkills],
        };
        let proposal = Uuid::new_v4().to_string();
        let request = ProjectImportRequest {
            proposal_id: &proposal,
            scope: &scope,
            policy: &policy,
            managed_project_root: &managed_path,
            choices: &[],
        };
        let (review, bytes) = prepare(&request).unwrap();
        let managed = ManagedRoot::open(&managed_path, Ownership::BatonManaged).unwrap();
        let mut change = managed
            .plan(vec![Edit::nonsecret_replacement(
                SNAPSHOT,
                None,
                bytes.clone(),
            )])
            .unwrap();
        change.id.clone_from(&proposal);
        let journal = managed_path.join(".baton-transactions").join(&proposal);
        let manifest = journal.join("manifest.json");
        let saved = journal.join("previous-manifest");
        let failure = managed
            .apply_observed(change, |index| {
                assert_eq!(index, 0);
                // The snapshot has already been atomically published and synced.
                // Force the OS rename of the final report to fail on a directory.
                fs::rename(&manifest, &saved).unwrap();
                fs::create_dir(&manifest).unwrap();
            })
            .unwrap_err();
        assert_eq!(failure.code, ErrorCode::OutcomeUnknown);
        assert_eq!(failure.transaction_id.as_deref(), Some(proposal.as_str()));
        assert_eq!(fs::read(managed_path.join(SNAPSHOT)).unwrap(), bytes);
        drop(managed);

        let approval = || ConfirmedProjectImport {
            approval_ref: "trusted-fixture-owner",
            binding: &review.binding,
        };
        assert_eq!(
            recover_project_import(&managed_path, &approval()).unwrap(),
            ProjectImportState::NeedsReconciliation
        );
        // Restore the original durable applying report. Recovery can now account
        // for the exact published file without re-running the transaction.
        fs::remove_dir(&manifest).unwrap();
        fs::rename(saved, manifest).unwrap();
        assert_eq!(
            recover_project_import(&managed_path, &approval()).unwrap(),
            ProjectImportState::Applied
        );
        assert_eq!(
            apply_project_import(&request, ProjectImportDecision::Confirm(approval())).unwrap(),
            ProjectImportState::Applied
        );
        assert_eq!(fs::read(managed_path.join(SNAPSHOT)).unwrap(), bytes);
        assert_eq!(
            fs::read_dir(managed_path.join(".baton-transactions"))
                .unwrap()
                .count(),
            1
        );
    }
}
