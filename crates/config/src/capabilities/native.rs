//! Explicit, read-only adapters for documented native source shapes. Parsing is
//! static evidence only: no executable, environment expansion, hooks or MCP call
//! is ever invoked. Private original values are not exposed by safe previews.
use super::{validate_descriptor, SourceDescriptor};
use crate::{error, io_error, sha256, validate_relative, ConfigError, ErrorCode, MAX_BYTES};
use rustix::fs::{self as rfs, AtFlags, FileType, Mode, OFlags};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs::File,
    io::Read,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, ConfigError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeFormat {
    CodexToml,
    ClaudeMcpJson,
    ClaudeSettingsJson,
    QoderSettingsUnknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseStatus {
    Parsed,
    Partial,
    Malformed,
    Unsupported,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    pub code: String,
    pub item_ref: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeTransport {
    Stdio,
    Http,
    Sse,
    WebSocket,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeMcpPreview {
    pub name: String,
    pub transport: NativeTransport,
    pub configured: bool,
    pub enabled_declaration: Option<bool>,
    pub executable_ref: Option<String>,
    pub arguments_ref: Option<String>,
    pub argument_count: usize,
    pub endpoint_ref: Option<String>,
    pub environment_names: Vec<String>,
    pub header_names: Vec<String>,
    pub credential_refs: Vec<String>,
    pub inline_values_present: bool,
    pub unknown_field_names: Vec<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaudeSettingsPreview {
    pub enable_all_project_mcp_servers: Option<bool>,
    pub enabled_project_mcp_names: Vec<String>,
    pub disabled_project_mcp_names: Vec<String>,
    pub hook_event_names: Vec<String>,
    pub environment_names: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativePreview {
    pub source: SourceDescriptor,
    pub source_digest: String,
    pub format: NativeFormat,
    pub status: ParseStatus,
    pub mcp_servers: Vec<NativeMcpPreview>,
    pub settings: Option<ClaudeSettingsPreview>,
    pub unknown_field_names: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
    pub runtime_evidence_observed: bool,
}
/// Root grant belongs to the trusted host registry. Read-only native roots/files
/// may be owner-readable 0755/0644, but group/other writable sources are rejected.
/// No write method or automatic personal/global-directory traversal exists.
pub struct NativeSourceRoot {
    root: File,
    identity: (u64, u64),
}
impl NativeSourceRoot {
    pub fn open_selected(path: &Path) -> Result<Self> {
        if !path.is_absolute() || io_error(path.canonicalize())? != path {
            return Err(error(ErrorCode::InvalidPath));
        }
        let root = readonly_dir(rfs::CWD, path)?;
        let metadata = io_error(root.metadata())?;
        Ok(Self {
            root,
            identity: (metadata.dev(), metadata.ino()),
        })
    }
    pub fn read_native(
        &self,
        source: SourceDescriptor,
        relative: &Path,
        format: NativeFormat,
    ) -> Result<NativeSource> {
        validate_descriptor(&source)?;
        let provider_matches = match format {
            NativeFormat::CodexToml => source.provider.provider_id == "codex",
            NativeFormat::ClaudeMcpJson | NativeFormat::ClaudeSettingsJson => matches!(
                source.provider.provider_id.as_str(),
                "claude" | "claude-code"
            ),
            NativeFormat::QoderSettingsUnknown => source.provider.provider_id == "qoder",
        };
        if !provider_matches {
            return Err(error(ErrorCode::Conflict));
        }
        let raw = self.read(relative)?;
        let digest = sha256(&raw);
        let mut preview = NativePreview {
            source,
            source_digest: digest,
            format,
            status: ParseStatus::Parsed,
            mcp_servers: Vec::new(),
            settings: None,
            unknown_field_names: Vec::new(),
            diagnostics: Vec::new(),
            runtime_evidence_observed: false,
        };
        if format == NativeFormat::QoderSettingsUnknown {
            preview.status = ParseStatus::Unsupported;
            diagnose(&mut preview, "QODER_JSON_SCHEMA_NOT_CONFIRMED", None);
            return Ok(NativeSource {
                raw,
                relative: relative.to_owned(),
                identity: self.identity,
                preview,
            });
        }
        let parsed = match format {
            NativeFormat::CodexToml => std::str::from_utf8(&raw)
                .ok()
                .and_then(|text| toml::from_str::<toml::Value>(text).ok())
                .and_then(|value| serde_json::to_value(value).ok()),
            _ => serde_json::from_slice::<Value>(&raw).ok(),
        };
        if let Some(document) = parsed.as_ref().and_then(Value::as_object) {
            match format {
                NativeFormat::CodexToml => {
                    parse_mcp_document(document, "mcp_servers", &mut preview)
                }
                NativeFormat::ClaudeMcpJson => {
                    parse_mcp_document(document, "mcpServers", &mut preview)
                }
                NativeFormat::ClaudeSettingsJson => parse_claude_settings(document, &mut preview),
                NativeFormat::QoderSettingsUnknown => unreachable!(),
            }
        } else {
            preview.status = ParseStatus::Malformed;
            diagnose(&mut preview, "MALFORMED_NATIVE_DOCUMENT", None);
        }
        Ok(NativeSource {
            raw,
            relative: relative.to_owned(),
            identity: self.identity,
            preview,
        })
    }
    pub fn verify_unchanged(&self, source: &NativeSource) -> Result<bool> {
        if source.identity != self.identity {
            return Err(error(ErrorCode::Conflict));
        }
        Ok(sha256(&self.read(&source.relative)?) == source.preview.source_digest)
    }
    fn parent(&self, path: &Path) -> Result<(File, PathBuf)> {
        validate_relative(path)?;
        let parts: Vec<_> = path.components().collect();
        let mut parent = io_error(self.root.try_clone())?;
        for part in &parts[..parts.len() - 1] {
            parent = readonly_dir(&parent, Path::new(part.as_os_str()))?;
        }
        Ok((parent, PathBuf::from(parts[parts.len() - 1].as_os_str())))
    }
    fn read(&self, path: &Path) -> Result<Vec<u8>> {
        let (parent, name) = self.parent(path)?;
        let filename = name.to_string_lossy();
        if matches!(
            filename.as_ref(),
            "auth.json" | "credentials.json" | ".credentials.json" | "keychain.json"
        ) {
            return Err(error(ErrorCode::PermissionDenied));
        }
        read_file(&parent, &name)
    }
    fn directory(&self, path: &Path) -> Result<File> {
        let (parent, name) = self.parent(path)?;
        readonly_dir(&parent, &name)
    }
}
pub struct NativeSource {
    raw: Vec<u8>,
    relative: PathBuf,
    identity: (u64, u64),
    preview: NativePreview,
}
impl NativeSource {
    pub fn preview(&self) -> NativePreview {
        self.preview.clone()
    }
    pub fn preserves_original_hash(&self) -> bool {
        sha256(&self.raw) == self.preview.source_digest
    }
}
fn diagnose(preview: &mut NativePreview, code: &str, item: Option<String>) {
    preview.diagnostics.push(Diagnostic {
        code: code.into(),
        item_ref: item,
    });
}
fn parse_mcp_document(document: &Map<String, Value>, key: &str, preview: &mut NativePreview) {
    preview.unknown_field_names = document
        .keys()
        .filter(|name| name.as_str() != key)
        .cloned()
        .collect();
    let Some(servers) = document.get(key) else {
        return;
    };
    let Some(servers) = servers.as_object() else {
        preview.status = ParseStatus::Malformed;
        diagnose(preview, "MCP_TABLE_MUST_BE_OBJECT", None);
        return;
    };
    if servers.len() > 256 {
        preview.status = ParseStatus::Unsupported;
        diagnose(preview, "MCP_SERVER_LIMIT", None);
        return;
    }
    for (name, value) in servers {
        match parse_server(name, value, preview) {
            Ok(server) => preview.mcp_servers.push(server),
            Err(code) => {
                preview.status = ParseStatus::Partial;
                diagnose(
                    preview,
                    code,
                    Some(opaque_ref(&preview.source_digest, name)),
                );
            }
        }
    }
}
fn parse_server(
    name: &str,
    value: &Value,
    preview: &NativePreview,
) -> std::result::Result<NativeMcpPreview, &'static str> {
    if name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err("INVALID_MCP_SERVER_NAME");
    }
    let table = value.as_object().ok_or("MCP_SERVER_MUST_BE_OBJECT")?;
    let codex = preview.format == NativeFormat::CodexToml;
    let command = table.get("command");
    let url = table.get("url");
    let transport = if codex {
        match (command, url) {
            (Some(_), None) => NativeTransport::Stdio,
            (None, Some(_)) => NativeTransport::Http,
            _ => return Err("AMBIGUOUS_MCP_TRANSPORT"),
        }
    } else {
        let declared_type = match table.get("type") {
            None => "stdio",
            Some(Value::String(kind)) => kind.as_str(),
            Some(_) => return Err("INVALID_MCP_TYPE"),
        };
        match declared_type {
            "stdio" if url.is_none() => NativeTransport::Stdio,
            "http" | "streamable-http" if command.is_none() => NativeTransport::Http,
            "sse" if command.is_none() => NativeTransport::Sse,
            "ws" if command.is_none() => NativeTransport::WebSocket,
            _ => return Err("UNSUPPORTED_OR_AMBIGUOUS_MCP_TRANSPORT"),
        }
    };
    let mut server = NativeMcpPreview {
        name: name.into(),
        transport,
        configured: true,
        enabled_declaration: None,
        executable_ref: None,
        arguments_ref: None,
        argument_count: 0,
        endpoint_ref: None,
        environment_names: Vec::new(),
        header_names: Vec::new(),
        credential_refs: Vec::new(),
        inline_values_present: false,
        unknown_field_names: Vec::new(),
    };
    match transport {
        NativeTransport::Stdio => {
            if !command.is_some_and(|value| value.as_str().is_some_and(|text| !text.is_empty())) {
                return Err("INVALID_MCP_COMMAND");
            }
            server.executable_ref = Some(opaque_ref(
                &preview.source_digest,
                &format!("{name}:command"),
            ));
            if let Some(args) = table.get("args") {
                let args = args.as_array().ok_or("INVALID_MCP_ARGUMENTS")?;
                if args.len() > 256 || args.iter().any(|arg| !arg.is_string()) {
                    return Err("INVALID_MCP_ARGUMENTS");
                }
                server.argument_count = args.len();
                server.arguments_ref =
                    Some(opaque_ref(&preview.source_digest, &format!("{name}:args")));
            }
        }
        _ => {
            if !url.is_some_and(|value| value.as_str().is_some_and(|text| !text.is_empty())) {
                return Err("INVALID_MCP_URL");
            }
            server.endpoint_ref = Some(opaque_ref(&preview.source_digest, &format!("{name}:url")));
        }
    }
    if codex {
        if let Some(enabled) = table.get("enabled") {
            server.enabled_declaration = Some(enabled.as_bool().ok_or("INVALID_MCP_ENABLED")?);
        }
    }
    let env = table.get("env");
    if let Some(env) = env {
        let env = env.as_object().ok_or("INVALID_MCP_ENV")?;
        if env.len() > 128 || env.values().any(|value| !value.is_string()) {
            return Err("INVALID_MCP_ENV");
        }
        server.environment_names = checked_names(env.keys().cloned())?;
        server.inline_values_present |= !env.is_empty();
        for key in env.keys() {
            server.credential_refs.push(opaque_ref(
                &preview.source_digest,
                &format!("{name}:env:{key}"),
            ));
        }
    }
    for header_key in ["http_headers", "headers"] {
        if let Some(headers) = table.get(header_key) {
            let headers = headers.as_object().ok_or("INVALID_MCP_HEADERS")?;
            if headers.values().any(|value| !value.is_string()) {
                return Err("INVALID_MCP_HEADERS");
            }
            server
                .header_names
                .extend(checked_names(headers.keys().cloned())?);
            server.inline_values_present |= !headers.is_empty();
            for key in headers.keys() {
                server.credential_refs.push(opaque_ref(
                    &preview.source_digest,
                    &format!("{name}:header:{key}"),
                ));
            }
        }
    }
    if let Some(env_vars) = table.get("env_vars") {
        let values = env_vars.as_array().ok_or("INVALID_MCP_ENV_VARS")?;
        for value in values {
            let name = value
                .as_str()
                .or_else(|| value.get("name").and_then(Value::as_str))
                .ok_or("INVALID_MCP_ENV_VARS")?;
            server
                .environment_names
                .extend(checked_names(std::iter::once(name.to_owned()))?);
        }
    }
    if let Some(headers) = table.get("env_http_headers") {
        let headers = headers.as_object().ok_or("INVALID_MCP_ENV_HEADERS")?;
        server
            .header_names
            .extend(checked_names(headers.keys().cloned())?);
        server.environment_names.extend(checked_names(
            headers
                .values()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or("INVALID_MCP_ENV_HEADERS")
                })
                .collect::<std::result::Result<Vec<_>, _>>()?
                .into_iter(),
        )?);
    }
    if let Some(variable) = table.get("bearer_token_env_var") {
        let variable = variable.as_str().ok_or("INVALID_MCP_BEARER_REFERENCE")?;
        server
            .environment_names
            .extend(checked_names(std::iter::once(variable.to_owned()))?);
        server.credential_refs.push(opaque_ref(
            &preview.source_digest,
            &format!("{name}:bearer-ref"),
        ));
    }
    if let Some(oauth) = table.get("oauth") {
        if !oauth.is_object() {
            return Err("INVALID_MCP_OAUTH");
        }
        server.inline_values_present = true;
        server
            .credential_refs
            .push(opaque_ref(&preview.source_digest, &format!("{name}:oauth")));
    }
    let known = if codex {
        vec![
            "command",
            "args",
            "url",
            "enabled",
            "env",
            "env_vars",
            "http_headers",
            "env_http_headers",
            "bearer_token_env_var",
            "oauth",
        ]
    } else {
        vec!["type", "command", "args", "url", "env", "headers"]
    };
    server.unknown_field_names = table
        .keys()
        .filter(|key| !known.contains(&key.as_str()))
        .cloned()
        .collect();
    server.environment_names.sort();
    server.environment_names.dedup();
    server.header_names.sort();
    server.header_names.dedup();
    Ok(server)
}
fn parse_claude_settings(document: &Map<String, Value>, preview: &mut NativePreview) {
    let mut settings = ClaudeSettingsPreview::default();
    let parsed = (|| -> std::result::Result<(), &'static str> {
        if let Some(value) = document.get("enableAllProjectMcpServers") {
            settings.enable_all_project_mcp_servers =
                Some(value.as_bool().ok_or("INVALID_CLAUDE_MCP_APPROVAL_LIST")?);
        }
        for (key, target) in [
            (
                "enabledMcpjsonServers",
                &mut settings.enabled_project_mcp_names,
            ),
            (
                "disabledMcpjsonServers",
                &mut settings.disabled_project_mcp_names,
            ),
        ] {
            if let Some(value) = document.get(key) {
                let array = value.as_array().ok_or("INVALID_CLAUDE_MCP_APPROVAL_LIST")?;
                *target = checked_names(
                    array
                        .iter()
                        .map(|value| {
                            value
                                .as_str()
                                .map(str::to_owned)
                                .ok_or("INVALID_CLAUDE_MCP_APPROVAL_LIST")
                        })
                        .collect::<std::result::Result<Vec<_>, _>>()?
                        .into_iter(),
                )?;
            }
        }
        if let Some(hooks) = document.get("hooks") {
            settings.hook_event_names = checked_names(
                hooks
                    .as_object()
                    .ok_or("INVALID_CLAUDE_HOOKS")?
                    .keys()
                    .cloned(),
            )?;
        }
        if let Some(env) = document.get("env") {
            let env = env.as_object().ok_or("INVALID_CLAUDE_ENV")?;
            if env.values().any(|value| !value.is_string()) {
                return Err("INVALID_CLAUDE_ENV");
            }
            settings.environment_names = checked_names(env.keys().cloned())?;
        }
        Ok(())
    })();
    if let Err(code) = parsed {
        preview.status = ParseStatus::Malformed;
        diagnose(preview, code, None);
        return;
    }
    preview.unknown_field_names = document
        .keys()
        .filter(|key| {
            ![
                "enableAllProjectMcpServers",
                "enabledMcpjsonServers",
                "disabledMcpjsonServers",
                "hooks",
                "env",
            ]
            .contains(&key.as_str())
        })
        .cloned()
        .collect();
    preview.settings = Some(settings);
}
fn checked_names(
    names: impl Iterator<Item = String>,
) -> std::result::Result<Vec<String>, &'static str> {
    let names: Vec<_> = names.collect();
    if names.len() > 256
        || names
            .iter()
            .any(|name| name.is_empty() || name.len() > 256 || name.chars().any(char::is_control))
    {
        Err("INVALID_METADATA_NAME")
    } else {
        Ok(names)
    }
}
fn opaque_ref(digest: &str, pointer: &str) -> String {
    format!("native:{digest}:{}", &sha256(pointer.as_bytes())[..16])
}
fn readonly_dir(dir: impl std::os::fd::AsFd, path: &Path) -> Result<File> {
    let fd = rfs::openat(
        dir,
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| error(ErrorCode::PermissionDenied))?;
    let stat = io_error(rfs::fstat(&fd))?;
    if stat.st_uid != rustix::process::geteuid().as_raw() || stat.st_mode & 0o022 != 0 {
        return Err(error(ErrorCode::PermissionDenied));
    }
    Ok(File::from(fd))
}
fn read_file(dir: &File, path: &Path) -> Result<Vec<u8>> {
    let fd = rfs::openat(
        dir,
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| error(ErrorCode::PermissionDenied))?;
    let stat = io_error(rfs::fstat(&fd))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_uid != rustix::process::geteuid().as_raw()
        || stat.st_nlink != 1
        || stat.st_mode & 0o022 != 0
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillDialect {
    Codex,
    Claude,
    Qoder,
}
#[derive(Clone, Copy, Debug)]
pub struct DiscoveryLimits {
    pub max_skills: usize,
    pub max_files: usize,
    pub max_depth: usize,
    pub max_total_bytes: usize,
}
impl Default for DiscoveryLimits {
    fn default() -> Self {
        Self {
            max_skills: 64,
            max_files: 256,
            max_depth: 3,
            max_total_bytes: 8_388_608,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillFilePreview {
    pub relative_ref: String,
    pub digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveredSkill {
    pub directory_name: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub short_description: Option<String>,
    pub source_digest: String,
    pub discovered: bool,
    pub runtime_loaded: bool,
    pub status: ParseStatus,
    pub compatibility_hint: Option<String>,
    pub dependency_hints: Vec<String>,
    pub unknown_frontmatter_names: Vec<String>,
    pub scripts: Vec<SkillFilePreview>,
    pub dynamic_context_present: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillDiscovery {
    pub source: SourceDescriptor,
    pub skills: Vec<DiscoveredSkill>,
    pub diagnostics: Vec<Diagnostic>,
    pub complete: bool,
    pub inspected_bytes: usize,
}
impl NativeSourceRoot {
    /// Scan only immediate skill folders in an explicitly approved catalog root.
    /// Script contents are hashed as evidence; never executed or interpreted.
    pub fn discover_skills(
        &self,
        source: SourceDescriptor,
        catalog: &Path,
        dialect: SkillDialect,
        limits: DiscoveryLimits,
    ) -> Result<SkillDiscovery> {
        validate_descriptor(&source)?;
        let expected = match dialect {
            SkillDialect::Codex => "codex",
            SkillDialect::Claude => "claude",
            SkillDialect::Qoder => "qoder",
        };
        if source.provider.provider_id != expected
            && !(dialect == SkillDialect::Claude && source.provider.provider_id == "claude-code")
        {
            return Err(error(ErrorCode::Conflict));
        }
        if limits.max_skills == 0
            || limits.max_skills > 256
            || limits.max_files == 0
            || limits.max_files > 2048
            || limits.max_depth > 8
            || limits.max_total_bytes == 0
            || limits.max_total_bytes > 67_108_864
        {
            return Err(error(ErrorCode::ResourceLimit));
        }
        let directory = self.directory(catalog)?;
        let mut result = SkillDiscovery {
            source,
            skills: Vec::new(),
            diagnostics: Vec::new(),
            complete: true,
            inspected_bytes: 0,
        };
        let entries = directory_names(&directory, limits.max_files)?;
        let mut remaining_files = limits.max_files;
        for name in entries {
            let kind = io_error(rfs::statat(
                &directory,
                name.as_str(),
                AtFlags::SYMLINK_NOFOLLOW,
            ))
            .map(|stat| FileType::from_raw_mode(stat.st_mode));
            if !matches!(kind, Ok(FileType::Directory)) {
                if matches!(kind, Ok(FileType::Symlink)) {
                    result.diagnostics.push(Diagnostic {
                        code: "SKILL_SYMLINK_REJECTED".into(),
                        item_ref: Some(name),
                    });
                    result.complete = false;
                }
                continue;
            }
            if result.skills.len() >= limits.max_skills || remaining_files == 0 {
                result.complete = false;
                result.diagnostics.push(Diagnostic {
                    code: "SKILL_DISCOVERY_LIMIT".into(),
                    item_ref: None,
                });
                break;
            }
            let child = match readonly_dir(&directory, Path::new(&name)) {
                Ok(child) => child,
                Err(_) => {
                    result.complete = false;
                    result.diagnostics.push(Diagnostic {
                        code: "SKILL_DIRECTORY_UNREADABLE".into(),
                        item_ref: Some(name),
                    });
                    continue;
                }
            };
            let bytes = match read_file(&child, Path::new("SKILL.md")) {
                Ok(bytes) => bytes,
                Err(_) => {
                    result.complete = false;
                    result.diagnostics.push(Diagnostic {
                        code: "SKILL_DOCUMENT_UNREADABLE".into(),
                        item_ref: Some(name),
                    });
                    continue;
                }
            };
            if bytes.len() > limits.max_total_bytes - result.inspected_bytes {
                result.complete = false;
                result.diagnostics.push(Diagnostic {
                    code: "SKILL_BYTE_LIMIT".into(),
                    item_ref: None,
                });
                break;
            }
            remaining_files -= 1;
            result.inspected_bytes += bytes.len();
            let mut skill = parse_skill(&name, &bytes, dialect);
            if skill.status != ParseStatus::Parsed {
                result.complete = false;
                result.diagnostics.push(Diagnostic {
                    code: "SKILL_METADATA_MALFORMED_OR_UNSUPPORTED".into(),
                    item_ref: Some(name.clone()),
                });
            }
            match rfs::statat(&child, "scripts", AtFlags::SYMLINK_NOFOLLOW) {
                Ok(stat) if FileType::from_raw_mode(stat.st_mode) == FileType::Directory => {
                    if let Ok(scripts) = readonly_dir(&child, Path::new("scripts")) {
                        inspect_scripts(
                            &scripts,
                            Path::new("scripts"),
                            0,
                            &limits,
                            &mut remaining_files,
                            &mut result,
                            &mut skill,
                        )?;
                    } else {
                        result.complete = false;
                    }
                }
                Ok(_) => {
                    result.complete = false;
                    result.diagnostics.push(Diagnostic {
                        code: "SKILL_SCRIPTS_UNSAFE".into(),
                        item_ref: Some(name.clone()),
                    });
                }
                Err(rustix::io::Errno::NOENT) => {}
                Err(_) => result.complete = false,
            }
            result.skills.push(skill);
        }
        Ok(result)
    }
}
fn directory_names(directory: &File, limit: usize) -> Result<Vec<String>> {
    let entries = io_error(rfs::Dir::read_from(directory))?;
    let mut names = BTreeSet::new();
    for entry in entries {
        let entry = io_error(entry)?;
        let bytes = entry.file_name().to_bytes();
        if matches!(bytes, b"." | b"..") {
            continue;
        }
        if names.len() >= limit {
            return Err(error(ErrorCode::ResourceLimit));
        }
        let name = OsStr::from_bytes(bytes)
            .to_str()
            .ok_or_else(|| error(ErrorCode::InvalidPath))?;
        if name.chars().any(char::is_control) {
            return Err(error(ErrorCode::InvalidPath));
        }
        names.insert(name.to_owned());
    }
    Ok(names.into_iter().collect())
}
// The limits here bound expanded metadata independently of source byte limits.
// IgnoredAny skips YAML aliases as events; it does not build an expanded tree.
const MAX_FRONTMATTER_FIELDS: usize = 256;
const MAX_METADATA_SCALAR_BYTES: usize = 4096;
#[derive(Default)]
struct BoundedFrontmatter {
    name: Option<String>,
    description: Option<String>,
    compatibility: Option<String>,
    allowed_tools: Option<String>,
    metadata: BoundedMetadata,
    unknown_names: Vec<String>,
    unsupported: bool,
}
#[derive(Default)]
struct BoundedMetadata {
    short_description: Option<String>,
    unsupported: bool,
}
struct FieldName(String);
impl<'de> Deserialize<'de> for FieldName {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct KeyVisitor;
        impl serde::de::Visitor<'_> for KeyVisitor {
            type Value = FieldName;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("bounded scalar field name")
            }
            fn visit_str<E: serde::de::Error>(
                self,
                value: &str,
            ) -> std::result::Result<Self::Value, E> {
                if value.len() > 256 {
                    return Err(E::custom("frontmatter field name exceeds limit"));
                }
                Ok(FieldName(value.into()))
            }
        }
        deserializer.deserialize_str(KeyVisitor)
    }
}
struct BoundedScalar {
    value: Option<String>,
    unsupported: bool,
}
impl<'de> Deserialize<'de> for BoundedScalar {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct ScalarVisitor;
        impl<'de> serde::de::Visitor<'de> for ScalarVisitor {
            type Value = BoundedScalar;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("bounded optional scalar")
            }
            fn visit_str<E: serde::de::Error>(
                self,
                value: &str,
            ) -> std::result::Result<Self::Value, E> {
                if value.len() > MAX_METADATA_SCALAR_BYTES {
                    return Ok(BoundedScalar {
                        value: None,
                        unsupported: true,
                    });
                }
                Ok(BoundedScalar {
                    value: Some(value.into()),
                    unsupported: false,
                })
            }
            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(BoundedScalar {
                    value: None,
                    unsupported: false,
                })
            }
            fn visit_bool<E: serde::de::Error>(
                self,
                _: bool,
            ) -> std::result::Result<Self::Value, E> {
                Ok(BoundedScalar {
                    value: None,
                    unsupported: true,
                })
            }
            fn visit_i64<E: serde::de::Error>(self, _: i64) -> std::result::Result<Self::Value, E> {
                Ok(BoundedScalar {
                    value: None,
                    unsupported: true,
                })
            }
            fn visit_u64<E: serde::de::Error>(self, _: u64) -> std::result::Result<Self::Value, E> {
                Ok(BoundedScalar {
                    value: None,
                    unsupported: true,
                })
            }
            fn visit_f64<E: serde::de::Error>(self, _: f64) -> std::result::Result<Self::Value, E> {
                Ok(BoundedScalar {
                    value: None,
                    unsupported: true,
                })
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                Ok(BoundedScalar {
                    value: None,
                    unsupported: true,
                })
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                while map
                    .next_entry::<serde::de::IgnoredAny, serde::de::IgnoredAny>()?
                    .is_some()
                {}
                Ok(BoundedScalar {
                    value: None,
                    unsupported: true,
                })
            }
        }
        deserializer.deserialize_any(ScalarVisitor)
    }
}
impl<'de> Deserialize<'de> for BoundedMetadata {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct MetadataVisitor;
        impl<'de> serde::de::Visitor<'de> for MetadataVisitor {
            type Value = BoundedMetadata;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("bounded metadata mapping")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut result = BoundedMetadata::default();
                let mut keys = BTreeSet::new();
                while let Some(FieldName(key)) = map.next_key()? {
                    if keys.len() >= MAX_FRONTMATTER_FIELDS || !keys.insert(key.clone()) {
                        return Err(serde::de::Error::custom(
                            "metadata field limit or duplicate",
                        ));
                    }
                    if key == "short-description" {
                        let scalar: BoundedScalar = map.next_value()?;
                        result.short_description = scalar.value;
                        result.unsupported |= scalar.unsupported;
                    } else {
                        map.next_value::<serde::de::IgnoredAny>()?;
                    }
                }
                Ok(result)
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                Ok(BoundedMetadata {
                    short_description: None,
                    unsupported: true,
                })
            }
            fn visit_str<E: serde::de::Error>(
                self,
                _: &str,
            ) -> std::result::Result<Self::Value, E> {
                Ok(BoundedMetadata {
                    short_description: None,
                    unsupported: true,
                })
            }
            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(BoundedMetadata {
                    short_description: None,
                    unsupported: true,
                })
            }
        }
        deserializer.deserialize_any(MetadataVisitor)
    }
}
impl<'de> Deserialize<'de> for BoundedFrontmatter {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct FrontmatterVisitor;
        impl<'de> serde::de::Visitor<'de> for FrontmatterVisitor {
            type Value = BoundedFrontmatter;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("bounded skill frontmatter mapping")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut result = BoundedFrontmatter::default();
                let mut keys = BTreeSet::new();
                while let Some(FieldName(key)) = map.next_key()? {
                    if keys.len() >= MAX_FRONTMATTER_FIELDS || !keys.insert(key.clone()) {
                        return Err(serde::de::Error::custom(
                            "frontmatter field limit or duplicate",
                        ));
                    }
                    match key.as_str() {
                        "name" | "description" | "compatibility" | "allowed-tools" => {
                            let scalar: BoundedScalar = map.next_value()?;
                            result.unsupported |= scalar.unsupported;
                            match key.as_str() {
                                "name" => result.name = scalar.value,
                                "description" => result.description = scalar.value,
                                "compatibility" => result.compatibility = scalar.value,
                                _ => result.allowed_tools = scalar.value,
                            }
                        }
                        "metadata" => {
                            result.metadata = map.next_value()?;
                            result.unknown_names.push(key);
                        }
                        _ => {
                            map.next_value::<serde::de::IgnoredAny>()?;
                            result.unknown_names.push(key);
                        }
                    }
                }
                Ok(result)
            }
        }
        deserializer.deserialize_map(FrontmatterVisitor)
    }
}
fn parse_skill(directory: &str, bytes: &[u8], dialect: SkillDialect) -> DiscoveredSkill {
    let mut result = DiscoveredSkill {
        directory_name: directory.into(),
        name: None,
        description: None,
        short_description: None,
        source_digest: sha256(bytes),
        discovered: true,
        runtime_loaded: false,
        status: ParseStatus::Malformed,
        compatibility_hint: None,
        dependency_hints: Vec::new(),
        unknown_frontmatter_names: Vec::new(),
        scripts: Vec::new(),
        dynamic_context_present: false,
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return result;
    };
    result.dynamic_context_present = text.contains("!`");
    let Some(after_start) = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
    else {
        return result;
    };
    let Some(end) = after_start.match_indices("\n---").find_map(|(index, _)| {
        let trailing = &after_start[index + 4..];
        (index <= 65_536
            && (trailing.is_empty() || trailing.starts_with('\n') || trailing.starts_with("\r\n")))
        .then_some(index)
    }) else {
        return result;
    };
    // Do not deserialize untrusted YAML into a generic Value: alias expansion
    // can turn a bounded source file into an unbounded heap allocation.
    let Ok(frontmatter) = serde_yaml_ng::from_str::<BoundedFrontmatter>(&after_start[..end]) else {
        return result;
    };
    if frontmatter.unsupported
        || (dialect == SkillDialect::Codex && frontmatter.metadata.unsupported)
    {
        result.status = ParseStatus::Unsupported;
        return result;
    }
    result.name = frontmatter
        .name
        .filter(|name| !name.trim().is_empty())
        .or_else(|| (dialect != SkillDialect::Qoder).then(|| directory.to_owned()));
    result.description = frontmatter.description;
    if dialect == SkillDialect::Codex {
        result.name = result
            .name
            .map(|name| name.split_whitespace().collect::<Vec<_>>().join(" "));
        result.description = result
            .description
            .map(|description| description.split_whitespace().collect::<Vec<_>>().join(" "));
        result.short_description = frontmatter
            .metadata
            .short_description
            .map(|description| description.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    if dialect == SkillDialect::Codex
        && result
            .name
            .as_ref()
            .is_some_and(|name| name.chars().count() > 64)
    {
        return result;
    }
    if result.name.as_ref().is_none_or(|name| {
        name.is_empty()
            || (dialect != SkillDialect::Codex && name.len() > 128)
            || name.chars().any(char::is_control)
    }) || (dialect != SkillDialect::Claude
        && result
            .description
            .as_ref()
            .is_none_or(|description| description.is_empty()))
        || result
            .description
            .as_ref()
            .is_some_and(|description| description.len() > 4096)
    {
        return result;
    }
    result.compatibility_hint = frontmatter.compatibility;
    if let Some(tools) = frontmatter.allowed_tools {
        result.dependency_hints = tools
            .split_whitespace()
            .take(64)
            .map(str::to_owned)
            .collect();
    }
    result.unknown_frontmatter_names = frontmatter.unknown_names;
    result.status = ParseStatus::Parsed;
    result
}
fn inspect_scripts(
    directory: &File,
    relative: &Path,
    depth: usize,
    limits: &DiscoveryLimits,
    remaining: &mut usize,
    result: &mut SkillDiscovery,
    skill: &mut DiscoveredSkill,
) -> Result<()> {
    if depth > limits.max_depth {
        result.complete = false;
        result.diagnostics.push(Diagnostic {
            code: "SKILL_SCRIPT_DEPTH_LIMIT".into(),
            item_ref: Some(skill.directory_name.clone()),
        });
        return Ok(());
    }
    let entries = match directory_names(directory, *remaining) {
        Ok(entries) => entries,
        Err(_) => {
            result.complete = false;
            result.diagnostics.push(Diagnostic {
                code: "SKILL_SCRIPT_FILE_LIMIT".into(),
                item_ref: Some(skill.directory_name.clone()),
            });
            return Ok(());
        }
    };
    for name in entries {
        if *remaining == 0 {
            result.complete = false;
            break;
        }
        let stat = io_error(rfs::statat(
            directory,
            name.as_str(),
            AtFlags::SYMLINK_NOFOLLOW,
        ))?;
        let path = relative.join(&name);
        match FileType::from_raw_mode(stat.st_mode) {
            FileType::Directory => {
                *remaining -= 1;
                let child = readonly_dir(directory, Path::new(&name))?;
                inspect_scripts(&child, &path, depth + 1, limits, remaining, result, skill)?;
            }
            FileType::RegularFile => {
                *remaining -= 1;
                let bytes = read_file(directory, Path::new(&name))?;
                if bytes.len() > limits.max_total_bytes - result.inspected_bytes {
                    result.complete = false;
                    break;
                }
                result.inspected_bytes += bytes.len();
                skill.scripts.push(SkillFilePreview {
                    relative_ref: path.to_string_lossy().into_owned(),
                    digest: sha256(&bytes),
                });
            }
            _ => {
                result.complete = false;
                result.diagnostics.push(Diagnostic {
                    code: "SKILL_SCRIPT_UNSAFE_FILE".into(),
                    item_ref: Some(skill.directory_name.clone()),
                });
            }
        }
    }
    Ok(())
}
