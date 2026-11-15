use super::*;
use std::{fs, time::Instant};
struct Fixture {
    _dir: tempfile::TempDir,
    store: Store,
    supervisor: Supervisor,
    root: PathBuf,
    c: ProviderConnection,
}
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let runtime = root.join("runtime");
    fs::create_dir(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let python = std::process::Command::new("python3")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    assert!(python.status.success());
    let exe = root.join("version-peer");
    let body = r#"
import sys,os,time
assert sys.argv[1:]==['--version']
p=os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(p,'calls'),'a') as f:f.write('version\n')
mode=open(os.path.join(p,'mode')).read()
if mode=='timeout':time.sleep(10)
if mode=='delay':time.sleep(.3)
if mode=='output':print('x'*5000,flush=True)
else:print('Generated version 1.0',flush=True)
if mode=='fail':sys.exit(9)
"#;
    fs::write(
        &exe,
        format!(
            "#!{}\n{}",
            String::from_utf8(python.stdout).unwrap().trim(),
            body
        ),
    )
    .unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(root.join("mode"), "fail").unwrap();
    let mut store = Store::open(&runtime.join("db.sqlite")).unwrap();
    let p = store
        .propose_connection("Generated", "custom_acp", &exe, &[], "register")
        .unwrap();
    let c: ProviderConnection =
        serde_json::from_value(store.finalize_proposal(&p.id, 1).unwrap()).unwrap();
    Fixture {
        _dir: dir,
        store,
        supervisor: Supervisor::new(&runtime),
        root,
        c,
    }
}
fn drain(f: &mut Fixture) {
    let at = Instant::now();
    while f.supervisor.has_owned_io() {
        f.supervisor.tick(&mut f.store).unwrap();
        assert!(at.elapsed() < Duration::from_secs(7));
        std::thread::sleep(Duration::from_millis(5));
    }
    f.c = f.store.get_connection(&f.c.id).unwrap();
}
fn initial_failure(f: &mut Fixture) {
    f.supervisor.probe(&f.store, &f.c.id).unwrap();
    drain(f);
    assert_eq!(f.c.probe_state, "failed");
    assert!(f.c.version_evidence.as_ref().unwrap().process_reaped);
}
fn query(f: &Fixture) -> ConnectionInspectRequest {
    ConnectionInspectRequest {
        connection_id: f.c.id.clone(),
        expected_revision: f.c.revision,
    }
}
fn approve(f: &mut Fixture, key: &str) -> (ApprovalProposal, ConnectionProbeConfirmation) {
    let p = f.store.propose_connection_probe(&query(f), key).unwrap();
    let v: ConnectionProbeConfirmation =
        serde_json::from_value(f.store.finalize_proposal(&p.id, 1).unwrap()).unwrap();
    (p, v)
}
fn calls(f: &Fixture) -> usize {
    fs::read_to_string(f.root.join("calls"))
        .unwrap_or_default()
        .lines()
        .count()
}
#[test]
fn generated_failed_probe_new_native_approval_success_and_replay_never_execute_twice() {
    let mut f = fixture();
    initial_failure(&mut f);
    let before = f.c.clone();
    let seq = f.store.sequence().unwrap();
    let inspection = f.store.inspect_connection(&query(&f)).unwrap();
    assert!(inspection.retry_eligible);
    assert_eq!(inspection.retry_basis.as_deref(), Some("reaped_failure"));
    assert_eq!(seq, f.store.sequence().unwrap());
    let (p, approved) = approve(&mut f, "retry");
    assert_eq!(p.snapshot["previous_stop_proof"]["kind"], "process_reaped");
    assert_eq!(approved.connection.approval_id, before.approval_id);
    assert_eq!(approved.connection.config_revision, before.config_revision);
    assert_eq!(approved.connection.environment_id, before.environment_id);
    fs::write(f.root.join("mode"), "success").unwrap();
    f.supervisor
        .probe_retry(&mut f.store, &approved.probe_attempt.id)
        .unwrap();
    drain(&mut f);
    assert_eq!(f.c.probe_state, "ready");
    assert_eq!(calls(&f), 2);
    let replay: ConnectionProbeConfirmation =
        serde_json::from_value(f.store.finalize_proposal(&p.id, 1).unwrap()).unwrap();
    assert_eq!(replay.probe_attempt.state, "succeeded");
    assert!(replay.probe_attempt.process_reaped);
    f.supervisor
        .probe_retry(&mut f.store, &replay.probe_attempt.id)
        .unwrap();
    f.supervisor.probe(&f.store, &f.c.id).unwrap();
    assert!(!f.supervisor.has_owned_io());
    assert_eq!(calls(&f), 2);
    assert_eq!(
        f.store
            .find_connection_probe(&ConnectionProbeFind {
                connection_id: f.c.id.clone(),
                request_key: "retry".into()
            })
            .unwrap()
            .unwrap()
            .probe_attempt
            .id,
        replay.probe_attempt.id
    );
    assert_eq!(
        f.store
            .inspect_connection(&query(&f))
            .unwrap()
            .retry_blocked_reason
            .as_deref(),
        Some("PERMISSION_DENIED")
    );
    if let Some(path) = std::env::var_os("AGENTBATON_CONNECTION_PROBE_FIXTURE") {
        fs::write(path,serde_json::to_vec_pretty(&json!({"request_key":"retry","connection":before,"request":{"connection_id":before.id,"expected_revision":before.revision},"inspection":inspection,"proposal":p,"confirmed":approved,"completed":replay})).unwrap()).unwrap();
    }
}
#[test]
fn generated_timeout_and_output_limit_have_owned_reap_but_no_execution_authority() {
    for mode in ["timeout", "output"] {
        let mut f = fixture();
        initial_failure(&mut f);
        let (_, approved) = approve(&mut f, mode);
        fs::write(f.root.join("mode"), mode).unwrap();
        f.supervisor
            .probe_retry(&mut f.store, &approved.probe_attempt.id)
            .unwrap();
        drain(&mut f);
        let result = f
            .store
            .get_connection_probe(&approved.probe_attempt.id)
            .unwrap();
        assert_eq!(result.probe_attempt.state, "failed");
        assert!(result.probe_attempt.process_reaped);
        assert!(
            f.store
                .inspect_connection(&query(&f))
                .unwrap()
                .retry_eligible
        );
        assert_eq!(calls(&f), 2);
    }
}
#[test]
fn changed_binary_unknown_restart_and_no_stop_evidence_cannot_retry() {
    let mut f = fixture();
    f.store.recover_sessions().unwrap();
    f.c = f.store.get_connection(&f.c.id).unwrap();
    assert_eq!(
        f.store
            .propose_connection_probe(&query(&f), "unknown")
            .unwrap_err()
            .code,
        "OUTCOME_UNKNOWN"
    );
    assert_eq!(calls(&f), 0);
    let mut f = fixture();
    initial_failure(&mut f);
    let (p, _) = approve(&mut f, "restarted");
    f.store.recover_sessions().unwrap();
    f.c = f.store.get_connection(&f.c.id).unwrap();
    let replay: ConnectionProbeConfirmation =
        serde_json::from_value(f.store.finalize_proposal(&p.id, 1).unwrap()).unwrap();
    assert_eq!(replay.probe_attempt.state, "outcome_unknown");
    f.supervisor
        .probe_retry(&mut f.store, &replay.probe_attempt.id)
        .unwrap();
    assert_eq!(calls(&f), 1);
    assert_eq!(
        f.store
            .propose_connection_probe(&query(&f), "never-retry")
            .unwrap_err()
            .code,
        "OUTCOME_UNKNOWN"
    );
    let mut f = fixture();
    initial_failure(&mut f);
    let p = f
        .store
        .propose_connection_probe(&query(&f), "changed")
        .unwrap();
    fs::write(&f.c.executable, "#!/bin/sh\nprintf 'changed'\n").unwrap();
    assert_eq!(
        f.store
            .inspect_connection(&query(&f))
            .unwrap()
            .executable_status,
        "changed"
    );
    assert!(f.store.finalize_proposal(&p.id, 1).is_err());
    assert_eq!(calls(&f), 1);
}
#[test]
fn retained_retry_receipt_survives_database_failure_and_old_generation_cannot_overwrite() {
    let mut f = fixture();
    initial_failure(&mut f);
    let (_, approved) = approve(&mut f, "retry");
    fs::write(f.root.join("mode"), "success").unwrap();
    f.store.conn.execute_batch("CREATE TRIGGER fail_retry_settlement BEFORE UPDATE ON connections WHEN json_extract(NEW.document,'$.probe_state')='ready' BEGIN SELECT RAISE(ABORT,'generated'); END;").unwrap();
    f.supervisor
        .probe_retry(&mut f.store, &approved.probe_attempt.id)
        .unwrap();
    let at = Instant::now();
    loop {
        if f.supervisor.tick(&mut f.store).is_err() {
            break;
        }
        assert!(at.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(f.supervisor.has_owned_io());
    assert_eq!(calls(&f), 2);
    f.store
        .conn
        .execute_batch("DROP TRIGGER fail_retry_settlement")
        .unwrap();
    drain(&mut f);
    assert_eq!(f.c.probe_state, "ready");
    assert_eq!(calls(&f), 2);
    let mut f = fixture();
    initial_failure(&mut f);
    let evidence = f.c.version_evidence.clone().unwrap();
    let (_, approved) = approve(&mut f, "generation");
    let mut changed = approved.connection.clone();
    changed.revision += 10;
    f.store
        .conn
        .execute(
            "UPDATE connections SET document=?1 WHERE id=?2",
            params![encode(&changed).unwrap(), changed.id],
        )
        .unwrap();
    assert!(f
        .store
        .finish_connection_probe(&approved.probe_attempt.id, Ok(evidence), false)
        .is_err());
    assert_eq!(
        f.store.get_connection(&changed.id).unwrap().revision,
        changed.revision
    );
}
#[test]
fn initial_scheduling_failure_proof_allows_new_approval_but_active_binding_does_not() {
    let mut f = fixture();
    let env = f.supervisor.runtime.join("environments");
    std::os::unix::fs::symlink(&f.root, &env).unwrap();
    assert!(f.supervisor.probe(&f.store, &f.c.id).is_err());
    f.store.record_initial_probe_not_started(&f.c.id).unwrap();
    f.c = f.store.get_connection(&f.c.id).unwrap();
    let d = f.store.inspect_connection(&query(&f)).unwrap();
    assert!(d.retry_eligible);
    assert_eq!(d.retry_basis.as_deref(), Some("not_started"));
    let (p, a) = approve(&mut f, "after-no-start");
    assert_eq!(p.snapshot["previous_stop_proof"]["kind"], "not_started");
    assert_eq!(calls(&f), 0);
    let error = f
        .supervisor
        .probe_retry(&mut f.store, &a.probe_attempt.id)
        .unwrap_err();
    assert_eq!(error.side_effect_state, "confirmed");
    f.c = f.store.get_connection(&f.c.id).unwrap();
    assert_eq!(
        f.store
            .get_connection_probe(&a.probe_attempt.id)
            .unwrap()
            .probe_attempt
            .state,
        "not_started"
    );
    assert!(
        f.store
            .inspect_connection(&query(&f))
            .unwrap()
            .retry_eligible
    );
    let mut f = crate::conversation_tests::fixture();
    let _c = f.create("active");
    let mut conn = f.connection.clone();
    conn.probe_state = "failed".into();
    f.store
        .conn
        .execute(
            "UPDATE connections SET document=?1 WHERE id=?2",
            params![encode(&conn).unwrap(), conn.id],
        )
        .unwrap();
    assert_eq!(
        f.store
            .propose_connection_probe(
                &ConnectionInspectRequest {
                    connection_id: conn.id,
                    expected_revision: conn.revision
                },
                "active"
            )
            .unwrap_err()
            .code,
        "WORKSPACE_BUSY"
    );
}

#[test]
fn missing_attempt_origin_and_drift_after_dispatch_stay_unknown_without_retry() {
    let mut f = fixture();
    initial_failure(&mut f);
    let (p, a) = approve(&mut f, "missing");
    f.store
        .conn
        .execute(
            "DELETE FROM idempotency WHERE scope=?1 AND key=?2",
            params![ORIGIN, a.probe_attempt.id],
        )
        .unwrap();
    assert!(f.supervisor.probe(&f.store, &f.c.id).is_err());
    assert!(f
        .store
        .inspect_connection(&ConnectionInspectRequest {
            connection_id: f.c.id.clone(),
            expected_revision: a.connection.revision
        })
        .is_err());
    assert_eq!(
        f.store
            .finalize_proposal(&p.id, 1)
            .unwrap_err()
            .side_effect_state,
        "confirmed"
    );
    assert_eq!(calls(&f), 1);
    f.store
        .conn
        .execute("DELETE FROM approval_proposals WHERE id=?1", [&p.id])
        .unwrap();
    assert!(f.supervisor.probe(&f.store, &f.c.id).is_err());
    let mut f = fixture();
    initial_failure(&mut f);
    let (_, a) = approve(&mut f, "drift");
    fs::write(f.root.join("mode"), "delay").unwrap();
    f.supervisor
        .probe_retry(&mut f.store, &a.probe_attempt.id)
        .unwrap();
    let at = Instant::now();
    while calls(&f) < 2 {
        assert!(at.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(5));
    }
    fs::write(&f.c.executable, "#!/bin/sh\nprintf changed\n").unwrap();
    drain(&mut f);
    let unknown = f.store.get_connection_probe(&a.probe_attempt.id).unwrap();
    assert_eq!(unknown.probe_attempt.state, "outcome_unknown");
    assert!(!unknown.probe_attempt.not_started);
    assert!(!unknown.probe_attempt.process_reaped);
    assert!(unknown.probe_attempt.version_evidence.is_none());
}

#[test]
fn unchanged_bytes_without_execute_mode_are_diagnostic_only_and_never_approved() {
    let mut f = fixture();
    initial_failure(&mut f);
    let q = query(&f);
    let p = f
        .store
        .propose_connection_probe(&q, "before-mode-change")
        .unwrap();
    let original = encode(&f.store.get_connection(&f.c.id).unwrap()).unwrap();
    let seq = f.store.sequence().unwrap();
    fs::set_permissions(&f.c.executable, fs::Permissions::from_mode(0o600)).unwrap();
    let inspection = f.store.inspect_connection(&q).unwrap();
    assert_eq!(inspection.executable_status, "matched");
    assert_eq!(
        inspection.current_executable_sha256.as_deref(),
        Some(f.c.executable_sha256.as_str())
    );
    assert!(!inspection.executable_is_executable);
    assert!(!inspection.retry_eligible);
    assert_eq!(
        inspection.retry_blocked_reason.as_deref(),
        Some("PERMISSION_DENIED")
    );
    assert_eq!(
        f.store
            .propose_connection_probe(&q, "after-mode-change")
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
    assert_eq!(
        f.store.finalize_proposal(&p.id, 1).unwrap_err().code,
        "PERMISSION_DENIED"
    );
    assert_eq!(
        original,
        encode(&f.store.get_connection(&f.c.id).unwrap()).unwrap()
    );
    assert_eq!(seq, f.store.sequence().unwrap());
    assert_eq!(calls(&f), 1);
}
#[test]
fn initial_and_retry_probes_share_one_four_worker_limit() {
    let mut f = fixture();
    let mut attempts = vec![];
    for i in 0..4 {
        if i > 0 {
            let p = f
                .store
                .propose_connection(
                    "Generated",
                    "custom_acp",
                    &f.c.executable,
                    &[],
                    &format!("register-{i}"),
                )
                .unwrap();
            f.c = serde_json::from_value(f.store.finalize_proposal(&p.id, 1).unwrap()).unwrap();
        }
        initial_failure(&mut f);
        let (_, a) = approve(&mut f, &format!("retry-{i}"));
        attempts.push(a.probe_attempt.id);
    }
    fs::write(f.root.join("mode"), "timeout").unwrap();
    for id in attempts {
        f.supervisor.probe_retry(&mut f.store, &id).unwrap();
    }
    assert_eq!(f.supervisor.retry_probes.len(), 4);
    let p = f
        .store
        .propose_connection(
            "Extra",
            "custom_acp",
            &f.c.executable,
            &[],
            "extra-registration",
        )
        .unwrap();
    let extra: ProviderConnection =
        serde_json::from_value(f.store.finalize_proposal(&p.id, 1).unwrap()).unwrap();
    assert_eq!(
        f.supervisor.probe(&f.store, &extra.id).unwrap_err().code,
        "RESOURCE_LIMIT"
    );
    assert!(f.supervisor.probes.is_empty());
    assert_eq!(f.store.get_connection(&extra.id).unwrap().revision, 1);
    f.supervisor.shutdown(&mut f.store).unwrap();
    assert!(!f.supervisor.has_owned_io());
}
