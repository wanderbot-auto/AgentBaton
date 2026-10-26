use agentbaton_providers::connections::*;
use agentbaton_providers::installation::{
    BinaryDigest, VersionEvidence, VersionEvidenceSource, VersionProbeState,
};
use serde_json::json;
fn digest(c: &str) -> BinaryDigest {
    BinaryDigest::parse(&c.repeat(64)).unwrap()
}
fn fixtures() -> (
    NamedConnection,
    InstallationRecord,
    RuntimeEnvironment,
    IdentityReference,
    SupportManifest,
) {
    let connection = NamedConnection {
        id: "connection-work".into(),
        label: "Work account".into(),
        revision: 1,
        provider: ProviderKind::Codex,
        protocol_route: ProtocolRoute::CodexAcpAdapter,
        installation_id: "codex-adapter".into(),
        environment_id: "managed-work".into(),
        identity_ref: "identity-work".into(),
        host_id: "local-mac".into(),
        support_manifest_ref: "support-1".into(),
        identity_fallback: IdentityFallback::Deny,
    };
    let installation = InstallationRecord {
        id: "codex-adapter".into(),
        provider: ProviderKind::Codex,
        host_id: "local-mac".into(),
        source: InstallationSource::ExternalExecutable,
        approval_reference: "fixture-selected-binary-grant".into(),
        canonical_executable: "/approved/bin/codex-acp".into(),
        binary_sha256: digest("1"),
        version_probe: VersionEvidence {
            source: VersionEvidenceSource::SelectedExecutableVersionProbe,
            executable_sha256: digest("1"),
            observed_at_unix_ms: 1,
            state: VersionProbeState::Succeeded,
            version_line: Some("fixture adapter 1.0".into()),
            exit_code: Some(0),
            stdout_bytes: 20,
            stderr_bytes: 0,
            process_reaped: true,
        },
    };
    let environment = RuntimeEnvironment {
        id: "managed-work".into(),
        host_id: "local-mac".into(),
        configuration: ConfigurationSource::BatonManaged {
            config_root: "/private/baton/work/config".into(),
            state_root: "/private/baton/work/state".into(),
        },
        config_snapshot_sha256: digest("2"),
    };
    let identity = IdentityReference {
        id: "identity-work".into(),
        provider: ProviderKind::Codex,
        host_id: "local-mac".into(),
        native_handle_ref: "native-secret-store:work".into(),
    };
    let support = SupportManifest {
        id: "support-1".into(),
        installation_id: "codex-adapter".into(),
        provider: ProviderKind::Codex,
        provider_version: "fixture adapter 1.0".into(),
        capabilities: vec![CapabilityEvidence {
            capability: "session-create".into(),
            state: SupportState::Supported,
            source: EvidenceSource::ProtocolNegotiation,
            evidence_ref: "fixture-handshake-record-1".into(),
            provider_version: "fixture adapter 1.0".into(),
            observed_at_unix_ms: 1,
        }],
    };
    (connection, installation, environment, identity, support)
}
#[test]
fn program_and_configuration_ownership_are_independent_and_binding_is_immutable() {
    let (c, installation, mut environment, identity, support) = fixtures();
    let binding = c
        .bind_execution(
            "task-1",
            "attempt-1",
            "input-sha-1",
            ExecutionMode::Interactive,
            &installation,
            &environment,
            &identity,
            &support,
            &["session-create"],
        )
        .unwrap();
    assert_eq!(binding.identity_ref, "identity-work");
    assert_eq!(binding.provider_version, "fixture adapter 1.0");
    assert_eq!(installation.source, InstallationSource::ExternalExecutable);
    assert!(matches!(
        binding.configuration,
        ConfigurationSource::BatonManaged { .. }
    ));
    environment.configuration = ConfigurationSource::NativeConnected {
        config_source_refs: vec!["approved-native-config-ref".into()],
    };
    environment.config_snapshot_sha256 = digest("3");
    assert_eq!(binding.config_snapshot_sha256, digest("2"));
    let native = c
        .bind_execution(
            "task-2",
            "attempt-2",
            "input-sha-2",
            ExecutionMode::Batch,
            &installation,
            &environment,
            &identity,
            &support,
            &[],
        )
        .unwrap();
    assert!(matches!(
        native.configuration,
        ConfigurationSource::NativeConnected { .. }
    ));
    assert_eq!(native.execution_mode, ExecutionMode::Batch);
}
#[test]
fn named_connections_cannot_substitute_accounts_or_hosts() {
    let (mut personal, installation, environment, mut identity, support) = fixtures();
    personal.id = "connection-personal".into();
    personal.identity_ref = "identity-personal".into();
    assert_eq!(
        personal
            .bind_execution(
                "t",
                "a",
                "rev",
                ExecutionMode::Interactive,
                &installation,
                &environment,
                &identity,
                &support,
                &[]
            )
            .unwrap_err(),
        BindingError::ReferenceMismatch
    );
    identity.id = "identity-personal".into();
    let personal_binding = personal
        .bind_execution(
            "t",
            "a",
            "rev",
            ExecutionMode::Interactive,
            &installation,
            &environment,
            &identity,
            &support,
            &[],
        )
        .unwrap();
    assert_eq!(personal_binding.connection_id, "connection-personal");
    assert_eq!(personal_binding.identity_ref, "identity-personal");
    identity.host_id = "remote-linux".into();
    assert_eq!(
        personal
            .bind_execution(
                "t",
                "a",
                "rev",
                ExecutionMode::Interactive,
                &installation,
                &environment,
                &identity,
                &support,
                &[]
            )
            .unwrap_err(),
        BindingError::ReferenceMismatch
    );
}
#[test]
fn native_appserver_and_vendor_acp_routes_remain_distinct() {
    assert_eq!(
        ProtocolRoute::CodexAppServer.documented_acp_arguments(),
        None
    );
    assert_eq!(ProtocolRoute::CustomAcp.documented_acp_arguments(), None);
    assert_eq!(
        ProtocolRoute::QoderNativeAcp.documented_acp_arguments(),
        Some(&["--acp"][..])
    );
    assert_eq!(
        ProtocolRoute::ClaudeAgentAcp.documented_acp_arguments(),
        Some(&[][..])
    );
    let (mut c, i, e, a, s) = fixtures();
    c.protocol_route = ProtocolRoute::QoderNativeAcp;
    assert_eq!(
        c.bind_execution(
            "t",
            "a",
            "rev",
            ExecutionMode::Interactive,
            &i,
            &e,
            &a,
            &s,
            &[]
        )
        .unwrap_err(),
        BindingError::ProviderRouteMismatch
    );
}
#[test]
fn version_drift_or_failed_probe_cannot_create_executable_binding() {
    let (c, mut i, e, a, mut s) = fixtures();
    i.binary_sha256 = digest("4");
    assert_eq!(
        c.bind_execution(
            "t",
            "a",
            "rev",
            ExecutionMode::Interactive,
            &i,
            &e,
            &a,
            &s,
            &[]
        )
        .unwrap_err(),
        BindingError::InstallationUnverified
    );
    i.binary_sha256 = digest("1");
    i.version_probe.state = VersionProbeState::TimedOut;
    assert_eq!(
        c.bind_execution(
            "t",
            "a",
            "rev",
            ExecutionMode::Interactive,
            &i,
            &e,
            &a,
            &s,
            &[]
        )
        .unwrap_err(),
        BindingError::InstallationUnverified
    );
    i.version_probe.state = VersionProbeState::Succeeded;
    s.provider_version = "new-version".into();
    assert_eq!(
        c.bind_execution(
            "t",
            "a",
            "rev",
            ExecutionMode::Interactive,
            &i,
            &e,
            &a,
            &s,
            &[]
        )
        .unwrap_err(),
        BindingError::InstallationUnverified
    );
}
#[test]
fn unknown_unsupported_untested_and_fixture_support_cannot_become_live_support() {
    let (_c, _i, _e, _a, mut s) = fixtures();
    for status in [
        SupportState::Unknown,
        SupportState::Unsupported,
        SupportState::Untested,
    ] {
        s.capabilities[0].state = status;
        assert_eq!(
            s.require_runtime_capability("session-create"),
            Err(BindingError::CapabilityUnavailable)
        );
    }
    s.capabilities[0].state = SupportState::Supported;
    s.capabilities[0].source = EvidenceSource::FixtureTest;
    assert_eq!(
        s.require_runtime_capability("session-create"),
        Err(BindingError::CapabilityUnavailable)
    );
    for source in [EvidenceSource::Documentation, EvidenceSource::VersionProbe] {
        s.capabilities[0].source = source;
        assert_eq!(s.validate(), Err(BindingError::InvalidEvidence));
    }
}
#[test]
fn evidence_must_have_unique_features_bound_to_exact_version_and_timestamp() {
    let (_c, _i, _e, _a, mut s) = fixtures();
    s.capabilities[0].observed_at_unix_ms = 0;
    assert_eq!(s.validate(), Err(BindingError::InvalidEvidence));
    s.capabilities[0].observed_at_unix_ms = 1;
    s.capabilities[0].provider_version = "wrong-version".into();
    assert_eq!(s.validate(), Err(BindingError::InvalidEvidence));
    s.capabilities[0].provider_version = s.provider_version.clone();
    s.capabilities.push(s.capabilities[0].clone());
    assert_eq!(s.validate(), Err(BindingError::InvalidEvidence));
}
#[test]
fn fallback_and_external_writeback_have_no_silent_deserialization_path() {
    let (c, _i, e, _a, _s) = fixtures();
    let mut value = serde_json::to_value(c).unwrap();
    value["identity_fallback"] = json!("automatic");
    assert!(serde_json::from_value::<NamedConnection>(value).is_err());
    let mut value = serde_json::to_value(e).unwrap();
    value["writeback_external"] = json!(true);
    assert!(serde_json::from_value::<RuntimeEnvironment>(value).is_err());
    let value = json!({"mode":"native_connected","config_source_refs":["config-1"],"copy_private_auth":true});
    assert!(serde_json::from_value::<ConfigurationSource>(value).is_err());
}

#[test]
fn contradictory_success_or_missing_provenance_cannot_bind_execution() {
    let (c, mut i, e, a, s) = fixtures();
    i.version_probe.exit_code = Some(7);
    assert_eq!(
        c.bind_execution(
            "t",
            "a",
            "rev",
            ExecutionMode::Interactive,
            &i,
            &e,
            &a,
            &s,
            &[]
        )
        .unwrap_err(),
        BindingError::InstallationUnverified
    );
    i.version_probe.exit_code = Some(0);
    i.approval_reference.clear();
    assert_eq!(
        c.bind_execution(
            "t",
            "a",
            "rev",
            ExecutionMode::Interactive,
            &i,
            &e,
            &a,
            &s,
            &[]
        )
        .unwrap_err(),
        BindingError::InstallationUnverified
    );
}
