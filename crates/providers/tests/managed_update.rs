#![cfg(unix)]

use agentbaton_providers::connections::*;
use agentbaton_providers::installation::{
    executable_digest, probe_version, BinaryDigest, InstallationError, VersionProbeRequest,
};
use agentbaton_providers::managed_update::*;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;

const REQUIRED: &[&str] = &["session-create"];

struct Fixture {
    _private: TempDir,
    root: PathBuf,
    current: InstallationRecord,
    staged: InstallationRecord,
    current_support: SupportManifest,
    staged_support: SupportManifest,
    connection: NamedConnection,
    environment: RuntimeEnvironment,
    identity: IdentityReference,
    now: u64,
}

impl Fixture {
    fn new() -> Self {
        let private = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(private.path())
            .unwrap()
            .join("installations");
        fs::create_dir(&root).unwrap();
        let (current, current_support) = synthetic_release(&root, "current-release", "fixture 1.0");
        let (staged, staged_support) = synthetic_release(&root, "candidate-release", "fixture 2.0");
        let now = staged.version_probe.observed_at_unix_ms + 100;
        Self {
            _private: private,
            connection: NamedConnection {
                id: "work-connection".into(),
                label: "Synthetic".into(),
                revision: 12,
                provider: ProviderKind::CustomAcp,
                protocol_route: ProtocolRoute::CustomAcp,
                installation_id: current.id.clone(),
                environment_id: "work-env".into(),
                identity_ref: "work-identity".into(),
                host_id: "fixture-host".into(),
                support_manifest_ref: current_support.id.clone(),
                identity_fallback: IdentityFallback::Deny,
            },
            environment: RuntimeEnvironment {
                id: "work-env".into(),
                host_id: "fixture-host".into(),
                configuration: ConfigurationSource::BatonManaged {
                    config_root: root.join("config"),
                    state_root: root.join("state"),
                },
                config_snapshot_sha256: current.binary_sha256.clone(),
            },
            identity: IdentityReference {
                id: "work-identity".into(),
                provider: ProviderKind::CustomAcp,
                host_id: "fixture-host".into(),
                native_handle_ref: "opaque-fixture-reference".into(),
            },
            root,
            current,
            staged,
            current_support,
            staged_support,
            now,
        }
    }

    fn scope(&self) -> UpdateScope<'_> {
        UpdateScope {
            connection_id: &self.connection.id,
            current_connection_revision: self.connection.revision,
            provider: self.connection.provider,
            route: self.connection.protocol_route,
            execution_mode: ExecutionMode::Batch,
            host_id: &self.connection.host_id,
            identity_ref: &self.connection.identity_ref,
            environment_id: &self.environment.id,
            configuration: &self.environment.configuration,
            config_snapshot_sha256: &self.environment.config_snapshot_sha256,
        }
    }

    fn approved(&self) -> ApprovedManagedUpdate<'_> {
        ApprovedManagedUpdate {
            approval_ref: "approved-exact-transition",
            scope: self.scope(),
            managed_root: &self.root,
            current: pin(&self.current, &self.current_support),
            candidate: pin(&self.staged, &self.staged_support),
            required_capabilities: REQUIRED,
            valid_until_unix_ms: self.now + 100,
            max_evidence_age_ms: 1000,
        }
    }

    fn compatibility(&self) -> ManagedUpdateCompatibility<'_> {
        ManagedUpdateCompatibility {
            scope: self.scope(),
            tested_installation: pin(&self.staged, &self.staged_support),
            support: &self.staged_support,
        }
    }

    fn request(&self) -> ManagedUpdateRequest<'_> {
        ManagedUpdateRequest {
            task: UpdateTask::New {
                task_id: "new-task",
                attempt_id: "first-attempt",
                input_revision: "input-revision-9",
                mode: ExecutionMode::Batch,
            },
            connection: &self.connection,
            current: &self.current,
            current_support: &self.current_support,
            staged: &self.staged,
            environment: &self.environment,
            identity: &self.identity,
            verification_host_id: &self.connection.host_id,
            now_unix_ms: self.now,
        }
    }

    fn admit(&self) -> Result<(NamedConnection, ExecutionBinding), ManagedUpdateError> {
        admit_managed_update(
            &self.request(),
            Some(&self.approved()),
            &self.compatibility(),
        )
    }
}

fn synthetic_release(
    root: &Path,
    id: &str,
    version: &str,
) -> (InstallationRecord, SupportManifest) {
    let directory = root.join(id);
    fs::create_dir(&directory).unwrap();
    let executable = directory.join("fixture-provider");
    fs::write(
        &executable,
        format!(
            "#!/bin/sh\n[ \"$#\" = 1 ] && [ \"$1\" = --version ] || exit 2\nprintf '%s\\n' '{version}'\n"
        ),
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let digest = executable_digest(&executable).unwrap();
    let probe = probe_version(VersionProbeRequest {
        executable: executable.clone(),
        expected_sha256: digest.clone(),
        working_directory: directory,
        approval_reference: format!("fixture-selected-{id}"),
        environment: BTreeMap::new(),
        timeout: Duration::from_secs(2),
    })
    .unwrap();
    assert_eq!(probe.version_line.as_deref(), Some(version));
    assert!(probe.process_reaped);
    let observed_at = probe.observed_at_unix_ms + 1;
    (
        InstallationRecord {
            id: id.into(),
            provider: ProviderKind::CustomAcp,
            host_id: "fixture-host".into(),
            source: InstallationSource::BatonManaged,
            approval_reference: format!("fixture-selected-{id}"),
            canonical_executable: executable,
            binary_sha256: digest,
            version_probe: probe,
        },
        SupportManifest {
            id: format!("support-{id}"),
            installation_id: id.into(),
            provider: ProviderKind::CustomAcp,
            provider_version: version.into(),
            // A synthetic trusted-verifier record for admission contract tests.
            capabilities: vec![CapabilityEvidence {
                capability: "session-create".into(),
                state: SupportState::Supported,
                source: EvidenceSource::ProtocolNegotiation,
                evidence_ref: format!("synthetic-observation-{id}"),
                provider_version: version.into(),
                observed_at_unix_ms: observed_at,
            }],
        },
    )
}

fn pin<'a>(
    installation: &'a InstallationRecord,
    support: &'a SupportManifest,
) -> UpdateInstallationPin<'a> {
    UpdateInstallationPin {
        installation_id: &installation.id,
        installation_approval_ref: &installation.approval_reference,
        executable: &installation.canonical_executable,
        binary_sha256: &installation.binary_sha256,
        version: &support.provider_version,
        support_manifest_id: &support.id,
    }
}

#[test]
fn new_task_pins_the_distinct_program_and_preserves_old_binding_and_files() {
    let f = Fixture::new();
    assert_eq!(
        f.current.binary_sha256.as_str(),
        "db8950dd30f13b902c56e6a1a7f52ee83e86654bd9ba81a984f7626a76e4219e"
    );
    assert_eq!(
        f.staged.binary_sha256.as_str(),
        "df9e8b2e3d76c08ace33644a7abed0b92c59a8a9612bf9aa9dbfab923d83533f"
    );
    let old = f
        .connection
        .bind_execution(
            "old-task",
            "active-attempt",
            "input-7",
            ExecutionMode::Batch,
            &f.current,
            &f.environment,
            &f.identity,
            &f.current_support,
            REQUIRED,
        )
        .unwrap();
    let old_snapshot = serde_json::to_value(&old).unwrap();
    let connection_before = serde_json::to_value(&f.connection).unwrap();
    let current_before = fs::read(&f.current.canonical_executable).unwrap();
    let staged_before = fs::read(&f.staged.canonical_executable).unwrap();

    let (next, binding) = f.admit().unwrap();
    assert_eq!(binding.task_id, "new-task");
    assert_eq!(binding.attempt_id, "first-attempt");
    assert_eq!(binding.input_revision, "input-revision-9");
    assert_eq!(binding.execution_mode, ExecutionMode::Batch);
    assert_eq!(next.id, f.connection.id);
    assert_eq!(next.revision, 13);
    assert_eq!(next.installation_id, f.staged.id);
    assert_eq!(binding.connection_revision, next.revision);
    assert_eq!(binding.connection_id, old.connection_id);
    assert_eq!(binding.installation_id, f.staged.id);
    assert_eq!(binding.provider_version, "fixture 2.0");
    assert_eq!(
        binding.executable_sha256,
        executable_digest(&f.staged.canonical_executable).unwrap()
    );
    assert_eq!(binding.support_manifest_ref, f.staged_support.id);
    assert_eq!(binding.provider, old.provider);
    assert_eq!(binding.protocol_route, old.protocol_route);
    assert_eq!(binding.environment_id, old.environment_id);
    assert_eq!(binding.host_id, old.host_id);
    assert_eq!(binding.identity_ref, old.identity_ref);
    assert_eq!(binding.configuration, old.configuration);
    assert_eq!(binding.config_snapshot_sha256, old.config_snapshot_sha256);

    assert_eq!(serde_json::to_value(&old).unwrap(), old_snapshot);
    assert_eq!(
        serde_json::to_value(&f.connection).unwrap(),
        connection_before
    );
    assert_eq!(old.installation_id, f.current.id);
    assert_eq!(old.provider_version, "fixture 1.0");
    assert_eq!(
        old.executable_sha256,
        executable_digest(&f.current.canonical_executable).unwrap()
    );
    assert_eq!(
        fs::read(&f.current.canonical_executable).unwrap(),
        current_before
    );
    assert_eq!(
        fs::read(&f.staged.canonical_executable).unwrap(),
        staged_before
    );
}

#[test]
fn existing_or_unknown_task_stops_before_inspection_even_with_approval() {
    let f = Fixture::new();
    fs::remove_file(&f.staged.canonical_executable).unwrap();
    let mut request = f.request();
    for (task, expected) in [
        (UpdateTask::AlreadyExists, ManagedUpdateError::ExistingTask),
        (
            UpdateTask::OutcomeUnknown,
            ManagedUpdateError::OutcomeUnknown,
        ),
    ] {
        request.task = task;
        assert_eq!(
            admit_managed_update(&request, Some(&f.approved()), &f.compatibility()).unwrap_err(),
            expected
        );
    }
}

#[test]
fn absent_or_invalid_policy_never_infers_approval_from_managed_records() {
    let f = Fixture::new();
    let request = f.request();
    let compatibility = f.compatibility();
    assert_eq!(
        admit_managed_update(&request, None, &compatibility).unwrap_err(),
        ManagedUpdateError::PolicyUnavailable
    );
    for approved in [
        ApprovedManagedUpdate {
            approval_ref: "",
            ..f.approved()
        },
        ApprovedManagedUpdate {
            valid_until_unix_ms: f.now,
            ..f.approved()
        },
        ApprovedManagedUpdate {
            required_capabilities: &[],
            ..f.approved()
        },
        ApprovedManagedUpdate {
            max_evidence_age_ms: 0,
            ..f.approved()
        },
    ] {
        assert_eq!(
            admit_managed_update(&request, Some(&approved), &compatibility).unwrap_err(),
            ManagedUpdateError::InvalidPolicy
        );
    }
}

#[test]
fn policy_is_scoped_to_exact_connection_revision_provider_host_and_config() {
    let f = Fixture::new();
    let scope = f.scope();
    let alternative = BinaryDigest::parse(&"c".repeat(64)).unwrap();
    for approved_scope in [
        UpdateScope {
            connection_id: "other-connection",
            ..scope
        },
        UpdateScope {
            current_connection_revision: 11,
            ..scope
        },
        UpdateScope {
            provider: ProviderKind::Codex,
            ..scope
        },
        UpdateScope {
            route: ProtocolRoute::ClaudeAgentAcp,
            ..scope
        },
        UpdateScope {
            execution_mode: ExecutionMode::Interactive,
            ..scope
        },
        UpdateScope {
            host_id: "other-host",
            ..scope
        },
        UpdateScope {
            identity_ref: "other-identity",
            ..scope
        },
        UpdateScope {
            environment_id: "other-environment",
            ..scope
        },
        UpdateScope {
            config_snapshot_sha256: &alternative,
            ..scope
        },
    ] {
        let approved = ApprovedManagedUpdate {
            scope: approved_scope,
            ..f.approved()
        };
        assert_eq!(
            admit_managed_update(&f.request(), Some(&approved), &f.compatibility()).unwrap_err(),
            ManagedUpdateError::PolicyMismatch
        );
    }
    let request = ManagedUpdateRequest {
        verification_host_id: "other-host",
        ..f.request()
    };
    assert_eq!(
        admit_managed_update(&request, Some(&f.approved()), &f.compatibility()).unwrap_err(),
        ManagedUpdateError::PolicyMismatch
    );
}

#[test]
fn either_release_must_match_the_policy_and_verifier_pin_is_independent() {
    let f = Fixture::new();
    let approved = ApprovedManagedUpdate {
        current: UpdateInstallationPin {
            version: "old-version-label",
            ..f.approved().current
        },
        ..f.approved()
    };
    let compatibility = f.compatibility();
    assert_eq!(
        admit_managed_update(&f.request(), Some(&approved), &compatibility).unwrap_err(),
        ManagedUpdateError::PolicyMismatch
    );
    let approved = ApprovedManagedUpdate {
        candidate: UpdateInstallationPin {
            binary_sha256: &f.current.binary_sha256,
            ..f.approved().candidate
        },
        ..f.approved()
    };
    assert_eq!(
        admit_managed_update(&f.request(), Some(&approved), &compatibility).unwrap_err(),
        ManagedUpdateError::PolicyMismatch
    );
    let scope = f.scope();
    for evidence in [
        ManagedUpdateCompatibility {
            tested_installation: UpdateInstallationPin {
                binary_sha256: &f.current.binary_sha256,
                ..compatibility.tested_installation
            },
            ..f.compatibility()
        },
        ManagedUpdateCompatibility {
            tested_installation: pin(&f.current, &f.current_support),
            ..f.compatibility()
        },
        ManagedUpdateCompatibility {
            scope: UpdateScope {
                host_id: "other-host",
                ..scope
            },
            ..f.compatibility()
        },
        ManagedUpdateCompatibility {
            scope: UpdateScope {
                config_snapshot_sha256: &f.staged.binary_sha256,
                ..scope
            },
            ..f.compatibility()
        },
    ] {
        assert_eq!(
            admit_managed_update(&f.request(), Some(&f.approved()), &evidence).unwrap_err(),
            ManagedUpdateError::CompatibilityMismatch
        );
    }
}

#[test]
fn existing_binding_rejects_different_provider_host_and_bad_probe() {
    let mut f = Fixture::new();
    f.staged.host_id = "different-host".into();
    assert_eq!(
        f.admit().unwrap_err(),
        ManagedUpdateError::Binding(BindingError::ReferenceMismatch)
    );
    f.staged.host_id.clone_from(&f.connection.host_id);
    f.staged.provider = ProviderKind::Codex;
    assert_eq!(
        f.admit().unwrap_err(),
        ManagedUpdateError::Binding(BindingError::ReferenceMismatch)
    );
    f.staged.provider = f.connection.provider;
    f.staged.version_probe.executable_sha256 = f.current.binary_sha256.clone();
    assert_eq!(
        f.admit().unwrap_err(),
        ManagedUpdateError::Binding(BindingError::InstallationUnverified)
    );
}

#[test]
fn evidence_must_be_complete_current_and_runtime_capable() {
    let mut f = Fixture::new();
    let manifest = f.staged_support.clone();
    let unavailable = ManagedUpdateError::Binding(BindingError::CapabilityUnavailable);
    f.staged_support.capabilities.clear();
    assert_eq!(f.admit().unwrap_err(), unavailable);
    f.staged_support = manifest.clone();
    f.staged_support.capabilities[0].source = EvidenceSource::FixtureTest;
    assert_eq!(f.admit().unwrap_err(), unavailable);
    f.staged_support.capabilities[0].source = EvidenceSource::VersionProbe;
    assert_eq!(
        f.admit().unwrap_err(),
        ManagedUpdateError::Binding(BindingError::InvalidEvidence)
    );
    f.staged_support = manifest.clone();
    f.staged_support.capabilities[0]
        .provider_version
        .clone_from(&f.current_support.provider_version);
    assert_eq!(
        f.admit().unwrap_err(),
        ManagedUpdateError::Binding(BindingError::InvalidEvidence)
    );
    f.staged_support = manifest.clone();
    f.staged_support.capabilities[0].observed_at_unix_ms =
        f.staged.version_probe.observed_at_unix_ms - 1;
    assert_eq!(f.admit().unwrap_err(), ManagedUpdateError::StaleEvidence);
    f.staged_support.capabilities[0].observed_at_unix_ms = f.now + 1;
    assert_eq!(f.admit().unwrap_err(), ManagedUpdateError::StaleEvidence);
    f.staged_support = manifest;
    let approved = ApprovedManagedUpdate {
        max_evidence_age_ms: 99,
        ..f.approved()
    };
    assert_eq!(
        admit_managed_update(&f.request(), Some(&approved), &f.compatibility()).unwrap_err(),
        ManagedUpdateError::StaleEvidence
    );
}

#[test]
fn external_installations_and_outside_root_declarations_are_denied() {
    let mut f = Fixture::new();
    let global_root = ApprovedManagedUpdate {
        managed_root: Path::new("/"),
        ..f.approved()
    };
    assert_eq!(
        admit_managed_update(&f.request(), Some(&global_root), &f.compatibility()).unwrap_err(),
        ManagedUpdateError::ExternalInstallation
    );
    f.current.source = InstallationSource::ExternalExecutable;
    assert_eq!(
        f.admit().unwrap_err(),
        ManagedUpdateError::ExternalInstallation
    );
    f.current.source = InstallationSource::BatonManaged;
    f.staged.source = InstallationSource::ExternalExecutable;
    assert_eq!(
        f.admit().unwrap_err(),
        ManagedUpdateError::ExternalInstallation
    );
    f.staged.source = InstallationSource::BatonManaged;
    let outside = f.root.parent().unwrap().join("outside");
    fs::copy(&f.staged.canonical_executable, &outside).unwrap();
    f.staged.canonical_executable = outside;
    assert_eq!(
        f.admit().unwrap_err(),
        ManagedUpdateError::ExternalInstallation
    );
}

#[test]
fn reused_ids_and_aliased_executables_are_denied() {
    let mut f = Fixture::new();
    let new_id = f.staged.id.clone();
    f.staged.id.clone_from(&f.current.id);
    f.staged_support.installation_id.clone_from(&f.current.id);
    assert_eq!(
        f.admit().unwrap_err(),
        ManagedUpdateError::InstallationsNotIndependent
    );
    f.staged.id = new_id;
    f.staged_support.installation_id.clone_from(&f.staged.id);
    let staged = &f.staged.canonical_executable;
    fs::remove_file(staged).unwrap();
    fs::hard_link(&f.current.canonical_executable, staged).unwrap();
    assert_eq!(
        f.admit().unwrap_err(),
        ManagedUpdateError::InstallationsNotIndependent
    );
}

#[test]
fn managed_staging_cannot_alias_an_external_file_with_the_approved_bytes() {
    let f = Fixture::new();
    let outside = f.root.parent().unwrap().join("external-alias");
    fs::hard_link(&f.staged.canonical_executable, &outside).unwrap();
    assert_eq!(
        f.admit().unwrap_err(),
        ManagedUpdateError::InstallationsNotIndependent
    );
    assert_eq!(executable_digest(&outside).unwrap(), f.staged.binary_sha256);
}

#[test]
fn changed_bytes_and_symlinks_fail_live_digest_verification() {
    let f = Fixture::new();
    let staged = &f.staged.canonical_executable;
    let outside = f.root.parent().unwrap().join("outside");
    fs::copy(staged, &outside).unwrap();
    fs::write(staged, b"changed candidate").unwrap();
    assert_eq!(
        f.admit().unwrap_err(),
        ManagedUpdateError::Verification(InstallationError::DigestMismatch)
    );
    fs::remove_file(staged).unwrap();
    symlink(&outside, staged).unwrap();
    assert_eq!(
        f.admit().unwrap_err(),
        ManagedUpdateError::Verification(InstallationError::InvalidPath)
    );
    fs::remove_file(staged).unwrap();
    fs::copy(outside, staged).unwrap();
    fs::write(&f.current.canonical_executable, b"changed current").unwrap();
    assert_eq!(
        f.admit().unwrap_err(),
        ManagedUpdateError::Verification(InstallationError::DigestMismatch)
    );
}

#[test]
fn managed_program_update_preserves_explicit_read_only_native_config_references() {
    let mut f = Fixture::new();
    let private_native = f.root.parent().unwrap().join("native-config");
    fs::write(&private_native, b"private synthetic configuration").unwrap();
    f.environment.configuration = ConfigurationSource::NativeConnected {
        config_source_refs: vec![private_native.to_str().unwrap().into()],
    };
    let (_, binding) = f.admit().unwrap();
    assert_eq!(binding.configuration, f.environment.configuration);
    assert_eq!(
        fs::read(private_native).unwrap(),
        b"private synthetic configuration"
    );
}
