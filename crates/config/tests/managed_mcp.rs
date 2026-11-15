//! Generated configuration only; never executes an MCP or reads credentials.
use agentbaton_config::{runtime::*, sha256, Ownership};
use std::{fs, os::unix::fs::PermissionsExt};
fn text(v: &str) -> NonSecretText {
    NonSecretText::approved(v.into()).unwrap()
}
fn plan() -> RuntimePlan {
    let mut plan = RuntimePlan::empty(RuntimeProvider::Codex);
    let tools = vec!["read.item".into(), "inspect".into()];
    let provenance = SourceProvenance {
        source_ref: MANAGED_CODEX_MCP_SOURCE.into(),
        revision: "1".into(),
        sha256: managed_codex_mcp_digest(
            "fixture_mcp",
            "/selected/generated-mcp",
            &sha256(b"fixture"),
            &tools,
        )
        .unwrap(),
    };
    plan.source_refs.push(provenance.clone());
    plan.mcp_servers.push(ManagedMcpServer {
        name: "fixture_mcp".into(),
        codex_policy: Some(CodexMcpPolicy {
            executable_sha256: sha256(b"fixture"),
            enabled_tools: tools,
        }),
        transport: ManagedMcpTransport::Stdio {
            command: text("/selected/generated-mcp"),
            arguments: vec![],
        },
        provenance,
    });
    plan
}
#[test]
fn materializes_closed_native_policy_and_preserves_skill_island() {
    let temp = tempfile::tempdir().unwrap();
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let root = RuntimeRoot::open(&temp.path().canonicalize().unwrap()).unwrap();
    let mut p = plan();
    let instruction = "---\nname: demo-skill\ndescription: Fixture\n---\nRead input.\n";
    let provenance = SourceProvenance {
        source_ref: agentbaton_config::capabilities::native::INSTRUCTION_ONLY_SKILL_SOURCE.into(),
        revision: "1".into(),
        sha256: sha256(instruction.as_bytes()),
    };
    p.skills.push(SelectedSkill {
        directory_name: "demo-skill".into(),
        instruction: text(instruction),
        assets: vec![],
        provenance: provenance.clone(),
    });
    p.source_refs.push(provenance);
    p.codex.sandbox = CodexSandbox::ReadOnly;
    p.codex.approval_policy = CodexApproval::Untrusted;
    let snapshot = root
        .materialize(p.clone(), Ownership::BatonManaged)
        .unwrap();
    let native = snapshot.provider_configuration_root().join("config.toml");
    let doc: toml::Value = toml::from_str(&fs::read_to_string(&native).unwrap()).unwrap();
    let m = &doc["mcp_servers"]["fixture_mcp"];
    assert_eq!(m["command"].as_str(), Some("/selected/generated-mcp"));
    assert!(m["args"].as_array().unwrap().is_empty());
    assert_eq!(
        m["enabled_tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["inspect", "read.item"]
    );
    assert_eq!(m["default_tools_approval_mode"].as_str(), Some("prompt"));
    assert_eq!(m["required"].as_bool(), Some(true));
    assert_eq!(m["startup_timeout_sec"].as_integer(), Some(3));
    assert_eq!(m["tool_timeout_sec"].as_integer(), Some(10));
    assert_eq!(doc["sandbox_mode"].as_str(), Some("read-only"));
    assert_eq!(doc["approval_policy"].as_str(), Some("untrusted"));
    assert_eq!(
        fs::read_to_string(
            snapshot
                .directory()
                .join("state/home/.agents/skills/demo-skill/SKILL.md")
        )
        .unwrap(),
        instruction
    );
    p.mcp_servers.clear();
    p.source_refs
        .retain(|s| s.source_ref != MANAGED_CODEX_MCP_SOURCE);
    root.materialize(p, Ownership::BatonManaged).unwrap();
    root.verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
        .unwrap();
    fs::write(native, "tampered").unwrap();
    assert!(root
        .verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
        .is_err());
}
#[test]
fn rejects_unbound_or_expanded_profiles() {
    let mut variants = vec![];
    let mut p = plan();
    p.source_refs.clear();
    variants.push(p);
    let mut p = plan();
    p.source_refs.push(p.source_refs[0].clone());
    variants.push(p);
    let mut p = plan();
    p.mcp_servers[0].codex_policy = None;
    variants.push(p);
    let mut p = plan();
    p.mcp_servers[0].provenance.revision = "2".into();
    variants.push(p);
    let mut p = plan();
    p.mcp_servers.push(p.mcp_servers[0].clone());
    variants.push(p);
    let mut p = plan();
    p.codex.model = Some(text("other-model"));
    variants.push(p);
    let mut p = plan();
    p.provider = RuntimeProvider::ClaudeAcp;
    variants.push(p);
    let mut p = plan();
    p.mcp_servers[0].transport = ManagedMcpTransport::Http {
        endpoint: text("https://example.com/mcp"),
    };
    variants.push(p);
    let mut p = plan();
    if let ManagedMcpTransport::Stdio { arguments, .. } = &mut p.mcp_servers[0].transport {
        arguments.push(text("--unsafe"));
    }
    variants.push(p);
    for p in variants {
        assert!(managed_codex_mcp_metadata(&p).is_err());
        assert!(p.validate().is_err());
    }
    let mut value = serde_json::to_value(plan()).unwrap();
    value["mcp_servers"][0]["codex_policy"]["env"] = serde_json::json!({"TOKEN":"fixture"});
    assert!(RuntimePlan::decode_record(&serde_json::to_vec(&value).unwrap()).is_err());
}
#[test]
fn binding_is_finite_and_order_independent() {
    let hash = sha256(b"fixture");
    let tools = vec!["a".into(), "b".into()];
    assert_eq!(
        managed_codex_mcp_digest("fixture", "/bin/fixture", &hash, &tools).unwrap(),
        managed_codex_mcp_digest("fixture", "/bin/fixture", &hash, &["b".into(), "a".into()])
            .unwrap()
    );
    for command in [
        "relative",
        "/",
        "/bin/../fixture",
        "/bin/./fixture",
        "//bin/fixture",
        "/bin/fixture/",
        "/bin/fixture\n",
    ] {
        assert!(managed_codex_mcp_digest("fixture", command, &hash, &tools).is_err());
    }
    for tools in [
        vec![],
        vec!["a".into(); 2],
        vec!["x:y".into()],
        vec!["x".repeat(65)],
        (0..17).map(|i| format!("t{i}")).collect(),
    ] {
        assert!(managed_codex_mcp_digest("fixture", "/bin/fixture", &hash, &tools).is_err());
    }
    for name in ["Upper", "dot.name", "../escape", ""] {
        assert!(managed_codex_mcp_digest(name, "/bin/fixture", &hash, &tools).is_err());
    }
}
#[test]
fn legacy_serialization_and_digest_are_unchanged() {
    let raw = concat!(
        r#"{"provider":"codex","codex":{"model":null,"sandbox":"workspace-write","approval_policy":"on-request"},"claude":{"enabled_project_mcp_names":[],"disabled_project_mcp_names":[]},"mcp_servers":[{"name":"legacy","transport":{"Stdio":{"command":"/legacy/mcp","arguments":[]}},"provenance":{"source_ref":"legacy-source","revision":"old","sha256":""#,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        r#""}}],"skills":[],"source_refs":[]}"#
    );
    let plan = RuntimePlan::decode_record(raw.as_bytes()).unwrap();
    assert_eq!(serde_json::to_string(&plan).unwrap(), raw);
    assert!(managed_codex_mcp_metadata(&plan).unwrap().is_none());
    let historical=format!("{{\"materializer_revision\":\"agentbaton-managed-runtime-v1\",\"native_contract_sha256\":\"d98f7bbe84dfe375d05ad5a8249a5368d82989551e38c6a4babe35356b92c14e\",\"plan\":{raw}}}");
    assert_eq!(plan.plan_digest().unwrap(), sha256(historical.as_bytes()));
}
