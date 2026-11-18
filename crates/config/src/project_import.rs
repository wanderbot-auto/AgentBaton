//! Read-only project configuration review and one confirmed managed snapshot.
//! Project source files do not confer authority. The trusted caller resolves
//! project identity, current input revisions, non-secret text classification,
//! eligible registered MCP definitions, and the original approval.
use crate::{
    capabilities::{
        native::{
            DiscoveryLimits, InstructionOnlySkillMetadata, NativeFormat, NativeSourceRoot,
            NativeTransport, ParseStatus, SkillDialect,
        },
        ProviderRevision, SourceDescriptor, SourceKind,
    },
    error, io_error, load_report, open_dir_at, open_private, read_at,
    runtime::{
        ManagedMcpServer, ManagedMcpTransport, NonSecretText, RuntimePlan, RuntimeProvider,
        SelectedSkill, SourceProvenance,
    },
    sha256, Edit, ErrorCode, FileState, LockGuard, ManagedRoot, Phase, Result, MAX_BYTES,
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

#[derive(Clone, Debug, Serialize)]
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
                    for skill in discovery.skills {
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
                            skill.source_digest,
                            admission,
                        ));
                    }
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
    if request
        .managed_project_root
        .starts_with(&request.scope.root)
        || request.scope.root.starts_with(request.managed_project_root)
    {
        return Err(error(ErrorCode::PermissionDenied));
    }
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

fn observed_state(
    root: &File,
    binding: &ProjectImportBinding,
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
    let current = match read_at(root, Path::new(SNAPSHOT)) {
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
        match rfs::statat(
            &journals,
            binding.proposal_id.as_str(),
            AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Ok(_) => {}
            Err(_) => return Err(error(ErrorCode::Io)),
        }
        let journal = open_dir_at(&journals, Path::new(&binding.proposal_id))?;
        load_report(&journal, &binding.proposal_id).map(Some)
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
        || file.relative_path != Path::new(SNAPSHOT)
        || file.before_sha256.is_some()
        || file.after_sha256 != binding.snapshot_sha256
        || matches!(
            report.phase,
            Phase::RollingBack | Phase::RolledBack | Phase::Conflict
        )
    {
        return Ok((State::NeedsReconciliation, true));
    }
    match current.as_deref() {
        Some(bytes) if sha256(bytes) == binding.snapshot_sha256 => {
            #[derive(Deserialize)]
            struct Header {
                schema_version: u32,
                proposal_id: String,
                scope: ProjectImportScope,
            }
            if !serde_json::from_slice::<Header>(bytes).is_ok_and(|header| {
                header.schema_version == VERSION
                    && header.proposal_id == binding.proposal_id
                    && header.scope.project_id == binding.project_id
            }) {
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
    #[derive(Deserialize)]
    struct StoredPlan {
        schema_version: u32,
        proposal_id: String,
        scope: ProjectImportScope,
        runtime_plan_sha256: String,
        runtime_plan: RuntimePlan,
    }
    let root = approved_target(path, approval)?;
    match observed_state(&root, approval.binding)?.0 {
        ProjectImportState::Applied => {}
        ProjectImportState::NotApplied => return Ok(None),
        _ => return Err(error(ErrorCode::OutcomeUnknown)),
    }
    let bytes = read_at(&root, Path::new(SNAPSHOT))?.ok_or_else(|| error(ErrorCode::Conflict))?;
    if sha256(&bytes) != approval.binding.snapshot_sha256 {
        return Err(error(ErrorCode::Conflict));
    }
    let plan: StoredPlan = io_error(serde_json::from_slice(&bytes))?;
    if plan.schema_version != VERSION
        || plan.proposal_id != approval.binding.proposal_id
        || plan.scope.project_id != approval.binding.project_id
        || plan.runtime_plan.plan_digest()? != plan.runtime_plan_sha256
    {
        return Err(error(ErrorCode::Conflict));
    }
    plan.runtime_plan.validate()?;
    Ok(Some(plan.runtime_plan))
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
