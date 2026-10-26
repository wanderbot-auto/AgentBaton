use agentbaton_config::{capabilities::*, sha256, ErrorCode};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::PathBuf,
};
struct Fixture {
    _temp: tempfile::TempDir,
    path: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().canonicalize().unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self { _temp: temp, path }
    }
    fn write(&self, name: &str, value: &Value) -> Vec<u8> {
        let bytes = serde_json::to_vec(value).unwrap();
        fs::write(self.path.join(name), &bytes).unwrap();
        fs::set_permissions(self.path.join(name), fs::Permissions::from_mode(0o600)).unwrap();
        bytes
    }
    fn root(&self) -> SourceRoot {
        SourceRoot::open_approved_private(&self.path).unwrap()
    }
    fn source(&self, id: &str, kind: SourceKind, enabled: bool) -> SourceSnapshot {
        self.write(id, &inventory(enabled));
        self.root()
            .read_selected(descriptor(id, kind), std::path::Path::new(id))
            .unwrap()
    }
}
fn provider() -> ProviderRevision {
    ProviderRevision {
        provider_id: "codex".into(),
        version: "test-fixture-v1".into(),
    }
}
fn descriptor(id: &str, kind: SourceKind) -> SourceDescriptor {
    SourceDescriptor {
        source_id: id.into(),
        kind,
        locator_ref: format!("fixture:{id}"),
        provider: provider(),
        mandatory: false,
    }
}
fn inventory(enabled: bool) -> Value {
    json!({"schema_version":1,"provider":provider(),"capabilities":[{"id":"review","name":"Review","enabled":enabled,
      "definition":{"kind":"skill","metadata":{"content_ref":"skill:review","dependency_refs":["mcp:tools"],"script_refs":["script:check"],"compatible_with":[provider()]}}}]})
}
fn policy(ids: &[&str]) -> ResolutionPolicy {
    ResolutionPolicy {
        provider: provider(),
        rule_revision: "fixture-policy-v1".into(),
        evidence_ref: "fixture:precedence-test".into(),
        lowest_to_highest: ids.iter().map(|id| (*id).into()).collect(),
        merge_rule: MergeRule::ReplaceWholeCapability,
    }
}
fn snapshot() -> CapabilitySnapshot {
    let fixture = Fixture::new();
    CapabilitySnapshot::resolve(
        policy(&["managed"]),
        vec![fixture.source("managed", SourceKind::Managed, true)],
    )
    .unwrap()
}
fn binding(snapshot: &CapabilitySnapshot) -> SessionBinding {
    SessionBinding {
        session_id: "session-A".into(),
        provider: provider(),
        config_snapshot_digest: snapshot.digest().into(),
    }
}
fn observation(snapshot: &CapabilitySnapshot, kind: ObservationKind) -> Observation {
    Observation {
        source_id: "managed".into(),
        capability_id: "review".into(),
        kind,
        origin: if kind == ObservationKind::DiagnosticSucceeded {
            ObservationOrigin::IndependentDiagnostic
        } else {
            ObservationOrigin::NativeRuntime
        },
        provider: provider(),
        snapshot_digest: snapshot.digest().into(),
        source_digest: snapshot.sources()[0].digest().into(),
        session_id: if kind == ObservationKind::DiagnosticSucceeded {
            None
        } else {
            Some("session-A".into())
        },
        evidence_ref: "fixture:observed-event".into(),
        observed_at_unix: 100,
        expires_at_unix: 200,
    }
}
#[test]
fn explicit_provider_policy_retains_every_source_and_override_chain() {
    let fixture = Fixture::new();
    let managed = fixture.source("managed", SourceKind::Managed, true);
    let project = fixture.source("project", SourceKind::Project, false);
    let plugin = fixture.source("plugin", SourceKind::Plugin, true);
    let resolved = CapabilitySnapshot::resolve(
        policy(&["plugin", "managed", "project"]),
        vec![managed, project, plugin],
    )
    .unwrap();
    let preview = resolved.preview();
    assert_eq!(preview.candidates.len(), 3);
    assert_eq!(preview.candidates[0].source_id, "plugin");
    assert_eq!(
        preview.candidates[0].replaced_by_source.as_deref(),
        Some("managed")
    );
    assert_eq!(
        preview.candidates[1].replaced_by_source.as_deref(),
        Some("project")
    );
    assert!(preview.candidates[2].winner);
    assert!(!preview.candidates[2].enabled);
    assert!(preview.inferred_resolution);
    assert_eq!(
        preview.candidates[0].compatibility,
        Compatibility::DeclaredCompatible
    );
}
#[test]
fn unsupported_merge_and_unspecified_or_duplicate_precedence_fail_closed() {
    let fixture = Fixture::new();
    let sources = vec![
        fixture.source("managed", SourceKind::Managed, true),
        fixture.source("native", SourceKind::Native, false),
    ];
    let mut rejected = policy(&["managed", "native"]);
    rejected.merge_rule = MergeRule::RejectDuplicates;
    assert!(CapabilitySnapshot::resolve(rejected, sources.clone()).is_err());
    assert!(CapabilitySnapshot::resolve(policy(&["managed"]), sources.clone()).is_err());
    assert!(
        CapabilitySnapshot::resolve(policy(&["managed", "native", "native"]), sources).is_err()
    );
    let unsupported = json!({"provider":provider(),"rule_revision":"v1","evidence_ref":"fixture:ref","lowest_to_highest":[],"merge_rule":"deep_merge_unknown_fields"});
    assert!(serde_json::from_value::<ResolutionPolicy>(unsupported).is_err());
}
#[test]
fn mandatory_organization_constraints_cannot_be_overridden_by_project() {
    let fixture = Fixture::new();
    fixture.write("organization", &inventory(false));
    let mut descriptor = descriptor("organization", SourceKind::Organization);
    descriptor.mandatory = true;
    let organization = fixture
        .root()
        .read_selected(descriptor, std::path::Path::new("organization"))
        .unwrap();
    let project = fixture.source("project", SourceKind::Project, true);
    assert!(CapabilitySnapshot::resolve(
        policy(&["organization", "project"]),
        vec![organization, project]
    )
    .is_err());
}
#[test]
fn provider_revision_and_source_descriptor_are_part_of_snapshot_identity() {
    let fixture = Fixture::new();
    let source = fixture.source("managed", SourceKind::Managed, true);
    let snapshot_a =
        CapabilitySnapshot::resolve(policy(&["managed"]), vec![source.clone()]).unwrap();
    let changed = fixture
        .root()
        .read_selected(
            descriptor("managed", SourceKind::Native),
            std::path::Path::new("managed"),
        )
        .unwrap();
    let snapshot_b = CapabilitySnapshot::resolve(policy(&["managed"]), vec![changed]).unwrap();
    assert_ne!(snapshot_a.digest(), snapshot_b.digest());
    let mut wrong = policy(&["managed"]);
    wrong.provider.version = "another-version".into();
    assert!(CapabilitySnapshot::resolve(wrong, vec![source]).is_err());
}
#[test]
fn unknown_values_remain_unmodified_but_never_appear_in_safe_preview() {
    let fixture = Fixture::new();
    let mut original = inventory(true);
    original["unknown_vendor_extension"] = json!({"nested":"DO-NOT-EXPORT-UNKNOWN-VALUE"});
    original["capabilities"][0]["unknown_option"] = "DO-NOT-EXPORT-UNKNOWN-CAPABILITY".into();
    let bytes = fixture.write("managed", &original);
    let source = fixture
        .root()
        .read_selected(
            descriptor("managed", SourceKind::Managed),
            std::path::Path::new("managed"),
        )
        .unwrap();
    assert_eq!(source.digest(), sha256(&bytes));
    assert!(source.preserves_original_hash());
    let snapshot = CapabilitySnapshot::resolve(policy(&["managed"]), vec![source]).unwrap();
    let preview = serde_json::to_string(&snapshot.preview()).unwrap();
    assert!(preview.contains("unknown_vendor_extension"));
    assert!(preview.contains("unknown_option"));
    assert!(!preview.contains("DO-NOT-EXPORT"));
    assert_eq!(fs::read(fixture.path.join("managed")).unwrap(), bytes);
}
#[test]
fn source_drift_yields_new_snapshot_and_old_snapshot_remains_immutable() {
    let fixture = Fixture::new();
    let root = fixture.root();
    let source = fixture.source("managed", SourceKind::Managed, true);
    let original = CapabilitySnapshot::resolve(policy(&["managed"]), vec![source]).unwrap();
    let digest = original.digest().to_string();
    assert!(root.verify_unchanged(&original.sources()[0]).unwrap());
    fixture.write("managed", &inventory(false));
    assert!(!root.verify_unchanged(&original.sources()[0]).unwrap());
    let source = root
        .read_selected(
            descriptor("managed", SourceKind::Managed),
            std::path::Path::new("managed"),
        )
        .unwrap();
    let changed = CapabilitySnapshot::resolve(policy(&["managed"]), vec![source]).unwrap();
    assert_ne!(changed.digest(), original.digest());
    assert_eq!(original.digest(), digest);
    assert!(original.preview().candidates[0].enabled);
    assert!(!changed.preview().candidates[0].enabled);
}
#[test]
fn static_configuration_and_successful_diagnostics_never_prove_session_use() {
    let snapshot = snapshot();
    let session = binding(&snapshot);
    let initial = snapshot
        .assess("managed", "review", &session, &[], 150)
        .unwrap();
    assert!(initial.configured && initial.enabled);
    assert_eq!(initial.available_to_session, EvidenceState::NotObserved);
    let diagnostic = observation(&snapshot, ObservationKind::DiagnosticSucceeded);
    let assessment = snapshot
        .assess("managed", "review", &session, &[diagnostic], 150)
        .unwrap();
    assert_eq!(assessment.diagnostic, EvidenceState::Observed);
    assert_eq!(assessment.available_to_session, EvidenceState::NotObserved);
    assert_eq!(assessment.content_loaded, EvidenceState::NotObserved);
    assert_eq!(assessment.execution_used, EvidenceState::NotObserved);
    let mut forged = observation(&snapshot, ObservationKind::AvailableToSession);
    forged.origin = ObservationOrigin::IndependentDiagnostic;
    let assessment = snapshot
        .assess("managed", "review", &session, &[forged], 150)
        .unwrap();
    assert_eq!(assessment.available_to_session, EvidenceState::Mismatch);
    assert_eq!(assessment.rejected_observations, 1);
}
#[test]
fn native_observation_requires_exact_session_provider_snapshot_source_and_time() {
    let snapshot = snapshot();
    let session = binding(&snapshot);
    let valid = observation(&snapshot, ObservationKind::ContentLoaded);
    assert_eq!(
        snapshot
            .assess(
                "managed",
                "review",
                &session,
                std::slice::from_ref(&valid),
                150
            )
            .unwrap()
            .content_loaded,
        EvidenceState::Observed
    );
    assert_eq!(
        snapshot
            .assess(
                "managed",
                "review",
                &session,
                std::slice::from_ref(&valid),
                200
            )
            .unwrap()
            .content_loaded,
        EvidenceState::Stale
    );
    for variant in 0..5 {
        let mut invalid = valid.clone();
        match variant {
            0 => invalid.session_id = Some("session-B".into()),
            1 => invalid.provider.version = "other".into(),
            2 => invalid.snapshot_digest = "other".into(),
            3 => invalid.source_digest = "other".into(),
            _ => invalid.observed_at_unix = 160,
        }
        assert_eq!(
            snapshot
                .assess("managed", "review", &session, &[invalid], 150)
                .unwrap()
                .content_loaded,
            EvidenceState::Mismatch
        );
    }
    let other_session = SessionBinding {
        config_snapshot_digest: "new-config".into(),
        ..session
    };
    assert_eq!(
        snapshot
            .assess("managed", "review", &other_session, &[valid], 150)
            .unwrap()
            .content_loaded,
        EvidenceState::Mismatch
    );
}
#[test]
fn skill_discovery_lists_scripts_dependencies_and_compatibility_without_executing() {
    let fixture = Fixture::new();
    let mut document = inventory(true);
    document["capabilities"][0]["definition"]["metadata"]["compatible_with"] = json!([]);
    let bytes = fixture.write("native", &document);
    let source = fixture
        .root()
        .read_selected(
            descriptor("native", SourceKind::Native),
            std::path::Path::new("native"),
        )
        .unwrap();
    let snapshot = CapabilitySnapshot::resolve(policy(&["native"]), vec![source]).unwrap();
    let candidate = &snapshot.preview().candidates[0];
    assert_eq!(candidate.compatibility, Compatibility::Unknown);
    assert_eq!(
        candidate.skill.as_ref().unwrap().script_refs,
        vec!["script:check"]
    );
    assert_eq!(
        candidate.skill.as_ref().unwrap().dependency_refs,
        vec!["mcp:tools"]
    );
    assert_eq!(fs::read(fixture.path.join("native")).unwrap(), bytes);
    assert_eq!(fs::read_dir(&fixture.path).unwrap().count(), 1);
}
#[test]
fn mcp_target_and_credentials_are_refs_and_inline_values_are_rejected() {
    let fixture = Fixture::new();
    let mut document = inventory(true);
    document["capabilities"][0]["definition"] = json!({"kind":"mcp","metadata":{"target":{"transport":"https","endpoint_ref":"endpoint:tools"},"credential_refs":["keychain:tools"],"tools_metadata_ref":"tools:list"}});
    fixture.write("managed", &document);
    let source = fixture
        .root()
        .read_selected(
            descriptor("managed", SourceKind::Managed),
            std::path::Path::new("managed"),
        )
        .unwrap();
    let snapshot = CapabilitySnapshot::resolve(policy(&["managed"]), vec![source]).unwrap();
    let preview = snapshot.preview();
    let metadata = preview.candidates[0].mcp.as_ref().unwrap();
    assert_eq!(metadata.credential_refs, vec!["keychain:tools"]);
    document["capabilities"][0]["definition"]["metadata"]["env"] =
        json!({"TOKEN":"should-not-be-accepted"});
    fixture.write("managed", &document);
    assert!(fixture
        .root()
        .read_selected(
            descriptor("managed", SourceKind::Managed),
            std::path::Path::new("managed")
        )
        .is_err());
}
#[test]
fn selected_roots_reject_symlinks_traversal_and_foreign_source_revalidation() {
    let fixture = Fixture::new();
    let outside = Fixture::new();
    let source = outside.source("managed", SourceKind::Managed, true);
    let root = fixture.root();
    assert!(matches!(root.verify_unchanged(&source),Err(e) if e.code==ErrorCode::Conflict));
    symlink(outside.path.join("managed"), fixture.path.join("link")).unwrap();
    for path in ["../managed", "/etc/passwd", "link"] {
        assert!(root
            .read_selected(
                descriptor("managed", SourceKind::Managed),
                std::path::Path::new(path)
            )
            .is_err());
    }
}
#[test]
fn empty_or_malformed_session_associations_never_prove_runtime_evidence() {
    let snapshot = snapshot();
    let session = binding(&snapshot);
    let valid = observation(&snapshot, ObservationKind::AvailableToSession);
    for invalid_id in [String::new(), "bad session\n".into(), "x".repeat(257)] {
        let mut invalid_binding = session.clone();
        invalid_binding.session_id = invalid_id.clone();
        let mut invalid_observation = valid.clone();
        invalid_observation.session_id = Some(invalid_id);
        assert!(matches!(
            snapshot.assess("managed", "review", &invalid_binding, std::slice::from_ref(&invalid_observation), 150),
            Err(error) if error.code == ErrorCode::Conflict
        ));
        let assessment = snapshot
            .assess("managed", "review", &session, &[invalid_observation], 150)
            .unwrap();
        assert_eq!(assessment.available_to_session, EvidenceState::Mismatch);
        assert_eq!(assessment.rejected_observations, 1);
    }
    let mut unknown = valid;
    unknown.session_id = None;
    assert_eq!(
        snapshot
            .assess("managed", "review", &session, &[unknown], 150)
            .unwrap()
            .available_to_session,
        EvidenceState::Mismatch
    );
}
