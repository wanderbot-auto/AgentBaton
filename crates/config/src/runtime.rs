//! Private, deterministic materialization of an already-approved nonsecret plan.
//!
//! This module never reads a native HOME, copies an authentication directory or
//! starts a Provider. Classification/approval belong to the trusted daemon: the
//! input wrappers are an explicit handoff, not a secret detector. Trusted stored
//! plans can be restored with decode_record; deserialization never grants approval.
//! File hashes prove local bytes, not native loading, account or OS isolation.
use crate::{
    error, io_error, open_dir_at, read_at, sha256, write_new, ConfigError, ErrorCode, Ownership,
    MAX_BYTES,
};
use rustix::fs::{self as rfs, AtFlags, FileType, Mode, RenameFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::File,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

type Result<T> = std::result::Result<T, ConfigError>;
const MAX_RUNTIME_FILES: usize = 128;
const MAX_TOTAL_BYTES: usize = 8 * MAX_BYTES;
const MANIFEST: &str = ".baton-runtime-manifest.json";
// Bump when generated/requested semantics change. An old grant must not silently
// approve a newer materializer's interpretation of the same serialized profile.
const MATERIALIZER_REVISION: &str = "agentbaton-managed-runtime-v1";
const CODEX_CONFIG_CONTRACT: &str =
    "d98f7bbe84dfe375d05ad5a8249a5368d82989551e38c6a4babe35356b92c14e";
const CLAUDE_ADAPTER_CONTRACT: &str =
    "e7f0e6d6783c33dbf7c8b5b2a6b8bfd35a0df4909a1667e716a01fbfab320fcd";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeProvider {
    Codex,
    ClaudeAcp,
    Qoder,
    CustomAcp,
}

/// Supplied only after trusted code has classified and approved this exact text.
/// Decode only from a trusted approved plan record; no filesystem-read API exists.
#[derive(Clone, Serialize)]
#[serde(transparent)]
pub struct NonSecretText(String);
impl NonSecretText {
    pub fn approved(value: String) -> Result<Self> {
        if value.len() > MAX_BYTES || value.contains('\0') {
            return Err(error(ErrorCode::ResourceLimit));
        }
        Ok(Self(value))
    }
}
/// Bounded already-selected nonsecret content; never inferred from a source path.
#[derive(Clone, Serialize)]
#[serde(transparent)]
pub struct NonSecretBytes(Vec<u8>);
impl NonSecretBytes {
    pub fn approved(value: Vec<u8>) -> Result<Self> {
        if value.len() > MAX_BYTES {
            return Err(error(ErrorCode::ResourceLimit));
        }
        Ok(Self(value))
    }
}
impl<'de> Deserialize<'de> for NonSecretText {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::approved(value).map_err(|_| serde::de::Error::custom("approved text exceeds bounds"))
    }
}
impl<'de> Deserialize<'de> for NonSecretBytes {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct Bounded;
        impl<'de> serde::de::Visitor<'de> for Bounded {
            type Value = NonSecretBytes;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("bounded approved bytes")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut bytes = Vec::new();
                while let Some(byte) = seq.next_element::<u8>()? {
                    if bytes.len() >= MAX_BYTES {
                        return Err(serde::de::Error::custom("approved bytes exceed bounds"));
                    }
                    bytes.push(byte);
                }
                Ok(NonSecretBytes(bytes))
            }
        }
        deserializer.deserialize_seq(Bounded)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceProvenance {
    /// Opaque registry reference, not a filesystem path or credential value.
    pub source_ref: String,
    pub revision: String,
    pub sha256: String,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CodexSandbox {
    ReadOnly,
    #[default]
    WorkspaceWrite,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CodexApproval {
    Untrusted,
    #[default]
    OnRequest,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodexSettings {
    pub model: Option<NonSecretText>,
    pub sandbox: CodexSandbox,
    pub approval_policy: CodexApproval,
}
impl CodexSettings {
    pub fn model_name(&self) -> Option<&str> {
        self.model.as_ref().map(|model| model.0.as_str())
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaudeSettings {
    /// Native declaration, not permission to connect or invoke any MCP server.
    pub enabled_project_mcp_names: Vec<String>,
    pub disabled_project_mcp_names: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ManagedMcpTransport {
    Stdio {
        command: NonSecretText,
        arguments: Vec<NonSecretText>,
    },
    /// No credentials, headers, query parameters or userinfo are accepted here.
    Http { endpoint: NonSecretText },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMcpServer {
    pub name: String,
    pub transport: ManagedMcpTransport,
    pub provenance: SourceProvenance,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillAsset {
    pub relative_path: PathBuf,
    pub content: NonSecretBytes,
    pub script: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedSkill {
    pub directory_name: String,
    pub instruction: NonSecretText,
    pub assets: Vec<SkillAsset>,
    pub provenance: SourceProvenance,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePlan {
    pub provider: RuntimeProvider,
    pub codex: CodexSettings,
    pub claude: ClaudeSettings,
    pub mcp_servers: Vec<ManagedMcpServer>,
    pub skills: Vec<SelectedSkill>,
    pub source_refs: Vec<SourceProvenance>,
}
impl std::fmt::Debug for RuntimePlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimePlan")
            .field("provider", &self.provider)
            .field("mcp_server_count", &self.mcp_servers.len())
            .field("skill_count", &self.skills.len())
            .field("source_count", &self.source_refs.len())
            .finish_non_exhaustive()
    }
}
impl RuntimePlan {
    /// Stable approved-plan identity, excluding machine paths, staging IDs and inodes.
    pub fn plan_digest(&self) -> Result<String> {
        self.validate()?;
        logical_plan_digest(self)
    }
    pub fn provider(&self) -> RuntimeProvider {
        self.provider
    }
    pub fn validate(&self) -> Result<()> {
        prepare(self.clone(), Path::new("/baton-placeholder-state"))?;
        Ok(())
    }
    /// Read only from the trusted connection registry, never treat JSON as consent.
    pub fn decode_record(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 16 * MAX_BYTES {
            return Err(error(ErrorCode::ResourceLimit));
        }
        let plan: Self = serde_json::from_slice(bytes).map_err(|_| error(ErrorCode::Conflict))?;
        plan.validate()?;
        Ok(plan)
    }
    pub fn empty(provider: RuntimeProvider) -> Self {
        Self {
            provider,
            codex: CodexSettings::default(),
            claude: ClaudeSettings::default(),
            mcp_servers: vec![],
            skills: vec![],
            source_refs: vec![],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    LocallyVerified,
    Unknown,
    Unsupported,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeClaims {
    pub materialized_file_integrity: EvidenceState,
    pub native_runtime_loading: EvidenceState,
    pub no_hidden_native_inheritance: EvidenceState,
    pub native_identity_isolation: EvidenceState,
    pub os_filesystem_sandbox: EvidenceState,
    pub native_settings_schema: EvidenceState,
}
#[derive(Clone, Copy, Default)]
pub struct RuntimeRequirements {
    pub no_hidden_inheritance: bool,
    pub native_identity_isolation: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterializedKind {
    NativeConfig,
    AcpConfiguration,
    SkillInstruction,
    SkillAsset,
    SkillScript,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializedFile {
    pub relative_path: PathBuf,
    pub sha256: String,
    pub bytes: usize,
    pub kind: MaterializedKind,
    pub source_refs: Vec<SourceProvenance>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeManifest {
    pub schema_version: u32,
    pub materializer_revision: String,
    pub plan_digest: String,
    pub provider: RuntimeProvider,
    pub files: Vec<MaterializedFile>,
    pub directories: Vec<PathBuf>,
    /// Their private roots are checked, but runtime-owned contents are excluded
    /// from immutable configuration integrity. Never evidence of OS isolation.
    pub mutable_directories: Vec<PathBuf>,
    pub source_refs: Vec<SourceProvenance>,
    pub claims: RuntimeClaims,
}
/// Persist in the trusted connection/attempt registry, never accept as renderer authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotDescriptor {
    pub id: String,
    pub digest: String,
    pub root_device: u64,
    pub root_inode: u64,
    pub snapshot_device: u64,
    pub snapshot_inode: u64,
}
pub struct RuntimeSnapshot {
    descriptor: SnapshotDescriptor,
    directory: PathBuf,
    manifest: RuntimeManifest,
    acp_configuration: Value,
    codex_settings: Option<CodexSettings>,
}
impl std::fmt::Debug for RuntimeSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeSnapshot")
            .field("id", &self.descriptor.id)
            .field("digest", &self.descriptor.digest)
            .field("provider", &self.manifest.provider)
            .finish_non_exhaustive()
    }
}
impl RuntimeSnapshot {
    pub fn descriptor(&self) -> SnapshotDescriptor {
        self.descriptor.clone()
    }
    /// Actual materialized manifest/file identity; may differ across attempts
    /// when native configuration contains a generated absolute state locator.
    pub fn digest(&self) -> &str {
        &self.descriptor.digest
    }
    pub fn plan_digest(&self) -> &str {
        &self.manifest.plan_digest
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn manifest(&self) -> &RuntimeManifest {
        &self.manifest
    }
    /// Only an explicit environment fragment. Caller must env_clear and separately
    /// authorize any PATH/proxy/trust/credential handles. No account is resolved.
    pub fn environment(&self) -> BTreeMap<OsString, OsString> {
        let mut env = BTreeMap::new();
        for (key, relative) in [
            ("HOME", "state/home"),
            ("TMPDIR", "state/tmp"),
            ("XDG_CONFIG_HOME", "state/xdg/config"),
            ("XDG_CACHE_HOME", "state/xdg/cache"),
            ("XDG_DATA_HOME", "state/xdg/data"),
        ] {
            env.insert(key.into(), self.directory.join(relative).into_os_string());
        }
        match self.manifest.provider {
            RuntimeProvider::Codex => {
                env.insert(
                    "CODEX_HOME".into(),
                    self.directory.join("codex").into_os_string(),
                );
                env.insert(
                    "CODEX_SQLITE_HOME".into(),
                    self.directory.join("state/sqlite").into_os_string(),
                );
            }
            RuntimeProvider::ClaudeAcp => {
                env.insert(
                    "CLAUDE_CONFIG_DIR".into(),
                    self.directory.join("claude").into_os_string(),
                );
            }
            RuntimeProvider::Qoder => {} // Caller uses documented --config-dir, not an invented variable.
            RuntimeProvider::CustomAcp => {}
        }
        env
    }
    pub fn provider_configuration_root(&self) -> PathBuf {
        self.directory
            .join(provider_directory(self.manifest.provider))
    }
    /// Client-owned ACP request data, not a native file-location assumption.
    /// Must still be transmitted by the selected driver and verified against its
    /// actual version/capabilities; configured does not mean connected/invoked.
    pub fn acp_configuration(&self) -> Result<Value> {
        Ok(self.acp_configuration.clone())
    }
    pub fn claude_setting_sources_override(&self) -> Option<&'static [&'static str]> {
        (self.manifest.provider == RuntimeProvider::ClaudeAcp).then_some(&[])
    }
    pub fn codex_settings(&self) -> Option<&CodexSettings> {
        self.codex_settings.as_ref()
    }
}

pub struct RuntimeRoot {
    directory: PathBuf,
    root: File,
    identity: (u64, u64),
}
impl RuntimeRoot {
    pub fn open(directory: &Path) -> Result<Self> {
        if !directory.is_absolute() || io_error(directory.canonicalize())? != directory {
            return Err(error(ErrorCode::InvalidPath));
        }
        let root = open_dir_at(rfs::CWD, directory)?;
        let meta = io_error(root.metadata())?;
        Ok(Self {
            directory: directory.into(),
            root,
            identity: (meta.dev(), meta.ino()),
        })
    }
    fn verify_root(&self) -> Result<()> {
        let current = Self::open(&self.directory)?;
        if current.identity != self.identity {
            return Err(error(ErrorCode::Conflict));
        }
        Ok(())
    }
    pub fn materialize(&self, plan: RuntimePlan, ownership: Ownership) -> Result<RuntimeSnapshot> {
        if ownership != Ownership::BatonManaged {
            return Err(error(ErrorCode::PermissionDenied));
        }
        self.verify_root()?;
        let id = Uuid::new_v4().to_string();
        let prepared = prepare(plan, &self.directory.join(&id).join("state"))?;
        let stage = format!(".baton-stage-{id}");
        io_error(rfs::mkdirat(
            &self.root,
            stage.as_str(),
            Mode::from_raw_mode(0o700),
        ))?;
        let result = (|| -> Result<RuntimeSnapshot> {
            let directory = open_dir_at(&self.root, Path::new(&stage))?;
            // Failed staging is retained privately for explicit recovery, never
            // recursively removed through untrusted paths or published as complete.
            for path in &prepared.manifest.directories {
                let (parent, name) = parent_and_name(&directory, path)?;
                io_error(rfs::mkdirat(
                    &parent,
                    name.as_str(),
                    Mode::from_raw_mode(0o700),
                ))?;
                io_error(parent.sync_all())?;
            }
            for (path, bytes) in prepared.files {
                let (parent, name) = parent_and_name(&directory, &path)?;
                write_new(&parent, &name, &bytes)?;
            }
            let manifest_bytes =
                serde_json::to_vec(&prepared.manifest).map_err(|_| error(ErrorCode::Conflict))?;
            if manifest_bytes.len() > MAX_BYTES {
                return Err(error(ErrorCode::ResourceLimit));
            }
            write_new(&directory, MANIFEST, &manifest_bytes)?;
            let meta = io_error(directory.metadata())?;
            self.verify_root()?;
            io_error(rfs::renameat_with(
                &self.root,
                stage.as_str(),
                &self.root,
                id.as_str(),
                RenameFlags::NOREPLACE,
            ))?;
            self.root.sync_all().map_err(|_| ConfigError {
                code: ErrorCode::OutcomeUnknown,
                transaction_id: Some(id.clone()),
            })?;
            let descriptor = SnapshotDescriptor {
                id: id.clone(),
                digest: sha256(&manifest_bytes),
                root_device: self.identity.0,
                root_inode: self.identity.1,
                snapshot_device: meta.dev(),
                snapshot_inode: meta.ino(),
            };
            self.verify_before_launch(&descriptor, RuntimeRequirements::default())
        })();
        result.map_err(|mut failure| {
            if failure.transaction_id.is_none() {
                failure.transaction_id = Some(id);
            }
            failure
        })
    }

    pub fn verify_before_launch(
        &self,
        descriptor: &SnapshotDescriptor,
        requirements: RuntimeRequirements,
    ) -> Result<RuntimeSnapshot> {
        if Uuid::parse_str(&descriptor.id).is_err()
            || descriptor.id.len() != 36
            || !valid_digest(&descriptor.digest)
        {
            return Err(error(ErrorCode::InvalidPath));
        }
        if requirements.no_hidden_inheritance || requirements.native_identity_isolation {
            return Err(error(ErrorCode::PermissionDenied));
        }
        self.verify_root()?;
        if (descriptor.root_device, descriptor.root_inode) != self.identity {
            return Err(error(ErrorCode::Conflict));
        }
        let dir = open_dir_at(&self.root, Path::new(&descriptor.id))?;
        let meta = io_error(dir.metadata())?;
        if (meta.dev(), meta.ino()) != (descriptor.snapshot_device, descriptor.snapshot_inode) {
            return Err(error(ErrorCode::Conflict));
        }
        let manifest_bytes =
            read_at(&dir, Path::new(MANIFEST))?.ok_or_else(|| error(ErrorCode::Conflict))?;
        if sha256(&manifest_bytes) != descriptor.digest {
            return Err(error(ErrorCode::Conflict));
        }
        let manifest: RuntimeManifest =
            serde_json::from_slice(&manifest_bytes).map_err(|_| error(ErrorCode::Conflict))?;
        validate_manifest(&manifest)?;
        verify_tree(&dir, &manifest)?;
        let acp_parent = open_dir_at(&dir, Path::new("acp"))?;
        let acp_bytes = read_at(&acp_parent, Path::new("configuration.json"))?
            .ok_or_else(|| error(ErrorCode::Conflict))?;
        if manifest
            .files
            .iter()
            .find(|entry| entry.relative_path == Path::new("acp/configuration.json"))
            .is_none_or(|entry| entry.sha256 != sha256(&acp_bytes))
        {
            return Err(error(ErrorCode::Conflict));
        }
        let acp_configuration =
            serde_json::from_slice(&acp_bytes).map_err(|_| error(ErrorCode::Conflict))?;
        let codex_settings = if manifest.provider == RuntimeProvider::Codex {
            #[derive(Deserialize)]
            struct GeneratedCodex {
                model: Option<NonSecretText>,
                sandbox_mode: CodexSandbox,
                approval_policy: CodexApproval,
            }
            let parent = open_dir_at(&dir, Path::new("codex"))?;
            let bytes = read_at(&parent, Path::new("config.toml"))?
                .ok_or_else(|| error(ErrorCode::Conflict))?;
            if manifest
                .files
                .iter()
                .find(|entry| entry.relative_path == Path::new("codex/config.toml"))
                .is_none_or(|entry| entry.sha256 != sha256(&bytes))
            {
                return Err(error(ErrorCode::Conflict));
            }
            let content = std::str::from_utf8(&bytes).map_err(|_| error(ErrorCode::Conflict))?;
            let settings: GeneratedCodex =
                toml::from_str(content).map_err(|_| error(ErrorCode::Conflict))?;
            Some(CodexSettings {
                model: settings.model,
                sandbox: settings.sandbox_mode,
                approval_policy: settings.approval_policy,
            })
        } else {
            None
        };
        self.verify_root()?;
        Ok(RuntimeSnapshot {
            descriptor: descriptor.clone(),
            directory: self.directory.join(&descriptor.id),
            manifest,
            acp_configuration,
            codex_settings,
        })
    }
}

struct Prepared {
    manifest: RuntimeManifest,
    files: BTreeMap<PathBuf, Vec<u8>>,
}
fn provider_directory(provider: RuntimeProvider) -> &'static str {
    match provider {
        RuntimeProvider::Codex => "codex",
        RuntimeProvider::ClaudeAcp => "claude",
        RuntimeProvider::Qoder => "qoder",
        RuntimeProvider::CustomAcp => "custom",
    }
}
fn valid_digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-.:".contains(&b))
}
fn leaf(s: &str) -> bool {
    identifier(s) && s != "." && s != ".." && !s.starts_with(".baton-")
}
fn provenance(p: &SourceProvenance) -> Result<()> {
    if !identifier(&p.source_ref) || !identifier(&p.revision) || !valid_digest(&p.sha256) {
        return Err(error(ErrorCode::InvalidPath));
    }
    Ok(())
}
fn relative(path: &Path) -> Result<()> {
    if path.as_os_str().len() > 1024
        || path.components().count() > 8
        || path
            .components()
            .any(|c| !matches!(c,Component::Normal(s) if s.to_str().is_some_and(leaf)))
        || path.as_os_str().is_empty()
    {
        return Err(error(ErrorCode::InvalidPath));
    }
    Ok(())
}
fn parent_and_name(dir: &File, path: &Path) -> Result<(File, String)> {
    relative(path)?;
    let mut parent = io_error(dir.try_clone())?;
    let components: Vec<_> = path.components().collect();
    for part in &components[..components.len() - 1] {
        parent = open_dir_at(&parent, Path::new(part.as_os_str()))?;
    }
    let name = components
        .last()
        .and_then(|p| p.as_os_str().to_str())
        .ok_or_else(|| error(ErrorCode::InvalidPath))?
        .to_string();
    Ok((parent, name))
}
fn logical_plan_digest(plan: &RuntimePlan) -> Result<String> {
    let contract = match plan.provider {
        RuntimeProvider::Codex => Some(CODEX_CONFIG_CONTRACT),
        RuntimeProvider::ClaudeAcp => Some(CLAUDE_ADAPTER_CONTRACT),
        RuntimeProvider::CustomAcp | RuntimeProvider::Qoder => None,
    };
    #[derive(Serialize)]
    struct BoundPlan<'a> {
        materializer_revision: &'static str,
        native_contract_sha256: Option<&'static str>,
        plan: &'a RuntimePlan,
    }
    let bytes = serde_json::to_vec(&BoundPlan {
        materializer_revision: MATERIALIZER_REVISION,
        native_contract_sha256: contract,
        plan,
    })
    .map_err(|_| error(ErrorCode::Conflict))?;
    if bytes.len() > 16 * MAX_BYTES {
        return Err(error(ErrorCode::ResourceLimit));
    }
    Ok(sha256(&bytes))
}
fn prepare(plan: RuntimePlan, state: &Path) -> Result<Prepared> {
    if plan.skills.len() > 32 || plan.mcp_servers.len() > 32 || plan.source_refs.len() > 128 {
        return Err(error(ErrorCode::ResourceLimit));
    }
    if plan.provider == RuntimeProvider::Qoder
        && (!plan.skills.is_empty() || !plan.mcp_servers.is_empty())
    {
        return Err(error(ErrorCode::Conflict));
    }
    if plan.provider == RuntimeProvider::CustomAcp && !plan.skills.is_empty() {
        return Err(error(ErrorCode::Conflict));
    }
    if plan.provider != RuntimeProvider::Codex
        && serde_json::to_value(&plan.codex).map_err(|_| error(ErrorCode::Conflict))?
            != serde_json::to_value(CodexSettings::default())
                .map_err(|_| error(ErrorCode::Conflict))?
    {
        return Err(error(ErrorCode::Conflict));
    }
    if plan.provider != RuntimeProvider::ClaudeAcp
        && (!plan.claude.enabled_project_mcp_names.is_empty()
            || !plan.claude.disabled_project_mcp_names.is_empty())
    {
        return Err(error(ErrorCode::Conflict));
    }
    let plan_digest = logical_plan_digest(&plan)?;
    let mut sources = plan.source_refs;
    for server in &plan.mcp_servers {
        sources.push(server.provenance.clone());
    }
    for skill in &plan.skills {
        sources.push(skill.provenance.clone());
    }
    for source in &sources {
        provenance(source)?;
    }
    sources.sort_by(|a, b| {
        (&a.source_ref, &a.revision, &a.sha256).cmp(&(&b.source_ref, &b.revision, &b.sha256))
    });
    sources.dedup();
    let mut file_data: BTreeMap<PathBuf, (Vec<u8>, MaterializedKind, Vec<SourceProvenance>)> =
        BTreeMap::new();
    let mut servers = BTreeMap::new();
    let mut acp_servers = vec![];
    for server in plan.mcp_servers {
        if !leaf(&server.name) || servers.contains_key(&server.name) {
            return Err(error(ErrorCode::Conflict));
        }
        let (native, acp) = match server.transport {
            ManagedMcpTransport::Stdio { command, arguments } => {
                if command.0.trim().is_empty()
                    || command.0.len() > 4096
                    || arguments.len() > 128
                    || arguments.iter().any(|v| v.0.len() > 4096)
                {
                    return Err(error(ErrorCode::ResourceLimit));
                }
                let args: Vec<_> = arguments.into_iter().map(|v| v.0).collect();
                (
                    json!({"command":command.0,"args":args}),
                    json!({"name":server.name,"command":command.0,"args":args,"env":[]}),
                )
            }
            ManagedMcpTransport::Http { endpoint } => {
                let host = endpoint
                    .0
                    .strip_prefix("https://")
                    .unwrap_or("")
                    .split('/')
                    .next()
                    .unwrap_or("");
                if !endpoint.0.starts_with("https://")
                    || endpoint.0.contains(['@', '?', '#', '\n', '\r'])
                    || endpoint.0.len() > 4096
                    || host.is_empty()
                    || endpoint.0.chars().any(char::is_whitespace)
                    || endpoint.0.chars().any(char::is_control)
                {
                    return Err(error(ErrorCode::PermissionDenied));
                }
                (
                    json!({"url":endpoint.0}),
                    json!({"type":"http","name":server.name,"url":endpoint.0,"headers":[]}),
                )
            }
        };
        servers.insert(server.name, native);
        acp_servers.push(acp);
    }
    acp_servers.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    let mut acp = json!({"mcpServers":acp_servers});
    match plan.provider {
        RuntimeProvider::Codex => {
            if plan.codex.model.as_ref().is_some_and(|m| !identifier(&m.0)) {
                return Err(error(ErrorCode::InvalidPath));
            }
            let mut doc = json!({"sandbox_mode":plan.codex.sandbox,"approval_policy":plan.codex.approval_policy,"mcp_servers":servers,"cli_auth_credentials_store":"file","mcp_oauth_credentials_store":"file","log_dir":state.join("log")});
            if let Some(model) = plan.codex.model {
                doc["model"] = json!(model.0);
            }
            let content = toml::to_string(&doc)
                .map_err(|_| error(ErrorCode::Conflict))?
                .into_bytes();
            file_data.insert(
                "codex/config.toml".into(),
                (content, MaterializedKind::NativeConfig, sources.clone()),
            );
        }
        RuntimeProvider::ClaudeAcp => {
            let enabled = plan.claude.enabled_project_mcp_names;
            let disabled = plan.claude.disabled_project_mcp_names;
            if enabled.len() > 128
                || disabled.len() > 128
                || enabled.iter().chain(&disabled).any(|name| !leaf(name))
                || enabled.iter().any(|name| disabled.contains(name))
            {
                return Err(error(ErrorCode::Conflict));
            }
            let mut enabled = enabled;
            let mut disabled = disabled;
            enabled.sort();
            enabled.dedup();
            disabled.sort();
            disabled.dedup();
            let doc = json!({"enableAllProjectMcpServers":false,"enabledMcpjsonServers":enabled,"disabledMcpjsonServers":disabled});
            // Pinned adapter accepts a programmatic settings object; without it,
            // settingSources=[] would also exclude our generated settings file.
            // These are requested options, not a runtime enforcement claim.
            acp["_meta"] = json!({"claudeCode":{"options":{
                "settingSources":[],"settings":doc,
                "allowDangerouslySkipPermissions":false,"strictMcpConfig":true
            }}});
            file_data.insert(
                "claude/settings.json".into(),
                (
                    serde_json::to_vec(&doc).map_err(|_| error(ErrorCode::Conflict))?,
                    MaterializedKind::NativeConfig,
                    sources.clone(),
                ),
            );
        }
        RuntimeProvider::Qoder | RuntimeProvider::CustomAcp => {}
    }
    file_data.insert(
        "acp/configuration.json".into(),
        (
            serde_json::to_vec(&acp).map_err(|_| error(ErrorCode::Conflict))?,
            MaterializedKind::AcpConfiguration,
            sources.clone(),
        ),
    );
    let mut skill_names = BTreeSet::new();
    for skill in plan.skills {
        if !leaf(&skill.directory_name)
            || !skill_names.insert(skill.directory_name.clone())
            || skill.assets.len() > 64
        {
            return Err(error(ErrorCode::Conflict));
        }
        let prefix = Path::new(provider_directory(plan.provider))
            .join("skills")
            .join(&skill.directory_name);
        file_data.insert(
            prefix.join("SKILL.md"),
            (
                skill.instruction.0.into_bytes(),
                MaterializedKind::SkillInstruction,
                vec![skill.provenance.clone()],
            ),
        );
        for asset in skill.assets {
            relative(&asset.relative_path)?;
            if asset.relative_path == Path::new("SKILL.md") {
                return Err(error(ErrorCode::Conflict));
            }
            let path = prefix.join(asset.relative_path);
            let kind = if asset.script {
                MaterializedKind::SkillScript
            } else {
                MaterializedKind::SkillAsset
            };
            if file_data
                .insert(
                    path,
                    (asset.content.0, kind, vec![skill.provenance.clone()]),
                )
                .is_some()
            {
                return Err(error(ErrorCode::Conflict));
            }
            if file_data.len() > MAX_RUNTIME_FILES
                || file_data
                    .values()
                    .map(|(bytes, _, _)| bytes.len())
                    .sum::<usize>()
                    > MAX_TOTAL_BYTES
            {
                return Err(error(ErrorCode::ResourceLimit));
            }
        }
    }
    if file_data.len() > MAX_RUNTIME_FILES
        || file_data
            .values()
            .map(|(bytes, _, _)| bytes.len())
            .sum::<usize>()
            > MAX_TOTAL_BYTES
    {
        return Err(error(ErrorCode::ResourceLimit));
    }
    let mutable_directories: Vec<PathBuf> = [
        "state/home",
        "state/tmp",
        "state/xdg/config",
        "state/xdg/cache",
        "state/xdg/data",
        "state/log",
        "state/sqlite",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    let mut dirs: BTreeSet<PathBuf> = [
        "state",
        "state/home",
        "state/tmp",
        "state/xdg",
        "state/xdg/config",
        "state/xdg/cache",
        "state/xdg/data",
        "state/log",
        "state/sqlite",
        "acp",
        provider_directory(plan.provider),
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    let mut entries = vec![];
    let mut files = BTreeMap::new();
    for (path, (bytes, kind, source_refs)) in file_data {
        relative(&path)?;
        if bytes.len() > MAX_BYTES {
            return Err(error(ErrorCode::ResourceLimit));
        }
        let mut parent = path.parent();
        while let Some(p) = parent {
            if p.as_os_str().is_empty() {
                break;
            }
            dirs.insert(p.into());
            parent = p.parent();
        }
        entries.push(MaterializedFile {
            relative_path: path.clone(),
            sha256: sha256(&bytes),
            bytes: bytes.len(),
            kind,
            source_refs,
        });
        files.insert(path, bytes);
    }
    let claims = RuntimeClaims {
        materialized_file_integrity: EvidenceState::LocallyVerified,
        native_runtime_loading: EvidenceState::Unknown,
        no_hidden_native_inheritance: EvidenceState::Unknown,
        native_identity_isolation: EvidenceState::Unknown,
        os_filesystem_sandbox: EvidenceState::Unsupported,
        native_settings_schema: if plan.provider == RuntimeProvider::Qoder {
            EvidenceState::Unsupported
        } else {
            EvidenceState::Unknown
        },
    };
    let manifest = RuntimeManifest {
        schema_version: 1,
        materializer_revision: MATERIALIZER_REVISION.into(),
        plan_digest,
        provider: plan.provider,
        files: entries,
        directories: dirs.into_iter().collect(),
        mutable_directories,
        source_refs: sources,
        claims,
    };
    validate_manifest(&manifest)?;
    Ok(Prepared { manifest, files })
}
fn validate_manifest(manifest: &RuntimeManifest) -> Result<()> {
    if !valid_digest(&manifest.plan_digest)
        || manifest.schema_version != 1
        || manifest.materializer_revision != MATERIALIZER_REVISION
        || manifest.files.is_empty()
        || manifest.files.len() > MAX_RUNTIME_FILES
        || manifest.directories.len() > 512
        || manifest.source_refs.len() > 192
    {
        return Err(error(ErrorCode::ResourceLimit));
    }
    let mut mutable = BTreeSet::new();
    for path in &manifest.mutable_directories {
        relative(path)?;
        if !path.starts_with("state")
            || !manifest.directories.contains(path)
            || !mutable.insert(path)
        {
            return Err(error(ErrorCode::Conflict));
        }
    }
    let mut names = BTreeSet::new();
    let mut total = 0usize;
    for path in &manifest.directories {
        relative(path)?;
        if !names.insert(path) {
            return Err(error(ErrorCode::Conflict));
        }
    }
    for file in &manifest.files {
        relative(&file.relative_path)?;
        if manifest
            .mutable_directories
            .iter()
            .any(|p| file.relative_path.starts_with(p))
        {
            return Err(error(ErrorCode::Conflict));
        }
        if !valid_digest(&file.sha256)
            || file.bytes > MAX_BYTES
            || !names.insert(&file.relative_path)
        {
            return Err(error(ErrorCode::Conflict));
        }
        for source in &file.source_refs {
            provenance(source)?;
            if !manifest.source_refs.contains(source) {
                return Err(error(ErrorCode::Conflict));
            }
        }
        total = total
            .checked_add(file.bytes)
            .ok_or_else(|| error(ErrorCode::ResourceLimit))?;
    }
    if total > MAX_TOTAL_BYTES {
        return Err(error(ErrorCode::ResourceLimit));
    }
    for source in &manifest.source_refs {
        provenance(source)?;
    }
    Ok(())
}
fn verify_tree(dir: &File, manifest: &RuntimeManifest) -> Result<()> {
    let expected_dirs: BTreeSet<_> = manifest.directories.iter().cloned().collect();
    let expected_files: BTreeMap<_, _> = manifest
        .files
        .iter()
        .map(|f| (f.relative_path.clone(), f))
        .collect();
    let mut pending = vec![(PathBuf::new(), io_error(dir.try_clone())?)];
    let mut observed = 0usize;
    let mut seen_dirs = BTreeSet::new();
    let mut seen_files = BTreeSet::new();
    while let Some((relative_dir, directory)) = pending.pop() {
        let entries = io_error(rfs::Dir::read_from(&directory))?;
        for entry in entries {
            let entry = io_error(entry)?;
            let name = entry.file_name().to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            observed += 1;
            if observed > MAX_RUNTIME_FILES + 512 + 1 {
                return Err(error(ErrorCode::ResourceLimit));
            }
            let name = std::str::from_utf8(name).map_err(|_| error(ErrorCode::InvalidPath))?;
            if relative_dir.as_os_str().is_empty() && name == MANIFEST {
                continue;
            }
            let path = relative_dir.join(name);
            let stat = io_error(rfs::statat(&directory, name, AtFlags::SYMLINK_NOFOLLOW))?;
            match FileType::from_raw_mode(stat.st_mode) {
                FileType::Directory if expected_dirs.contains(&path) => {
                    let child = open_dir_at(&directory, Path::new(name))?;
                    seen_dirs.insert(path.clone());
                    if !manifest.mutable_directories.contains(&path) {
                        pending.push((path, child));
                    }
                }
                FileType::RegularFile if expected_files.contains_key(&path) => {
                    let bytes = read_at(&directory, Path::new(name))?
                        .ok_or_else(|| error(ErrorCode::Conflict))?;
                    let expected = expected_files[&path];
                    if bytes.len() != expected.bytes || sha256(&bytes) != expected.sha256 {
                        return Err(error(ErrorCode::Conflict));
                    }
                    seen_files.insert(path);
                }
                _ => return Err(error(ErrorCode::Conflict)),
            }
        }
    }
    if seen_dirs != expected_dirs || seen_files.len() != expected_files.len() {
        return Err(error(ErrorCode::Conflict));
    }
    Ok(())
}
