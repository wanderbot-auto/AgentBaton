use agentbaton_config::{
    capabilities::{native::*, ProviderRevision, SourceDescriptor, SourceKind},
    sha256, ErrorCode,
};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::{Path, PathBuf},
};
struct Fixture {
    _temp: tempfile::TempDir,
    path: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().canonicalize().unwrap();
        Self { _temp: temp, path }
    }
    fn write(&self, path: &str, bytes: impl AsRef<[u8]>) {
        let path = self.path.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o644)).unwrap();
    }
    fn root(&self) -> NativeSourceRoot {
        NativeSourceRoot::open_selected(&self.path).unwrap()
    }
}
fn source(id: &str, provider: &str) -> SourceDescriptor {
    SourceDescriptor {
        source_id: id.into(),
        kind: SourceKind::Native,
        locator_ref: format!("fixture:{id}"),
        provider: ProviderRevision {
            provider_id: provider.into(),
            version: "fixture-version".into(),
        },
        mandatory: false,
    }
}
#[test]
fn codex_documented_toml_shape_is_read_without_secret_values_or_execution() {
    let fixture = Fixture::new();
    let raw = br#"# unknown comment remains intact
model = "fixture-model"
[mcp_servers.tools]
command = "/bin/sh"
args = ["-c", "echo SHOULD-NOT-EXECUTE"]
enabled = false
future_option = "PRIVATE-UNKNOWN-VALUE"
[mcp_servers.tools.env]
API_TOKEN = "PRIVATE-ENV-VALUE"
[mcp_servers.remote]
url = "https://example.test/mcp?token=PRIVATE-URL-VALUE"
bearer_token_env_var = "REMOTE_TOKEN"
[mcp_servers.remote.http_headers]
Authorization = "PRIVATE-HEADER-VALUE"
[mcp_servers.remote.env_http_headers]
"X-Auth" = "HEADER_TOKEN"
"#;
    fixture.write("config.toml", raw);
    let root = fixture.root();
    let native = root
        .read_native(
            source("user", "codex"),
            Path::new("config.toml"),
            NativeFormat::CodexToml,
        )
        .unwrap();
    let preview = native.preview();
    assert_eq!(preview.status, ParseStatus::Parsed);
    assert_eq!(preview.mcp_servers.len(), 2);
    assert!(native.preserves_original_hash());
    assert!(root.verify_unchanged(&native).unwrap());
    let tools = preview
        .mcp_servers
        .iter()
        .find(|server| server.name == "tools")
        .unwrap();
    assert_eq!(tools.transport, NativeTransport::Stdio);
    assert_eq!(tools.enabled_declaration, Some(false));
    assert_eq!(tools.argument_count, 2);
    assert_eq!(tools.environment_names, vec!["API_TOKEN"]);
    assert!(tools.inline_values_present);
    assert!(tools.unknown_field_names.contains(&"future_option".into()));
    let remote = preview
        .mcp_servers
        .iter()
        .find(|server| server.name == "remote")
        .unwrap();
    assert!(remote.environment_names.contains(&"REMOTE_TOKEN".into()));
    assert!(remote.environment_names.contains(&"HEADER_TOKEN".into()));
    let encoded = serde_json::to_string(&preview).unwrap();
    for private in [
        "PRIVATE-ENV-VALUE",
        "PRIVATE-URL-VALUE",
        "PRIVATE-HEADER-VALUE",
        "PRIVATE-UNKNOWN-VALUE",
        "SHOULD-NOT-EXECUTE",
        "/bin/sh",
    ] {
        assert!(!encoded.contains(private));
    }
    assert!(!preview.runtime_evidence_observed);
    assert_eq!(fs::read(fixture.path.join("config.toml")).unwrap(), raw);
}
#[test]
fn claude_project_and_user_mcp_json_shapes_remain_distinct_without_inferred_enablement() {
    let fixture = Fixture::new();
    fixture.write("project.mcp.json",serde_json::to_vec(&json!({"mcpServers":{"shared":{"command":"npx","args":["-y","@fixture/server"],"env":{"TOKEN":"PRIVATE-PROJECT-TOKEN"}}}})).unwrap());
    fixture.write("user.claude.json",serde_json::to_vec(&json!({"mcpServers":{"shared":{"type":"http","url":"https://example.test/private-token","headers":{"Authorization":"PRIVATE-USER-TOKEN"}}},"projects":{"/private/path":{"mcpServers":{}}}})).unwrap());
    let root = fixture.root();
    let project = root
        .read_native(
            source("project", "claude"),
            Path::new("project.mcp.json"),
            NativeFormat::ClaudeMcpJson,
        )
        .unwrap();
    let user = root
        .read_native(
            source("user", "claude"),
            Path::new("user.claude.json"),
            NativeFormat::ClaudeMcpJson,
        )
        .unwrap();
    assert_ne!(
        project.preview().source.source_id,
        user.preview().source.source_id
    );
    assert_eq!(
        project.preview().mcp_servers[0].name,
        user.preview().mcp_servers[0].name
    );
    assert_eq!(project.preview().mcp_servers[0].enabled_declaration, None);
    assert_eq!(
        user.preview().mcp_servers[0].transport,
        NativeTransport::Http
    );
    let preview = serde_json::to_string(&user.preview()).unwrap();
    assert!(!preview.contains("PRIVATE-USER-TOKEN"));
    assert!(!preview.contains("/private/path"));
}
#[test]
fn claude_url_without_explicit_type_is_diagnostic_and_never_fake_connected() {
    let fixture = Fixture::new();
    fixture.write("mcp.json",br#"{"mcpServers":{"bad":{"url":"https://example.test"},"sdk":{"type":"sdk"},"good":{"command":"echo"}}}"#);
    let native = fixture
        .root()
        .read_native(
            source("project", "claude"),
            Path::new("mcp.json"),
            NativeFormat::ClaudeMcpJson,
        )
        .unwrap();
    let preview = native.preview();
    assert_eq!(preview.status, ParseStatus::Partial);
    assert_eq!(preview.mcp_servers.len(), 1);
    assert_eq!(preview.diagnostics.len(), 2);
    assert!(!preview.runtime_evidence_observed);
}
#[test]
fn claude_settings_report_only_approval_names_env_names_and_hook_events() {
    let fixture = Fixture::new();
    let marker = fixture.path.join("executed");
    fixture.write("settings.json",serde_json::to_vec(&json!({"enableAllProjectMcpServers":true,"enabledMcpjsonServers":["a"],"disabledMcpjsonServers":["a","b"],"env":{"API_TOKEN":"PRIVATE-SETTINGS-TOKEN"},"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":format!("touch {}",marker.display())}]}]},"apiKeyHelper":"PRIVATE-HELPER-COMMAND"})).unwrap());
    let native = fixture
        .root()
        .read_native(
            source("settings", "claude-code"),
            Path::new("settings.json"),
            NativeFormat::ClaudeSettingsJson,
        )
        .unwrap();
    let preview = native.preview();
    let settings = preview.settings.as_ref().unwrap();
    assert_eq!(settings.enable_all_project_mcp_servers, Some(true));
    assert_eq!(settings.enabled_project_mcp_names, vec!["a"]);
    assert_eq!(settings.disabled_project_mcp_names, vec!["a", "b"]);
    assert_eq!(settings.hook_event_names, vec!["SessionStart"]);
    let encoded = serde_json::to_string(&preview).unwrap();
    assert!(!encoded.contains("PRIVATE-SETTINGS-TOKEN"));
    assert!(!encoded.contains("PRIVATE-HELPER-COMMAND"));
    assert!(!encoded.contains("touch "));
    assert!(!marker.exists());
    // Native approval declarations are not converted into AgentBaton grants.
    assert!(preview.mcp_servers.is_empty());
    assert!(!preview.runtime_evidence_observed);
}
#[test]
fn unknown_qoder_configuration_shape_is_explicitly_unsupported() {
    let fixture = Fixture::new();
    let raw =
        br#"{"mcpServers":{"server":{"command":"echo","env":{"TOKEN":"PRIVATE-QODER-TOKEN"}}}}"#;
    fixture.write("settings.json", raw);
    let native = fixture
        .root()
        .read_native(
            source("qoder", "qoder"),
            Path::new("settings.json"),
            NativeFormat::QoderSettingsUnknown,
        )
        .unwrap();
    let preview = native.preview();
    assert_eq!(preview.status, ParseStatus::Unsupported);
    assert!(preview.mcp_servers.is_empty());
    assert_eq!(
        preview.diagnostics[0].code,
        "QODER_JSON_SCHEMA_NOT_CONFIRMED"
    );
    assert!(!serde_json::to_string(&preview)
        .unwrap()
        .contains("PRIVATE-QODER-TOKEN"));
    assert!(native.preserves_original_hash());
}
#[test]
fn malformed_native_document_never_echoes_secret_parse_snippet() {
    let fixture = Fixture::new();
    for (name, provider, format, bytes) in [
        (
            "config.toml",
            "codex",
            NativeFormat::CodexToml,
            "[mcp_servers.\nsecret=PRIVATE-PARSE-VALUE",
        ),
        (
            "settings.json",
            "claude",
            NativeFormat::ClaudeSettingsJson,
            "{\"token\":\"PRIVATE-PARSE-VALUE\",}",
        ),
    ] {
        fixture.write(name, bytes);
        let preview = fixture
            .root()
            .read_native(source("bad", provider), Path::new(name), format)
            .unwrap()
            .preview();
        assert_eq!(preview.status, ParseStatus::Malformed);
        assert!(!serde_json::to_string(&preview)
            .unwrap()
            .contains("PRIVATE-PARSE-VALUE"));
    }
}
#[test]
fn source_drift_symlink_traversal_auth_file_and_world_writes_fail_safely() {
    let fixture = Fixture::new();
    let outside = Fixture::new();
    outside.write("mcp.json", br#"{"mcpServers":{}}"#);
    fixture.write("mcp.json", br#"{"mcpServers":{}}"#);
    let root = fixture.root();
    let native = root
        .read_native(
            source("one", "claude"),
            Path::new("mcp.json"),
            NativeFormat::ClaudeMcpJson,
        )
        .unwrap();
    fixture.write("mcp.json", br#"{"mcpServers":{},"new":1}"#);
    assert!(!root.verify_unchanged(&native).unwrap());
    symlink(
        outside.path.join("mcp.json"),
        fixture.path.join("link.json"),
    )
    .unwrap();
    for path in ["../mcp.json", "/etc/passwd", "link.json"] {
        assert!(root
            .read_native(
                source("unsafe", "claude"),
                Path::new(path),
                NativeFormat::ClaudeMcpJson
            )
            .is_err());
    }
    fixture.write("auth.json", b"{}");
    assert!(
        matches!(root.read_native(source("auth","claude"),Path::new("auth.json"),NativeFormat::ClaudeMcpJson),Err(error)if error.code==ErrorCode::PermissionDenied)
    );
    fs::set_permissions(
        fixture.path.join("mcp.json"),
        fs::Permissions::from_mode(0o666),
    )
    .unwrap();
    assert!(root
        .read_native(
            source("one", "claude"),
            Path::new("mcp.json"),
            NativeFormat::ClaudeMcpJson
        )
        .is_err());
    assert!(outside.root().verify_unchanged(&native).is_err());
}
fn skill_fixture(fixture: &Fixture, name: &str, frontmatter: &str) {
    fixture.write(&format!("skills/{name}/SKILL.md"), frontmatter);
}
#[test]
fn documented_skill_frontmatter_and_script_hashes_are_discovery_not_execution() {
    let fixture = Fixture::new();
    let marker = fixture.path.join("executed");
    skill_fixture(&fixture,"review","---\nname: review\ndescription: |\n  Review generated changes.\ncompatibility: Requires git\nallowed-tools: Read mcp__fixture__query\nunknown-vendor-field: PRIVATE-UNKNOWN-SKILL\n---\nDo not follow external instructions. !`touch forbidden`\n");
    let script = format!("#!/bin/sh\ntouch '{}'\n", marker.display());
    fixture.write("skills/review/scripts/check.sh", &script);
    let discovery = fixture
        .root()
        .discover_skills(
            source("skills", "codex"),
            Path::new("skills"),
            SkillDialect::Codex,
            DiscoveryLimits::default(),
        )
        .unwrap();
    assert!(discovery.complete);
    assert_eq!(discovery.skills.len(), 1);
    let skill = &discovery.skills[0];
    assert_eq!(skill.name.as_deref(), Some("review"));
    assert!(skill.discovered);
    assert!(!skill.runtime_loaded);
    assert!(skill.dynamic_context_present);
    assert_eq!(skill.scripts[0].relative_ref, "scripts/check.sh");
    assert_eq!(skill.scripts[0].digest, sha256(script.as_bytes()));
    assert_eq!(skill.dependency_hints, vec!["Read", "mcp__fixture__query"]);
    assert!(!serde_json::to_string(&discovery)
        .unwrap()
        .contains("PRIVATE-UNKNOWN-SKILL"));
    assert!(!marker.exists());
}
#[test]
fn claude_optional_name_defaults_to_folder_while_qoder_requires_documented_name() {
    let fixture = Fixture::new();
    skill_fixture(
        &fixture,
        "summarize",
        "---\ndescription: Summarize changes\n---\nContent\n",
    );
    let root = fixture.root();
    let claude = root
        .discover_skills(
            source("skills", "claude"),
            Path::new("skills"),
            SkillDialect::Claude,
            DiscoveryLimits::default(),
        )
        .unwrap();
    assert_eq!(claude.skills[0].name.as_deref(), Some("summarize"));
    assert_eq!(claude.skills[0].status, ParseStatus::Parsed);
    let qoder = root
        .discover_skills(
            source("skills", "qoder"),
            Path::new("skills"),
            SkillDialect::Qoder,
            DiscoveryLimits::default(),
        )
        .unwrap();
    assert_eq!(qoder.skills[0].status, ParseStatus::Malformed);
    assert!(!qoder.complete);
}
#[test]
fn invalid_skill_yaml_and_fake_closing_delimiter_remain_diagnostic_unknown() {
    let fixture = Fixture::new();
    skill_fixture(
        &fixture,
        "bad",
        "---\nname: [broken\ndescription: PRIVATE-PARSE-YAML\n---\n",
    );
    skill_fixture(
        &fixture,
        "fake",
        "---\nname: fake\ndescription: text\n---NOT-A-DELIMITER\n",
    );
    let result = fixture
        .root()
        .discover_skills(
            source("skills", "codex"),
            Path::new("skills"),
            SkillDialect::Codex,
            DiscoveryLimits::default(),
        )
        .unwrap();
    assert_eq!(result.skills.len(), 2);
    assert!(result
        .skills
        .iter()
        .all(|skill| skill.status == ParseStatus::Malformed));
    assert!(!result.complete);
    assert!(!serde_json::to_string(&result)
        .unwrap()
        .contains("PRIVATE-PARSE-YAML"));
}
#[test]
fn skill_symlinks_and_script_symlinks_are_never_followed_and_discovery_is_bounded() {
    let fixture = Fixture::new();
    let outside = Fixture::new();
    skill_fixture(
        &fixture,
        "safe",
        "---\nname: safe\ndescription: safe\n---\n",
    );
    outside.write("secret.sh", b"PRIVATE-OUTSIDE-SCRIPT");
    symlink(&outside.path, fixture.path.join("skills/linked")).unwrap();
    symlink(
        outside.path.join("secret.sh"),
        fixture.path.join("skills/safe/scripts"),
    )
    .unwrap();
    let result = fixture
        .root()
        .discover_skills(
            source("skills", "codex"),
            Path::new("skills"),
            SkillDialect::Codex,
            DiscoveryLimits::default(),
        )
        .unwrap();
    assert!(!result.complete);
    assert!(result.skills[0].scripts.is_empty());
    assert!(!serde_json::to_string(&result)
        .unwrap()
        .contains("PRIVATE-OUTSIDE-SCRIPT"));
    let tiny = DiscoveryLimits {
        max_total_bytes: 1,
        ..DiscoveryLimits::default()
    };
    let result = fixture
        .root()
        .discover_skills(
            source("skills", "codex"),
            Path::new("skills"),
            SkillDialect::Codex,
            tiny,
        )
        .unwrap();
    assert!(!result.complete);
    assert!(result.skills.is_empty());
}
#[test]
fn inherited_skill_name_collision_preserves_separate_source_evidence() {
    let project = Fixture::new();
    let user = Fixture::new();
    for fixture in [&project, &user] {
        skill_fixture(
            fixture,
            "same",
            "---\nname: same\ndescription: same name different source\n---\n",
        );
    }
    let project_skills = project
        .root()
        .discover_skills(
            source("project", "qoder"),
            Path::new("skills"),
            SkillDialect::Qoder,
            DiscoveryLimits::default(),
        )
        .unwrap();
    let user_skills = user
        .root()
        .discover_skills(
            source("user", "qoder"),
            Path::new("skills"),
            SkillDialect::Qoder,
            DiscoveryLimits::default(),
        )
        .unwrap();
    assert_eq!(project_skills.skills[0].name, user_skills.skills[0].name);
    assert_ne!(
        project_skills.source.source_id,
        user_skills.source.source_id
    );
    assert!(!project_skills.skills[0].runtime_loaded && !user_skills.skills[0].runtime_loaded);
}
#[test]
fn codex_pinned_parser_optional_name_short_description_and_length_are_respected() {
    let fixture = Fixture::new();
    skill_fixture(&fixture,"review","---\ndescription: |\n  Review generated\n  changes\nmetadata:\n  short-description: Quick review\n---\n");
    let result = fixture
        .root()
        .discover_skills(
            source("skills", "codex"),
            Path::new("skills"),
            SkillDialect::Codex,
            DiscoveryLimits::default(),
        )
        .unwrap();
    assert_eq!(result.skills[0].name.as_deref(), Some("review"));
    assert_eq!(
        result.skills[0].description.as_deref(),
        Some("Review generated changes")
    );
    assert_eq!(
        result.skills[0].short_description.as_deref(),
        Some("Quick review")
    );
    skill_fixture(
        &fixture,
        "long",
        &format!(
            "---\nname: {}\ndescription: too long\n---\n",
            "x".repeat(65)
        ),
    );
    let result = fixture
        .root()
        .discover_skills(
            source("skills", "codex"),
            Path::new("skills"),
            SkillDialect::Codex,
            DiscoveryLimits::default(),
        )
        .unwrap();
    assert_eq!(
        result
            .skills
            .iter()
            .find(|skill| skill.directory_name == "long")
            .unwrap()
            .status,
        ParseStatus::Malformed
    );
}
#[test]
fn non_string_claude_transport_is_not_silently_defaulted_to_stdio() {
    let fixture = Fixture::new();
    fixture.write(
        "mcp.json",
        br#"{"mcpServers":{"bad":{"type":5,"command":"echo"}}}"#,
    );
    let preview = fixture
        .root()
        .read_native(
            source("one", "claude"),
            Path::new("mcp.json"),
            NativeFormat::ClaudeMcpJson,
        )
        .unwrap()
        .preview();
    assert_eq!(preview.status, ParseStatus::Partial);
    assert!(preview.mcp_servers.is_empty());
    assert_eq!(preview.diagnostics[0].code, "INVALID_MCP_TYPE");
}
#[test]
fn malformed_known_skill_metadata_is_not_silently_defaulted_or_enabled() {
    let fixture = Fixture::new();
    skill_fixture(
        &fixture,
        "bad",
        "---\nname: [unexpected, array]\ndescription: Normal description\n---\n",
    );
    for (provider, dialect) in [
        ("codex", SkillDialect::Codex),
        ("claude", SkillDialect::Claude),
        ("qoder", SkillDialect::Qoder),
    ] {
        let result = fixture
            .root()
            .discover_skills(
                source("skills", provider),
                Path::new("skills"),
                dialect,
                DiscoveryLimits::default(),
            )
            .unwrap();
        assert_eq!(result.skills[0].status, ParseStatus::Unsupported);
        assert!(!result.complete);
        assert!(!result.skills[0].runtime_loaded);
    }
}
#[test]
fn codex_native_name_bound_counts_unicode_characters_instead_of_utf8_bytes() {
    let fixture = Fixture::new();
    for (directory, count) in [("exact", 64), ("too-long", 65)] {
        skill_fixture(
            &fixture,
            directory,
            &format!(
                "---\nname: {}\ndescription: Unicode name validation\n---\n",
                "中".repeat(count)
            ),
        );
    }
    let discovery = fixture
        .root()
        .discover_skills(
            source("skills", "codex"),
            Path::new("skills"),
            SkillDialect::Codex,
            DiscoveryLimits::default(),
        )
        .unwrap();
    let exact = discovery
        .skills
        .iter()
        .find(|skill| skill.directory_name == "exact")
        .unwrap();
    assert_eq!(exact.status, ParseStatus::Parsed);
    assert_eq!(exact.name.as_ref().unwrap().chars().count(), 64);
    assert_eq!(
        discovery
            .skills
            .iter()
            .find(|skill| skill.directory_name == "too-long")
            .unwrap()
            .status,
        ParseStatus::Malformed
    );
}
#[test]
fn claude_settings_non_string_env_values_are_malformed_without_value_export() {
    let fixture = Fixture::new();
    fixture.write(
        "settings.json",
        br#"{"env":{"TOKEN":{"private":"PRIVATE-INVALID-VALUE"}}}"#,
    );
    let native = fixture
        .root()
        .read_native(
            source("settings", "claude"),
            Path::new("settings.json"),
            NativeFormat::ClaudeSettingsJson,
        )
        .unwrap();
    let preview = native.preview();
    assert_eq!(preview.status, ParseStatus::Malformed);
    assert!(preview.settings.is_none());
    assert_eq!(preview.diagnostics[0].code, "INVALID_CLAUDE_ENV");
    assert!(!serde_json::to_string(&preview)
        .unwrap()
        .contains("PRIVATE-INVALID-VALUE"));
    assert!(native.preserves_original_hash());
}
#[test]
fn yaml_alias_expansion_is_bounded_in_an_isolated_process() {
    const CHILD_MARKER: &str = "AGENTBATON_YAML_BOUNDED_CHILD";
    if std::env::var_os(CHILD_MARKER).is_none() {
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "yaml_alias_expansion_is_bounded_in_an_isolated_process",
                "--test-threads=1",
                "--nocapture",
            ])
            .env(CHILD_MARKER, "1")
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "isolated YAML parser regression failed: {}",
            String::from_utf8_lossy(&child.stderr)
        );
        return;
    }
    let fixture = Fixture::new();
    // Linux uses the reviewer's 128MiB limit in the child only. Other platforms
    // exercise the same alias topology with a small safe expansion if regressed.
    let scalar_length = if cfg!(target_os = "linux") {
        30_000
    } else {
        128
    };
    let mut bomb = format!(
        "---\nname: alias-check\ndescription: Bounded alias fixture\na: &a '{}'\n",
        "x".repeat(scalar_length)
    );
    for (anchor, previous) in [('b', 'a'), ('c', 'b'), ('d', 'c'), ('e', 'd'), ('f', 'e')] {
        bomb.push_str(&format!(
            "{anchor}: &{anchor} [{}]\n",
            std::iter::repeat_n(format!("*{previous}"), 8)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    skill_fixture(
        &fixture,
        "unknown",
        &format!("{bomb}---\nOpaque unknown values must not expand.\n"),
    );
    skill_fixture(
        &fixture,
        "known",
        &format!(
            "{bomb}allowed-tools: *f\n---\nKnown metadata does not accept expanded sequences.\n"
        ),
    );
    skill_fixture(
        &fixture,
        "literal",
        "---\nname: literal\ndescription: 'Literal * and & remain ordinary text'\n---\n",
    );
    #[cfg(target_os = "linux")]
    rustix::process::setrlimit(
        rustix::process::Resource::As,
        rustix::process::Rlimit {
            current: Some(128 * 1024 * 1024),
            maximum: Some(128 * 1024 * 1024),
        },
    )
    .unwrap();
    let discovery = fixture
        .root()
        .discover_skills(
            source("skills", "codex"),
            Path::new("skills"),
            SkillDialect::Codex,
            DiscoveryLimits {
                max_skills: 4,
                max_files: 10,
                max_depth: 1,
                max_total_bytes: 65_536,
            },
        )
        .unwrap();
    assert_eq!(
        discovery
            .skills
            .iter()
            .find(|skill| skill.directory_name == "unknown")
            .unwrap()
            .status,
        ParseStatus::Parsed
    );
    assert_eq!(
        discovery
            .skills
            .iter()
            .find(|skill| skill.directory_name == "known")
            .unwrap()
            .status,
        ParseStatus::Unsupported
    );
    assert_eq!(
        discovery
            .skills
            .iter()
            .find(|skill| skill.directory_name == "literal")
            .unwrap()
            .description
            .as_deref(),
        Some("Literal * and & remain ordinary text")
    );
    assert!(discovery.inspected_bytes < 65_536);
}
#[test]
fn skill_frontmatter_field_and_scalar_limits_are_independent_of_file_budget() {
    let fixture = Fixture::new();
    skill_fixture(
        &fixture,
        "scalar",
        &format!(
            "---\nname: scalar\ndescription: '{}'\n---\n",
            "x".repeat(4097)
        ),
    );
    let mut fields = "---\nname: fields\ndescription: Too many fields\n".to_string();
    for index in 0..257 {
        fields.push_str(&format!("unknown-{index}: null\n"));
    }
    fields.push_str("---\n");
    skill_fixture(&fixture, "fields", &fields);
    let discovery = fixture
        .root()
        .discover_skills(
            source("skills", "codex"),
            Path::new("skills"),
            SkillDialect::Codex,
            DiscoveryLimits::default(),
        )
        .unwrap();
    assert_eq!(
        discovery
            .skills
            .iter()
            .find(|skill| skill.directory_name == "scalar")
            .unwrap()
            .status,
        ParseStatus::Unsupported
    );
    assert_eq!(
        discovery
            .skills
            .iter()
            .find(|skill| skill.directory_name == "fields")
            .unwrap()
            .status,
        ParseStatus::Malformed
    );
    assert!(!discovery.complete);
}
