use agentbaton_config::capabilities::plugin_trust::*;
use agentbaton_config::capabilities::{
    CapabilitySnapshot, Compatibility, MergeRule, ProviderRevision, ResolutionPolicy,
    SessionBinding, SourceDescriptor, SourceKind, SourceRoot,
};
use agentbaton_config::{sha256, ErrorCode};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::{Path, PathBuf},
};

const LICENSE: &str = "LicenseRef-Fixture-Only";
const MANIFEST: &str = "approved-plugin.json";
const ENTRY: &str = "approved-entry.sh";
const ENTRY_REF: &str = "fixture:entry";
const PRIVATE_MARKER: &str = "synthetic-sensitive-body-marker";

struct Fixture {
    _dir: tempfile::TempDir,
    path: PathBuf,
    descriptor: SourceDescriptor,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self {
            _dir: dir,
            path,
            descriptor: SourceDescriptor {
                source_id: "fixture-plugin".into(),
                kind: SourceKind::Plugin,
                locator_ref: "registry:local-fixture".into(),
                provider: ProviderRevision {
                    provider_id: "fixture-provider".into(),
                    version: "test-v1".into(),
                },
                mandatory: false,
            },
        }
    }

    fn source(&self) -> SourceRoot {
        SourceRoot::open_approved_private(&self.path).unwrap()
    }

    fn selection(&self, code: bool) -> ApprovedPluginSource<'_> {
        ApprovedPluginSource {
            descriptor: &self.descriptor,
            manifest: Path::new(MANIFEST),
            license_ref: LICENSE,
            entrypoint: code.then_some(ApprovedEntrypoint {
                reference: ENTRY_REF,
                relative: Path::new(ENTRY),
            }),
        }
    }

    fn manifest(&self, code: bool) -> Value {
        json!({
            "schema_version": 1,
            "plugin_id": self.descriptor.source_id,
            "source_ref": self.descriptor.locator_ref,
            "version": "1.2.3",
            "license_ref": LICENSE,
            "kind": if code { "code" } else { "declarative" },
            "compatible_with": [self.descriptor.provider],
            "permissions": if code { json!(["execute", "read_workspace"]) } else { json!([]) },
            "entrypoint_ref": code.then_some(ENTRY_REF),
        })
    }

    fn write(&self, name: &str, bytes: &[u8]) {
        let file = self.path.join(name);
        fs::write(&file, bytes).unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    }

    fn write_manifest(&self, value: &Value) -> Vec<u8> {
        let bytes = serde_json::to_vec(value).unwrap();
        self.write(MANIFEST, &bytes);
        bytes
    }

    fn write_entry(&self) -> Vec<u8> {
        let code = format!(
            "#!/bin/sh\n# {PRIVATE_MARKER}\n: > '{}'/ran-plugin\n",
            self.path.display()
        );
        let bytes = code.into_bytes();
        self.write(ENTRY, &bytes);
        fs::set_permissions(self.path.join(ENTRY), fs::Permissions::from_mode(0o700)).unwrap();
        bytes
    }

    fn import_error(&self, manifest: Value, code: bool, expected: ImportError) {
        self.write_manifest(&manifest);
        assert_eq!(
            self.source().import_plugin(self.selection(code)).err(),
            Some(expected)
        );
    }
}

fn evidence<'a>(
    import: &'a PluginImport,
    enforcement: Option<&'a EnforcedProfile>,
) -> RuntimeEvidence<'a> {
    let preview = import.preview();
    RuntimeEvidence {
        review_digest: &preview.review_digest,
        provider: &preview.provider,
        compatibility: RuntimeCompatibility::VerifiedCompatible,
        enforcement,
    }
}

fn profile() -> EnforcedProfile {
    EnforcedProfile {
        constraint_ref: "fixture:isolated-runtime-v1".into(),
        isolation: Isolation::OsSandbox,
        effective_permissions: [PluginPermission::Execute, PluginPermission::ReadWorkspace].into(),
    }
}

fn approval<'a>(import: &'a PluginImport, profile: &'a EnforcedProfile) -> CodeApproval<'a> {
    CodeApproval {
        review_digest: &import.preview().review_digest,
        enforced_profile: profile,
    }
}

#[test]
fn declarative_import_pins_actual_local_bytes_and_checks_independent_compatibility() {
    let fixture = Fixture::new();
    let manifest = fixture.write_manifest(&fixture.manifest(false));
    let root = fixture.source();
    let import = root.import_plugin(fixture.selection(false)).unwrap();
    let preview = import.preview();
    assert_eq!(preview.trust, PluginTrust::Declarative);
    assert_eq!(preview.manifest_digest, sha256(&manifest));
    assert_eq!(preview.license_ref, LICENSE);
    assert_eq!(
        preview.declared_compatibility,
        Compatibility::DeclaredCompatible
    );
    assert!(preview.requested_permissions.is_empty() && preview.entrypoint_digest.is_none());
    let unknown = RuntimeEvidence {
        compatibility: RuntimeCompatibility::Unknown,
        ..evidence(&import, None)
    };
    assert_eq!(
        root.check_plugin_enablement(&import, unknown, None),
        EnableDecision::Blocked(EnableBlock::RuntimeCompatibilityUnknown)
    );
    let incompatible = RuntimeEvidence {
        compatibility: RuntimeCompatibility::VerifiedIncompatible,
        ..evidence(&import, None)
    };
    assert_eq!(
        root.check_plugin_enablement(&import, incompatible, None),
        EnableDecision::Blocked(EnableBlock::RuntimeIncompatible)
    );
    assert_eq!(
        root.check_plugin_enablement(&import, evidence(&import, None), None),
        EnableDecision::Eligible(PluginTrust::Declarative)
    );
    assert_eq!(fs::read(fixture.path.join(MANIFEST)).unwrap(), manifest);
    assert_eq!(fs::read_dir(fixture.path).unwrap().count(), 1);
}

#[test]
fn code_import_and_approval_preview_never_execute_or_expose_selected_bytes() {
    let fixture = Fixture::new();
    let manifest = fixture.write_manifest(&fixture.manifest(true));
    let code = fixture.write_entry();
    let root = fixture.source();
    let import = root.import_plugin(fixture.selection(true)).unwrap();
    let preview = import.preview();
    assert_eq!(preview.trust, PluginTrust::Code);
    assert_eq!(preview.manifest_digest, sha256(&manifest));
    assert_eq!(
        preview.entrypoint_digest.as_deref(),
        Some(sha256(&code).as_str())
    );
    let profile = profile();
    assert_eq!(
        root.check_plugin_enablement(&import, evidence(&import, Some(&profile)), None),
        EnableDecision::Blocked(EnableBlock::CodeReviewRequired)
    );
    assert_eq!(
        root.check_plugin_enablement(
            &import,
            evidence(&import, Some(&profile)),
            Some(approval(&import, &profile))
        ),
        EnableDecision::Eligible(PluginTrust::Code)
    );
    assert!(!fixture.path.join("ran-plugin").exists());
    assert_eq!(fs::read(fixture.path.join(ENTRY)).unwrap(), code);
    let exported = format!("{:?} {}", preview, serde_json::to_string(preview).unwrap());
    for private in [
        fixture.path.to_str().unwrap(),
        MANIFEST,
        ENTRY,
        PRIVATE_MARKER,
        "#!/bin/sh",
    ] {
        assert!(!exported.contains(private));
    }
}

#[test]
fn declarations_cannot_supply_trust_activation_or_undeclared_permissions() {
    let fixture = Fixture::new();
    fixture.write_entry();
    for (key, value) in [
        ("enabled", json!(true)),
        ("trust", json!("approved")),
        ("permissions", json!(["execute", "credential_access"])),
    ] {
        let mut manifest = fixture.manifest(true);
        manifest[key] = value;
        fixture.import_error(manifest, true, ImportError::InvalidManifest);
    }
    let mut downgraded = fixture.manifest(true);
    downgraded["kind"] = json!("declarative");
    fixture.import_error(downgraded, true, ImportError::ContradictoryDeclaration);
    let mut hidden_permission = fixture.manifest(false);
    hidden_permission["permissions"] = json!(["network"]);
    fixture.import_error(
        hidden_permission,
        false,
        ImportError::ContradictoryDeclaration,
    );
    let mut missing_execute = fixture.manifest(true);
    missing_execute["permissions"] = json!(["read_workspace"]);
    fixture.import_error(missing_execute, true, ImportError::ContradictoryDeclaration);
    let mut duplicates = fixture.manifest(true);
    duplicates["permissions"] = json!(["execute", "execute"]);
    fixture.import_error(duplicates, true, ImportError::InvalidPermissions);
    fixture.import_error(
        fixture.manifest(false),
        true,
        ImportError::EntrypointMismatch,
    );
    assert!(!fixture.path.join("ran-plugin").exists());
}

#[test]
fn source_schema_version_license_and_compatibility_metadata_are_checked() {
    let fixture = Fixture::new();
    for (key, value, reason) in [
        ("schema_version", json!(2), ImportError::UnsupportedSchema),
        (
            "plugin_id",
            json!("other-plugin"),
            ImportError::SourceMismatch,
        ),
        (
            "source_ref",
            json!("registry:other"),
            ImportError::SourceMismatch,
        ),
        ("version", json!("01.2.3"), ImportError::InvalidVersion),
        ("version", json!("../v1"), ImportError::InvalidVersion),
        (
            "license_ref",
            json!("LicenseRef-Different"),
            ImportError::LicenseMismatch,
        ),
        (
            "compatible_with",
            json!([fixture.descriptor.provider, fixture.descriptor.provider]),
            ImportError::InvalidCompatibility,
        ),
        (
            "compatible_with",
            json!([{"provider_id":"/private/synthetic","version":"v1"}]),
            ImportError::InvalidCompatibility,
        ),
    ] {
        let mut manifest = fixture.manifest(false);
        manifest[key] = value;
        fixture.import_error(manifest, false, reason);
    }
    for (compatible_with, declared, reason) in [
        (
            json!([]),
            Compatibility::Unknown,
            EnableBlock::CompatibilityUndeclared,
        ),
        (
            json!([{"provider_id":"fixture-provider","version":"different"}]),
            Compatibility::DeclaredIncompatible,
            EnableBlock::DeclaredIncompatible,
        ),
    ] {
        let mut manifest = fixture.manifest(false);
        manifest["compatible_with"] = compatible_with;
        fixture.write_manifest(&manifest);
        let root = fixture.source();
        let import = root.import_plugin(fixture.selection(false)).unwrap();
        assert_eq!(import.preview().declared_compatibility, declared);
        assert_eq!(
            root.check_plugin_enablement(&import, evidence(&import, None), None),
            EnableDecision::Blocked(reason)
        );
    }
}

#[test]
fn selected_files_enforce_private_local_boundaries_and_fixed_entrypoint() {
    let fixture = Fixture::new();
    let root = fixture.source();
    fixture.write_manifest(&fixture.manifest(true));
    fixture.write_entry();
    assert_eq!(
        root.import_plugin(fixture.selection(false)).err(),
        Some(ImportError::EntrypointMismatch)
    );
    let some_code = fixture.selection(true);
    let other_ref = ApprovedEntrypoint {
        reference: "fixture:other",
        relative: Path::new(ENTRY),
    };
    assert_eq!(
        root.import_plugin(ApprovedPluginSource {
            entrypoint: Some(other_ref),
            ..some_code
        })
        .err(),
        Some(ImportError::EntrypointMismatch)
    );
    let forged_relative = ApprovedEntrypoint {
        reference: ENTRY_REF,
        relative: Path::new("../forged-entry.sh"),
    };
    assert_eq!(
        root.import_plugin(ApprovedPluginSource {
            entrypoint: Some(forged_relative),
            ..some_code
        })
        .err(),
        Some(ImportError::SourceRead(ErrorCode::InvalidPath))
    );
    let absolute = fixture.path.join(MANIFEST);
    assert_eq!(
        root.import_plugin(ApprovedPluginSource {
            manifest: &absolute,
            ..some_code
        })
        .err(),
        Some(ImportError::SourceRead(ErrorCode::InvalidPath))
    );
    symlink(ENTRY, fixture.path.join("linked-entry")).unwrap();
    let symlinked = ApprovedEntrypoint {
        reference: ENTRY_REF,
        relative: Path::new("linked-entry"),
    };
    assert_eq!(
        root.import_plugin(ApprovedPluginSource {
            entrypoint: Some(symlinked),
            ..some_code
        })
        .err(),
        Some(ImportError::SourceRead(ErrorCode::PermissionDenied))
    );
    fixture.write(ENTRY, b"");
    assert_eq!(
        root.import_plugin(some_code).err(),
        Some(ImportError::EmptyEntrypoint)
    );
    fs::set_permissions(
        fixture.path.join(MANIFEST),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    assert_eq!(
        root.import_plugin(some_code).err(),
        Some(ImportError::ExecutableManifest)
    );
    fs::set_permissions(
        fixture.path.join(MANIFEST),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    fs::set_permissions(fixture.path.join(ENTRY), fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        root.import_plugin(some_code).err(),
        Some(ImportError::SourceRead(ErrorCode::PermissionDenied))
    );
}

#[test]
fn old_approval_cannot_follow_changed_manifest_entrypoint_permissions_or_replacement() {
    let fixture = Fixture::new();
    let script = fixture.write_entry();
    let mut manifest = fixture.manifest(true);
    fixture.write_manifest(&manifest);
    let root = fixture.source();
    let original = root.import_plugin(fixture.selection(true)).unwrap();
    let profile = profile();
    let mut edited = script.clone();
    edited.extend_from_slice(b"# new build\n");
    fixture.write(ENTRY, &edited);
    assert_eq!(
        root.check_plugin_enablement(
            &original,
            evidence(&original, Some(&profile)),
            Some(approval(&original, &profile))
        ),
        EnableDecision::Blocked(EnableBlock::SourceChanged)
    );
    let current = root.import_plugin(fixture.selection(true)).unwrap();
    assert_ne!(
        original.preview().entrypoint_digest,
        current.preview().entrypoint_digest
    );
    assert_eq!(
        root.check_plugin_enablement(
            &current,
            evidence(&current, Some(&profile)),
            Some(approval(&original, &profile))
        ),
        EnableDecision::Blocked(EnableBlock::ApprovalMismatch)
    );
    manifest["version"] = json!("1.2.4");
    manifest["permissions"] = json!(["execute", "read_workspace", "network"]);
    fixture.write_manifest(&manifest);
    assert_eq!(
        root.check_plugin_enablement(
            &current,
            evidence(&current, Some(&profile)),
            Some(approval(&current, &profile))
        ),
        EnableDecision::Blocked(EnableBlock::SourceChanged)
    );
    let upgraded = root.import_plugin(fixture.selection(true)).unwrap();
    let mut new_profile = profile.clone();
    new_profile
        .effective_permissions
        .insert(PluginPermission::Network);
    assert_eq!(
        root.check_plugin_enablement(
            &upgraded,
            evidence(&upgraded, Some(&new_profile)),
            Some(approval(&current, &profile))
        ),
        EnableDecision::Blocked(EnableBlock::ApprovalMismatch)
    );
    // A same-byte replacement still changes the selected file identity.
    fixture.write("replacement", &edited);
    fs::rename(fixture.path.join("replacement"), fixture.path.join(ENTRY)).unwrap();
    let replaced = root.import_plugin(fixture.selection(true)).unwrap();
    assert_eq!(
        replaced.preview().entrypoint_digest,
        upgraded.preview().entrypoint_digest
    );
    assert_ne!(
        replaced.preview().review_digest,
        upgraded.preview().review_digest
    );
    assert!(!fixture.path.join("ran-plugin").exists());
}

#[test]
fn same_contents_at_a_different_approved_root_cannot_reuse_approval() {
    let first = Fixture::new();
    let second = Fixture::new();
    let manifest = first.write_manifest(&first.manifest(true));
    let code = first.write_entry();
    second.write(MANIFEST, &manifest);
    second.write(ENTRY, &code);
    let first_root = first.source();
    let second_root = second.source();
    let prior = first_root.import_plugin(first.selection(true)).unwrap();
    let current = second_root.import_plugin(second.selection(true)).unwrap();
    assert_eq!(
        prior.preview().manifest_digest,
        current.preview().manifest_digest
    );
    assert_eq!(
        prior.preview().entrypoint_digest,
        current.preview().entrypoint_digest
    );
    let profile = profile();
    assert_eq!(
        second_root.check_plugin_enablement(
            &prior,
            evidence(&prior, Some(&profile)),
            Some(approval(&prior, &profile))
        ),
        EnableDecision::Blocked(EnableBlock::SourceChanged)
    );
    assert_eq!(
        second_root.check_plugin_enablement(
            &current,
            evidence(&current, Some(&profile)),
            Some(approval(&prior, &profile))
        ),
        EnableDecision::Blocked(EnableBlock::ApprovalMismatch)
    );
}

#[test]
fn code_approval_must_match_runtime_facts_permissions_and_constraints() {
    let fixture = Fixture::new();
    fixture.write_manifest(&fixture.manifest(true));
    fixture.write_entry();
    let root = fixture.source();
    let import = root.import_plugin(fixture.selection(true)).unwrap();
    let profile = profile();
    assert_eq!(
        root.check_plugin_enablement(
            &import,
            evidence(&import, None),
            Some(approval(&import, &profile))
        ),
        EnableDecision::Blocked(EnableBlock::EnforcementUnknown)
    );
    let other_provider = ProviderRevision {
        version: "other-generation".into(),
        ..import.preview().provider.clone()
    };
    for runtime in [
        RuntimeEvidence {
            review_digest: "stale-content",
            ..evidence(&import, Some(&profile))
        },
        RuntimeEvidence {
            provider: &other_provider,
            ..evidence(&import, Some(&profile))
        },
    ] {
        assert_eq!(
            root.check_plugin_enablement(&import, runtime, Some(approval(&import, &profile))),
            EnableDecision::Blocked(EnableBlock::RuntimeEvidenceMismatch)
        );
    }
    let mut overbroad = profile.clone();
    overbroad
        .effective_permissions
        .insert(PluginPermission::Network);
    assert_eq!(
        root.check_plugin_enablement(
            &import,
            evidence(&import, Some(&overbroad)),
            Some(approval(&import, &overbroad))
        ),
        EnableDecision::Blocked(EnableBlock::EnforcementMismatch)
    );
    for changed in [
        EnforcedProfile {
            isolation: Isolation::Process,
            ..profile.clone()
        },
        EnforcedProfile {
            constraint_ref: "fixture:independent-context".into(),
            ..profile.clone()
        },
    ] {
        assert_eq!(
            root.check_plugin_enablement(
                &import,
                evidence(&import, Some(&changed)),
                Some(approval(&import, &profile))
            ),
            EnableDecision::Blocked(EnableBlock::ApprovalMismatch)
        );
    }
}

#[test]
fn legacy_plugin_enabled_does_not_enable_and_ordinary_source_inventory_still_works() {
    let fixture = Fixture::new();
    let root = fixture.source();
    for kind in [SourceKind::Plugin, SourceKind::Managed, SourceKind::Native] {
        let inventory = json!({"schema_version":1,"provider":fixture.descriptor.provider,"capabilities":[
          {"id":"fixture-skill","name":"Fixture","enabled":true,
           "definition":{"kind":"skill","metadata":{"content_ref":"skill:opaque"}}}
        ]});
        fixture.write("inventory.json", &serde_json::to_vec(&inventory).unwrap());
        let descriptor = SourceDescriptor {
            kind,
            ..fixture.descriptor.clone()
        };
        let policy = ResolutionPolicy {
            provider: descriptor.provider.clone(),
            rule_revision: "fixture-rule".into(),
            evidence_ref: "fixture:evidence".into(),
            lowest_to_highest: vec![descriptor.source_id.clone()],
            merge_rule: MergeRule::RejectDuplicates,
        };
        let source = root
            .read_selected(descriptor.clone(), Path::new("inventory.json"))
            .unwrap();
        let snap = CapabilitySnapshot::resolve(policy.clone(), vec![source]).unwrap();
        let preview = snap.preview();
        let legacy_fingerprint = json!({
            "schema_version": 1,
            "policy": policy,
            "sources": preview.sources,
            "descriptors": [descriptor],
        });
        let legacy_digest = sha256(&serde_json::to_vec(&legacy_fingerprint).unwrap());
        assert_eq!(snap.digest() == legacy_digest, kind != SourceKind::Plugin);
        assert_eq!(preview.candidates.len(), 1);
        assert!(preview.candidates[0].winner);
        let expected = kind != SourceKind::Plugin;
        assert_eq!(preview.candidates[0].enabled, expected);
        let binding = SessionBinding {
            session_id: "fixture-session".into(),
            provider: descriptor.provider,
            config_snapshot_digest: snap.digest().into(),
        };
        let assessment = snap
            .assess(&descriptor.source_id, "fixture-skill", &binding, &[], 1)
            .unwrap();
        assert_eq!(assessment.enabled, expected);
    }
}
