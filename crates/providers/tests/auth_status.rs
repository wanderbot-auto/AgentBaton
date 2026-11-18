#![cfg(unix)]

use agentbaton_providers::connections::ProtocolRoute;
use agentbaton_providers::installation::{
    executable_digest, probe_auth_status, AuthIdentityVerification, AuthStatusEvidence,
    AuthStatusProbeRequest, AuthStatusProbeState, BinaryDigest, InstallationError,
    LocalAuthObservation, VersionEvidence, VersionEvidenceSource, VersionProbeState,
};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

#[test]
fn public_api_does_not_launch_without_a_verified_runtime_and_version() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().canonicalize().unwrap();
    let executable = directory.join("generated-executable");
    let marker = directory.join("launched");
    fs::write(&executable, "#!/bin/sh\n: > \"${0%/*}/launched\"\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let digest = executable_digest(&executable).unwrap();
    let version = VersionEvidence {
        source: VersionEvidenceSource::SelectedExecutableVersionProbe,
        executable_sha256: digest.clone(),
        observed_at_unix_ms: 1,
        state: VersionProbeState::Succeeded,
        version_line: Some("codex-cli 0.0.0".into()),
        exit_code: Some(0),
        stdout_bytes: 16,
        stderr_bytes: 0,
        process_reaped: true,
    };
    let request = AuthStatusProbeRequest {
        route: ProtocolRoute::CodexAppServer,
        reviewed_source_revision: "970111f6cb62cc02fe36a82ea7d71b37a2889c97",
        executable: &executable,
        expected_sha256: &digest,
        version: &version,
        approval_reference: "generated-auth-approval",
        timeout: Duration::from_secs(1),
        context: None,
    };
    let other_digest = BinaryDigest::parse(&"0".repeat(64)).unwrap();
    let other_version = VersionEvidence {
        version_line: Some("codex-cli 1.2.3".into()),
        ..version.clone()
    };
    let unreaped = VersionEvidence {
        process_reaped: false,
        ..version.clone()
    };
    use AuthStatusProbeState::*;
    for (request, expected) in [
        (request, UnverifiedContext),
        (
            AuthStatusProbeRequest {
                route: ProtocolRoute::CodexAcpAdapter,
                ..request
            },
            UnsupportedRoute,
        ),
        (
            AuthStatusProbeRequest {
                route: ProtocolRoute::CustomAcp,
                ..request
            },
            UnsupportedRoute,
        ),
        (
            AuthStatusProbeRequest {
                reviewed_source_revision: "another-revision",
                ..request
            },
            UnreviewedBuild,
        ),
        (
            AuthStatusProbeRequest {
                expected_sha256: &other_digest,
                ..request
            },
            UnreviewedBuild,
        ),
        (
            AuthStatusProbeRequest {
                version: &other_version,
                ..request
            },
            UnreviewedBuild,
        ),
        (
            AuthStatusProbeRequest {
                version: &unreaped,
                ..request
            },
            UnreviewedBuild,
        ),
    ] {
        let observed = probe_auth_status(request).unwrap();
        assert_eq!(observed.state, expected);
        assert_eq!(observed.local_observation, LocalAuthObservation::Unknown);
        assert_eq!(
            observed.identity_verification,
            AuthIdentityVerification::Unknown
        );
        assert_eq!(observed.executable_sha256, None);
        assert_eq!(observed.process, None);
        assert!(!marker.exists());
    }
    assert_eq!(
        probe_auth_status(AuthStatusProbeRequest {
            approval_reference: " ",
            ..request
        }),
        Err(InstallationError::InvalidApproval)
    );
    assert!(!marker.exists());

    let mut json = serde_json::to_value(probe_auth_status(request).unwrap()).unwrap();
    json["identity_verification"] = serde_json::json!("verified");
    assert!(serde_json::from_value::<AuthStatusEvidence>(json).is_err());
}
