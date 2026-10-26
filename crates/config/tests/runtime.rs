//! Private generated namespaces only. No native configuration/account is read.
use agentbaton_config::runtime::*;
use agentbaton_config::{sha256, ErrorCode, Ownership};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::{symlink, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};
use tempfile::TempDir;

fn root() -> (TempDir, PathBuf, RuntimeRoot) {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().canonicalize().unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let root = RuntimeRoot::open(&path).unwrap();
    (tmp, path, root)
}
fn text(s: &str) -> NonSecretText {
    NonSecretText::approved(s.into()).unwrap()
}
fn source() -> SourceProvenance {
    SourceProvenance {
        source_ref: "selected-source".into(),
        revision: "revision-1".into(),
        sha256: sha256(b"selected-nonsecret-source"),
    }
}
fn codex(root: &RuntimeRoot) -> RuntimeSnapshot {
    root.materialize(
        RuntimePlan::empty(RuntimeProvider::Codex),
        Ownership::BatonManaged,
    )
    .unwrap()
}
fn verify(root: &RuntimeRoot, snapshot: &RuntimeSnapshot) -> RuntimeSnapshot {
    root.verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
        .unwrap()
}
fn skill() -> SelectedSkill {
    SelectedSkill{directory_name:"approved-skill".into(),instruction:text("---\nname: approved-skill\ndescription: selected test content\n---\nRead selected input."),assets:vec![SkillAsset{relative_path:"scripts/check.sh".into(),content:NonSecretBytes::approved(b"#!/bin/sh\ntouch MUST_NOT_EXECUTE\n".to_vec()).unwrap(),script:true}],provenance:source()}
}

#[test]
fn codex_uses_pinned_native_file_credential_backends_and_private_roots() {
    let (_tmp, path, root) = root();
    let snapshot = codex(&root);
    let bytes =
        fs::read_to_string(snapshot.provider_configuration_root().join("config.toml")).unwrap();
    let doc: toml::Value = toml::from_str(&bytes).unwrap();
    assert_eq!(doc["cli_auth_credentials_store"].as_str(), Some("file"));
    assert_eq!(doc["mcp_oauth_credentials_store"].as_str(), Some("file"));
    assert_eq!(doc["sandbox_mode"].as_str(), Some("workspace-write"));
    assert_eq!(doc["approval_policy"].as_str(), Some("on-request"));
    assert_eq!(
        doc["log_dir"].as_str().unwrap(),
        snapshot.directory().join("state/log").to_str().unwrap()
    );
    assert!(snapshot.provider_configuration_root().is_dir());
    assert!(!snapshot
        .provider_configuration_root()
        .join("auth.json")
        .exists());
    assert!(!snapshot
        .provider_configuration_root()
        .join(".credentials.json")
        .exists());
    let env = snapshot.environment();
    assert_eq!(
        Path::new(&env[std::ffi::OsStr::new("CODEX_HOME")]),
        snapshot.provider_configuration_root()
    );
    assert_eq!(env.len(), 7);
    for value in env.values() {
        assert!(Path::new(value).starts_with(&path));
        assert!(Path::new(value).is_dir());
    }
    assert_eq!(
        snapshot.manifest().claims.native_identity_isolation,
        EvidenceState::Unknown
    );
}

#[test]
fn logical_plan_digest_is_stable_but_real_file_digest_captures_absolute_materialization() {
    let (_tmp, _path, root) = root();
    let mut plan = RuntimePlan::empty(RuntimeProvider::Codex);
    plan.codex.model = Some(text("example-model"));
    let expected = plan.plan_digest().unwrap();
    assert_ne!(expected, sha256(&serde_json::to_vec(&plan).unwrap()));
    let first = root
        .materialize(plan.clone(), Ownership::BatonManaged)
        .unwrap();
    let second = root.materialize(plan, Ownership::BatonManaged).unwrap();
    assert_eq!(first.plan_digest(), expected);
    assert_eq!(
        first.manifest().materializer_revision,
        "agentbaton-managed-runtime-v1"
    );
    assert_eq!(second.plan_digest(), expected);
    assert_ne!(first.descriptor().id, second.descriptor().id);
    assert_ne!(first.digest(), second.digest());
    assert_eq!(
        first.codex_settings().unwrap().model_name(),
        Some("example-model")
    );
    assert_ne!(
        first.descriptor().snapshot_inode,
        second.descriptor().snapshot_inode
    );
}

#[test]
fn approved_plan_can_be_boundedly_restored_without_conferring_ownership() {
    let (_tmp, path, root) = root();
    let plan = RuntimePlan::empty(RuntimeProvider::CustomAcp);
    let bytes = serde_json::to_vec(&plan).unwrap();
    let restored = RuntimePlan::decode_record(&bytes).unwrap();
    assert_eq!(restored.provider(), RuntimeProvider::CustomAcp);
    assert_eq!(restored.plan_digest().unwrap(), plan.plan_digest().unwrap());
    assert_eq!(
        root.materialize(restored, Ownership::NativeConnected)
            .err()
            .unwrap()
            .code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(fs::read_dir(path).unwrap().count(), 0);
}

#[test]
fn strict_native_identity_or_hidden_inheritance_requirement_fails_closed() {
    let (_tmp, _path, root) = root();
    let snapshot = codex(&root);
    for requirements in [
        RuntimeRequirements {
            no_hidden_inheritance: true,
            native_identity_isolation: false,
        },
        RuntimeRequirements {
            no_hidden_inheritance: false,
            native_identity_isolation: true,
        },
    ] {
        assert_eq!(
            root.verify_before_launch(&snapshot.descriptor(), requirements)
                .err()
                .unwrap()
                .code,
            ErrorCode::PermissionDenied
        );
    }
    verify(&root, &snapshot);
}

#[test]
fn content_or_manifest_drift_is_rejected() {
    let (_tmp, _path, root) = root();
    let first = codex(&root);
    fs::write(
        first.provider_configuration_root().join("config.toml"),
        "model='changed'",
    )
    .unwrap();
    assert_eq!(
        root.verify_before_launch(&first.descriptor(), RuntimeRequirements::default())
            .err()
            .unwrap()
            .code,
        ErrorCode::Conflict
    );
    let second = codex(&root);
    fs::write(
        second.directory().join(".baton-runtime-manifest.json"),
        "{}",
    )
    .unwrap();
    assert_eq!(
        root.verify_before_launch(&second.descriptor(), RuntimeRequirements::default())
            .err()
            .unwrap()
            .code,
        ErrorCode::Conflict
    );
}

#[test]
fn unknown_config_file_is_not_silently_inherited() {
    let (_tmp, _path, root) = root();
    let snapshot = codex(&root);
    fs::write(
        snapshot
            .provider_configuration_root()
            .join("extra-config.toml"),
        "selected=false",
    )
    .unwrap();
    assert_eq!(
        root.verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
            .err()
            .unwrap()
            .code,
        ErrorCode::Conflict
    );
}

#[test]
fn mutable_runtime_logs_cache_and_tmp_are_separate_from_configuration_hashes() {
    let (_tmp, _path, root) = root();
    let snapshot = codex(&root);
    for dir in ["state/log", "state/sqlite", "state/tmp", "state/xdg/cache"] {
        fs::write(
            snapshot.directory().join(dir).join("native-state"),
            b"owned runtime output",
        )
        .unwrap();
    }
    let again = verify(&root, &snapshot);
    assert_eq!(again.digest(), snapshot.digest());
    assert_eq!(again.plan_digest(), snapshot.plan_digest());
    assert_eq!(
        again.manifest().claims.no_hidden_native_inheritance,
        EvidenceState::Unknown
    );
}

#[test]
fn root_replacement_is_rejected_without_touching_new_directory() {
    let container = tempfile::tempdir().unwrap();
    let path = container.path().canonicalize().unwrap().join("runtime");
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let root = RuntimeRoot::open(&path).unwrap();
    let snapshot = codex(&root);
    fs::rename(&path, container.path().join("original")).unwrap();
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        root.verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
            .err()
            .unwrap()
            .code,
        ErrorCode::Conflict
    );
    assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
}

#[test]
fn snapshot_directory_replacement_is_rejected_even_with_copied_files() {
    let (_tmp, _path, root) = root();
    let snapshot = codex(&root);
    let old = snapshot.directory().with_extension("old");
    fs::rename(snapshot.directory(), &old).unwrap();
    fs::create_dir(snapshot.directory()).unwrap();
    fs::set_permissions(snapshot.directory(), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        root.verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
            .err()
            .unwrap()
            .code,
        ErrorCode::Conflict
    );
    assert!(old.join("codex/config.toml").exists());
}

#[test]
fn symlinked_config_file_cannot_escape_selected_namespace() {
    let (tmp, _path, root) = root();
    let snapshot = codex(&root);
    let outside = tmp.path().join("unselected-original");
    fs::write(&outside, "unselected-original").unwrap();
    let file = snapshot.provider_configuration_root().join("config.toml");
    fs::remove_file(&file).unwrap();
    symlink(&outside, &file).unwrap();
    assert_eq!(
        root.verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
            .err()
            .unwrap()
            .code,
        ErrorCode::Conflict
    );
    assert_eq!(fs::read_to_string(&outside).unwrap(), "unselected-original");
}

#[test]
fn symlinked_mutable_directory_is_rejected_without_scanning_target() {
    let (tmp, _path, root) = root();
    let snapshot = codex(&root);
    let directory = snapshot.directory().join("state/log");
    fs::remove_dir(&directory).unwrap();
    let outside = tmp.path().join("external");
    fs::create_dir(&outside).unwrap();
    symlink(&outside, &directory).unwrap();
    assert!(root
        .verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
        .is_err());
    assert_eq!(fs::read_dir(outside).unwrap().count(), 0);
}

#[test]
fn fifo_config_file_is_rejected_without_waiting_for_a_writer() {
    let (_tmp, _path, root) = root();
    let snapshot = codex(&root);
    let file = snapshot.provider_configuration_root().join("config.toml");
    fs::remove_file(&file).unwrap();
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &file,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    let started = std::time::Instant::now();
    assert!(root
        .verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
        .is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn hardlinked_config_and_permissive_files_are_rejected() {
    let (tmp, _path, root) = root();
    let first = codex(&root);
    fs::hard_link(
        first.provider_configuration_root().join("config.toml"),
        tmp.path().join("unapproved-link"),
    )
    .unwrap();
    assert_eq!(
        root.verify_before_launch(&first.descriptor(), RuntimeRequirements::default())
            .err()
            .unwrap()
            .code,
        ErrorCode::PermissionDenied
    );
    let second = codex(&root);
    let file = second.provider_configuration_root().join("config.toml");
    fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        root.verify_before_launch(&second.descriptor(), RuntimeRequirements::default())
            .err()
            .unwrap()
            .code,
        ErrorCode::PermissionDenied
    );
}

#[test]
fn skills_and_script_assets_are_hashed_and_never_executed_or_marked_executable() {
    let (_tmp, _path, root) = root();
    let mut plan = RuntimePlan::empty(RuntimeProvider::ClaudeAcp);
    plan.skills.push(skill());
    let snapshot = root.materialize(plan, Ownership::BatonManaged).unwrap();
    let script = snapshot
        .provider_configuration_root()
        .join("skills/approved-skill/scripts/check.sh");
    assert_eq!(fs::metadata(&script).unwrap().mode() & 0o777, 0o600);
    let entry = snapshot
        .manifest()
        .files
        .iter()
        .find(|f| f.kind == MaterializedKind::SkillScript)
        .unwrap();
    assert_eq!(entry.sha256, sha256(&fs::read(script).unwrap()));
    assert!(!snapshot.directory().join("MUST_NOT_EXECUTE").exists());
    assert!(snapshot
        .provider_configuration_root()
        .join("skills/approved-skill/SKILL.md")
        .is_file());
}

#[test]
fn malicious_skill_paths_and_unknown_native_fields_are_rejected_before_staging() {
    let (_tmp, path, root) = root();
    let mut plan = RuntimePlan::empty(RuntimeProvider::Codex);
    let mut selected = skill();
    selected.assets[0].relative_path = "../escape".into();
    plan.skills.push(selected);
    assert_eq!(
        root.materialize(plan, Ownership::BatonManaged)
            .err()
            .unwrap()
            .code,
        ErrorCode::InvalidPath
    );
    assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
    let mut json = serde_json::to_value(RuntimePlan::empty(RuntimeProvider::Codex)).unwrap();
    json["codex"]["api_key"] = json!("fixture-not-a-key");
    assert_eq!(
        RuntimePlan::decode_record(&serde_json::to_vec(&json).unwrap())
            .err()
            .unwrap()
            .code,
        ErrorCode::Conflict
    );
}

#[test]
fn qoder_unknown_settings_schema_stays_explicit() {
    let (_tmp, _path, root) = root();
    let snapshot = root
        .materialize(
            RuntimePlan::empty(RuntimeProvider::Qoder),
            Ownership::BatonManaged,
        )
        .unwrap();
    assert_eq!(
        snapshot.manifest().claims.native_settings_schema,
        EvidenceState::Unsupported
    );
    assert_eq!(
        fs::read_dir(snapshot.provider_configuration_root())
            .unwrap()
            .count(),
        0
    );
    assert!(!snapshot
        .environment()
        .keys()
        .any(|k| k.to_string_lossy().contains("QODER")));
    let mut plan = RuntimePlan::empty(RuntimeProvider::Qoder);
    plan.skills.push(skill());
    assert_eq!(
        root.materialize(plan, Ownership::BatonManaged)
            .err()
            .unwrap()
            .code,
        ErrorCode::Conflict
    );
}

#[test]
fn claude_configuration_is_explicit_client_metadata_not_global_writeback() {
    let (_tmp, _path, root) = root();
    let mut plan = RuntimePlan::empty(RuntimeProvider::ClaudeAcp);
    plan.claude
        .disabled_project_mcp_names
        .push("unselected-server".into());
    plan.mcp_servers.push(ManagedMcpServer {
        name: "selected-server".into(),
        transport: ManagedMcpTransport::Stdio {
            command: text("/approved/nonsecret-server"),
            arguments: vec![text("--selected")],
        },
        provenance: source(),
    });
    let snapshot = root.materialize(plan, Ownership::BatonManaged).unwrap();
    let settings: Value = serde_json::from_slice(
        &fs::read(snapshot.provider_configuration_root().join("settings.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(settings["enableAllProjectMcpServers"], false);
    assert_eq!(
        settings["disabledMcpjsonServers"],
        json!(["unselected-server"])
    );
    let acp = snapshot.acp_configuration().unwrap();
    assert_eq!(acp["mcpServers"][0]["env"], json!([]));
    assert_eq!(
        acp["_meta"]["claudeCode"]["options"]["settingSources"],
        json!([])
    );
    assert_eq!(acp["_meta"]["claudeCode"]["options"]["settings"], settings);
    assert_eq!(
        acp["_meta"]["claudeCode"]["options"]["allowDangerouslySkipPermissions"],
        false
    );
    assert_eq!(
        acp["_meta"]["claudeCode"]["options"]["strictMcpConfig"],
        true
    );
    assert_eq!(snapshot.claude_setting_sources_override(), Some(&[][..]));
    assert_eq!(
        snapshot.manifest().claims.native_runtime_loading,
        EvidenceState::Unknown
    );
    let preview = serde_json::to_string(snapshot.manifest()).unwrap();
    assert!(!preview.contains("/approved/nonsecret-server"));
    assert!(!preview.contains("--selected"));
}

#[test]
fn mcp_credentials_or_malformed_endpoint_are_not_accepted_as_nonsecret_endpoint() {
    for endpoint in [
        "https://name:token@example.invalid/mcp",
        "https://example.invalid/mcp?key=fixture",
        "https:///missing-host",
        "http://example.invalid/mcp",
    ] {
        let mut plan = RuntimePlan::empty(RuntimeProvider::Codex);
        plan.mcp_servers.push(ManagedMcpServer {
            name: "selected".into(),
            transport: ManagedMcpTransport::Http {
                endpoint: text(endpoint),
            },
            provenance: source(),
        });
        assert_eq!(
            plan.validate().err().unwrap().code,
            ErrorCode::PermissionDenied
        );
    }
}

#[test]
fn native_settings_for_a_different_provider_are_not_silently_ignored() {
    let mut plan = RuntimePlan::empty(RuntimeProvider::CustomAcp);
    plan.codex.model = Some(text("example-model"));
    assert_eq!(plan.validate().err().unwrap().code, ErrorCode::Conflict);
    let mut plan = RuntimePlan::empty(RuntimeProvider::Codex);
    plan.claude.enabled_project_mcp_names.push("server".into());
    assert_eq!(plan.validate().err().unwrap().code, ErrorCode::Conflict);
}

#[test]
fn persisted_descriptor_reopens_and_detects_tampered_bound_identity() {
    let (_tmp, path, root) = root();
    let snapshot = codex(&root);
    let descriptor: SnapshotDescriptor =
        serde_json::from_slice(&serde_json::to_vec(&snapshot.descriptor()).unwrap()).unwrap();
    let reopened = RuntimeRoot::open(&path).unwrap();
    assert_eq!(
        reopened
            .verify_before_launch(&descriptor, RuntimeRequirements::default())
            .unwrap()
            .digest(),
        snapshot.digest()
    );
    let mut wrong = descriptor;
    wrong.snapshot_inode += 1;
    assert_eq!(
        reopened
            .verify_before_launch(&wrong, RuntimeRequirements::default())
            .err()
            .unwrap()
            .code,
        ErrorCode::Conflict
    );
}

#[test]
fn snapshot_paths_are_private_and_manifest_hashes_match_real_bytes() {
    let (_tmp, _path, root) = root();
    let snapshot = codex(&root);
    assert_eq!(
        fs::metadata(snapshot.directory())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    for directory in &snapshot.manifest().directories {
        assert_eq!(
            fs::metadata(snapshot.directory().join(directory))
                .unwrap()
                .mode()
                & 0o777,
            0o700
        );
    }
    for entry in &snapshot.manifest().files {
        let path = snapshot.directory().join(&entry.relative_path);
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        assert_eq!(sha256(&fs::read(path).unwrap()), entry.sha256);
    }
    assert_eq!(
        sha256(&fs::read(snapshot.directory().join(".baton-runtime-manifest.json")).unwrap()),
        snapshot.digest()
    );
}

#[test]
fn selected_source_digest_changes_approved_plan_and_never_reads_a_source_path() {
    let plan = RuntimePlan::empty(RuntimeProvider::CustomAcp);
    let before = plan.plan_digest().unwrap();
    let mut changed = plan.clone();
    changed.source_refs.push(source());
    assert_ne!(before, changed.plan_digest().unwrap());
    let mut invalid = plan;
    invalid.source_refs.push(SourceProvenance {
        source_ref: "/private/native/auth.json".into(),
        ..source()
    });
    assert_eq!(
        invalid.validate().err().unwrap().code,
        ErrorCode::InvalidPath
    );
}
