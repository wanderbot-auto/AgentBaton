#![cfg(unix)]
use agentbaton_providers::installation::{
    executable_digest, probe_version, verify_executable, BinaryDigest, InstallationError,
    VersionProbeRequest, VersionProbeState,
};
use agentbaton_providers::transport::{
    ApprovedInstallation, FailureKind, ProviderProcess, SideEffectState,
};
use std::collections::BTreeMap;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::TempDir;
fn fixture(mode: &str) -> (TempDir, VersionProbeRequest) {
    let dir = tempfile::tempdir().unwrap();
    let python = Command::new("python3")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    assert!(python.status.success());
    let python = std::fs::canonicalize(String::from_utf8(python.stdout).unwrap().trim()).unwrap();
    let executable = dir.path().join("selected-provider");
    let script = format!(
        r##"#!{}
import os,sys,time
mode={:?}
with open('probe.pid','w') as f: f.write(str(os.getpid()))
assert sys.argv[1:]==['--version']
if mode=='timeout': time.sleep(60)
if mode=='nonzero': print('private failure',file=sys.stderr); sys.exit(7)
if mode=='flood': os.write(1,b'x'*2000000); sys.exit(0)
if mode in ('stderr','stderr_small'):
    import fcntl
    if hasattr(fcntl,'F_SETPIPE_SZ'): fcntl.fcntl(2,fcntl.F_SETPIPE_SZ,4096)
    for _ in range(1024): os.write(2,b'x'*4096)
if mode=='invalid': os.write(1,b'\xff'); sys.exit(0)
if mode=='multiline': print('one\ntwo'); sys.exit(0)
if mode=='selfchange':
    with open(__file__,'a') as f: f.write('\n# binary changed during probe\n')
if mode=='env':
    print('fixture 1.2.3 inherited_credentials='+str(any(k in os.environ for k in ('GH_TOKEN','GITHUB_TOKEN')))); sys.exit(0)
print('fixture-provider 1.2.3')
"##,
        python.display(),
        mode
    );
    std::fs::write(&executable, script).unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let executable = std::fs::canonicalize(executable).unwrap();
    let request = VersionProbeRequest {
        expected_sha256: executable_digest(&executable).unwrap(),
        executable,
        working_directory: dir.path().to_path_buf(),
        approval_reference: "fixture-owner-selection".into(),
        environment: BTreeMap::new(),
        timeout: Duration::from_millis(500),
    };
    (dir, request)
}
fn assert_reaped(dir: &TempDir) {
    let pid = std::fs::read_to_string(dir.path().join("probe.pid"))
        .unwrap()
        .parse::<i32>()
        .unwrap();
    // SAFETY: signal zero only checks process existence.
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
}
#[test]
fn exact_digest_is_pinned_and_probe_records_actual_version_only() {
    let (dir, request) = fixture("normal");
    let expected = request.expected_sha256.clone();
    let result = probe_version(request).unwrap();
    assert_eq!(result.state, VersionProbeState::Succeeded);
    assert_eq!(
        result.version_line.as_deref(),
        Some("fixture-provider 1.2.3")
    );
    assert_eq!(result.executable_sha256, expected);
    assert_eq!(result.exit_code, Some(0));
    assert!(result.observed_at_unix_ms > 0 && result.process_reaped);
    assert_reaped(&dir);
}
#[test]
fn digests_and_paths_are_strict_and_symlinks_do_not_bypass_pin() {
    let (dir, request) = fixture("normal");
    let link = dir.path().join("alias");
    symlink(&request.executable, &link).unwrap();
    assert_eq!(
        executable_digest(&link),
        Err(InstallationError::InvalidPath)
    );
    assert_eq!(
        executable_digest(&PathBuf::from("selected-provider")),
        Err(InstallationError::InvalidPath)
    );
    assert!(BinaryDigest::parse(&"A".repeat(64)).is_err());
    assert!(BinaryDigest::parse(&"1".repeat(63)).is_err());
    assert!(serde_json::from_str::<BinaryDigest>("\"not-a-digest\"").is_err());
    assert_eq!(
        verify_executable(
            &request.executable,
            &BinaryDigest::parse(&"0".repeat(64)).unwrap()
        ),
        Err(InstallationError::DigestMismatch)
    );
}
#[test]
fn changed_selected_binary_is_rejected_before_probe_or_transport_spawn() {
    let (dir, request) = fixture("normal");
    std::fs::write(&request.executable, "#!/bin/sh\necho must-not-run\n").unwrap();
    assert_eq!(
        probe_version(request.clone()).unwrap_err(),
        InstallationError::DigestMismatch
    );
    let descriptor = ApprovedInstallation {
        executable: request.executable,
        expected_sha256: request.expected_sha256,
        arguments: vec![],
        workspace: request.working_directory,
        approval_reference: request.approval_reference,
        environment: BTreeMap::new(),
    };
    let error = ProviderProcess::start(descriptor, Duration::from_secs(1))
        .err()
        .unwrap();
    assert_eq!(error.kind, FailureKind::BinaryChanged);
    assert_eq!(error.side_effect_state, SideEffectState::None);
    assert!(!dir.path().join("probe.pid").exists());
}
#[test]
fn executable_modified_during_probe_invalidates_observed_version() {
    let (dir, request) = fixture("selfchange");
    assert_eq!(
        probe_version(request).unwrap_err(),
        InstallationError::DigestMismatch
    );
    assert_reaped(&dir);
}
#[test]
fn version_probe_failures_preserve_distinct_non_success_evidence() {
    for (mode, state) in [
        ("nonzero", VersionProbeState::ExitFailure),
        ("invalid", VersionProbeState::InvalidOutput),
        ("multiline", VersionProbeState::InvalidOutput),
        ("flood", VersionProbeState::OutputLimit),
    ] {
        let (dir, request) = fixture(mode);
        let result = probe_version(request).unwrap();
        assert_eq!(result.state, state, "{mode}");
        assert_eq!(result.version_line, None);
        if mode == "nonzero" {
            assert_eq!(result.exit_code, Some(7));
        }
        assert!(result.process_reaped);
        assert_reaped(&dir);
    }
}
#[test]
fn timeout_is_bounded_and_probe_is_reaped() {
    let (dir, request) = fixture("timeout");
    let started = Instant::now();
    let result = probe_version(request).unwrap();
    assert_eq!(result.state, VersionProbeState::TimedOut);
    assert!(result.process_reaped);
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_reaped(&dir);
}
#[test]
fn stderr_flood_is_drained_without_retaining_raw_output() {
    let (dir, request) = fixture("stderr");
    let result = probe_version(request).unwrap();
    assert_eq!(result.state, VersionProbeState::Succeeded);
    assert_eq!(result.stderr_bytes, 4 * 1024 * 1024);
    assert_reaped(&dir);
}
#[test]
fn probe_environment_is_explicit_and_debug_redacts_values() {
    let (dir, mut request) = fixture("env");
    request.environment.insert(
        "TEST_OPAQUE_VALUE".into(),
        "must-not-appear-in-debug".into(),
    );
    let debug = format!("{request:?}");
    assert!(!debug.contains("must-not-appear-in-debug"));
    let result = probe_version(request).unwrap();
    assert_eq!(
        result.version_line.as_deref(),
        Some("fixture 1.2.3 inherited_credentials=False")
    );
    assert_reaped(&dir);
}
#[test]
fn missing_approval_cannot_execute_selected_binary() {
    let (dir, mut request) = fixture("normal");
    request.approval_reference.clear();
    assert_eq!(
        probe_version(request).unwrap_err(),
        InstallationError::InvalidApproval
    );
    assert!(!dir.path().join("probe.pid").exists());
}

#[test]
fn small_pipe_version_probe_preserves_volume_and_original_deadline() {
    let (dir, request) = fixture("stderr_small");
    assert_eq!(request.timeout, Duration::from_millis(500));
    let result = probe_version(request).unwrap();
    assert_eq!(result.state, VersionProbeState::Succeeded);
    assert_eq!(result.stderr_bytes, 4 * 1024 * 1024);
    assert_eq!(
        result.version_line.as_deref(),
        Some("fixture-provider 1.2.3")
    );
    assert!(result.process_reaped);
    assert_reaped(&dir);
}
