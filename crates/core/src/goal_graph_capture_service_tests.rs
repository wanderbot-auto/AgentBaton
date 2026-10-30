//! Private worker/FD/lifecycle controls; no Provider or fabricated approval.
use super::*;
use std::{
    fs,
    os::unix::fs::{symlink, MetadataExt, PermissionsExt},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

fn fixture() -> (tempfile::TempDir, evidence::PreparedCandidateCapture) {
    let temp = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in(std::env::temp_dir().canonicalize().unwrap())
        .unwrap();
    fs::write(temp.path().join("report.txt"), b"actual capture\n").unwrap();
    let metadata = fs::metadata(temp.path()).unwrap();
    let root = agentbaton_workspaces::WorkspaceIdentity {
        canonical_root: temp.path().to_path_buf(),
        device: metadata.dev(),
        inode: metadata.ino(),
        kind: agentbaton_workspaces::WorkspaceKind::Directory,
    };
    let prepared = evidence::PreparedCandidateCapture {
        root,
        inputs: vec![],
        outputs: vec![(
            "output:report".into(),
            ArtifactRequirement {
                artifact_id: "report".into(),
                relative_path: "report.txt".into(),
                max_bytes: 1024,
                expected_sha256: None,
            },
        )],
        cancelled: Arc::new(AtomicBool::new(false)),
        deadline: Instant::now() + Duration::from_secs(5),
    };
    (temp, prepared)
}
fn origin(root: agentbaton_workspaces::WorkspaceIdentity) -> CaptureOrigin {
    CaptureOrigin {
        id: "not-an-authorized-capture".into(),
        run_id: "no-authorized-run".into(),
        node_id: "no-authorized-node".into(),
        plan_revision: 1,
        plan_digest: "not-authority".into(),
        attempt_id: "no-authorized-attempt".into(),
        session_id: "no-authorized-session".into(),
        workspace_id: "no-authorized-workspace".into(),
        root,
        writer_epoch: 0,
        stop_receipt_sequence: 0,
        capability_digest: "not-authority".into(),
        elapsed_at_stop_ms: 37,
        prepared_at_ms: 0,
        runtime_reservation_ms: 4000,
        consumed_inputs: vec![],
    }
}
fn empty_store(temp: &tempfile::TempDir) -> Store {
    Store::open(&temp.path().join("test.sqlite3")).unwrap()
}

#[test]
fn real_capture_thread_observes_queue_start_completion_and_later_join() {
    let (_temp, prepared) = fixture();
    let root = prepared.root.clone();
    let queued_at = Instant::now();
    let (allow_start, start) = mpsc::channel();
    let (finished, complete) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        start.recv().unwrap();
        let outcome = observe_capture_worker(queued_at, || prepared.execute());
        // Actual execute has returned, not a synthetic completion timestamp.
        finished.send(()).unwrap();
        outcome
    });
    std::thread::sleep(Duration::from_millis(25));
    allow_start.send(()).unwrap();
    complete.recv().unwrap();
    std::thread::sleep(Duration::from_millis(25));
    let completion = CaptureCompletion::joined(thread.join());
    let observed = capture_millis(queued_at.elapsed());
    let CaptureCompletion::Joined { result, timing } = completion else {
        panic!("real join required")
    };
    let captured = match *result {
        Ok(value) => value,
        Err(error) => panic!("{}", error.code),
    };
    assert_eq!(
        captured.outputs[0].1.bytes,
        b"actual capture\n".len() as u64
    );
    let timing = timing.unwrap();
    assert!(timing.started_queue_ms >= 25);
    assert!(timing.completed_queue_ms >= timing.started_queue_ms);
    assert!(observed >= timing.completed_queue_ms + 25);
    assert!(timing.elapsed_ms <= timing.completed_queue_ms);
    let fields = serde_json::to_value(CaptureSettlementTiming::observed(
        &origin(root),
        Some(timing),
        true,
        observed,
    ))
    .unwrap();
    assert_eq!(fields.as_object().unwrap().len(), 6);
    assert_eq!(fields["producer_elapsed_ms"], 37);
    assert_eq!(fields["runtime_reservation_ms"], 4000);
    assert_eq!(fields["capture_join_observed_queue_ms"], observed);
    assert_eq!(
        fields["capture_worker_started_queue_ms"],
        timing.started_queue_ms
    );
    assert_eq!(
        fields["capture_worker_completed_queue_ms"],
        timing.completed_queue_ms
    );
    assert_eq!(fields["capture_worker_elapsed_ms"], timing.elapsed_ms);
}

#[test]
fn cancelled_real_capture_keeps_observed_thread_metrics_and_no_candidate() {
    let (_temp, prepared) = fixture();
    prepared.cancelled.store(true, Ordering::Relaxed);
    let root = prepared.root.clone();
    let queued_at = Instant::now();
    let completion = CaptureCompletion::joined(
        std::thread::spawn(move || observe_capture_worker(queued_at, || prepared.execute())).join(),
    );
    let CaptureCompletion::Joined { result, timing } = completion else {
        panic!("real join required")
    };
    assert_eq!(result.err().unwrap().code, "CANCEL_REQUESTED");
    let fields = serde_json::to_value(CaptureSettlementTiming::observed(
        &origin(root),
        timing,
        true,
        capture_millis(queued_at.elapsed()),
    ))
    .unwrap();
    for field in [
        "capture_worker_started_queue_ms",
        "capture_worker_completed_queue_ms",
        "capture_join_observed_queue_ms",
        "capture_worker_elapsed_ms",
    ] {
        assert!(fields[field].as_u64().is_some());
    }
}

#[test]
fn original_expired_deadline_is_not_refreshed_by_worker_start() {
    let (_temp, mut prepared) = fixture();
    let queued_at = Instant::now();
    prepared.deadline = queued_at;
    std::thread::sleep(Duration::from_millis(10));
    let outcome = observe_capture_worker(queued_at, || prepared.execute());
    assert_eq!(outcome.result.err().unwrap().code, "TIMEOUT");
    assert!(outcome.timing.started_queue_ms >= 10);
}

#[test]
fn byte_identical_symlink_replacement_is_rejected_by_actual_capture() {
    let (temp, prepared) = fixture();
    fs::rename(
        temp.path().join("report.txt"),
        temp.path().join("other.txt"),
    )
    .unwrap();
    symlink("other.txt", temp.path().join("report.txt")).unwrap();
    let outcome = observe_capture_worker(Instant::now(), || prepared.execute());
    assert_eq!(outcome.result.err().unwrap().code, "INPUT_CHANGED");
}

#[test]
fn real_panic_join_has_no_fabricated_worker_start_or_completion_metrics() {
    let (_temp, prepared) = fixture();
    let root = prepared.root.clone();
    let queued_at = Instant::now();
    let thread = std::thread::spawn(move || {
        observe_capture_worker(queued_at, || panic!("controlled actual panic"))
    });
    let completion = CaptureCompletion::joined(thread.join());
    let CaptureCompletion::Joined { result, timing } = completion else {
        panic!("actual panic handle joined")
    };
    assert_eq!(result.err().unwrap().code, "OUTCOME_UNKNOWN");
    assert!(timing.is_none());
    let fields = serde_json::to_value(CaptureSettlementTiming::observed(
        &origin(root),
        timing,
        true,
        capture_millis(queued_at.elapsed()),
    ))
    .unwrap();
    assert!(fields["capture_join_observed_queue_ms"].as_u64().is_some());
    for field in [
        "capture_worker_started_queue_ms",
        "capture_worker_completed_queue_ms",
        "capture_worker_elapsed_ms",
    ] {
        assert!(fields[field].is_null());
    }
}

#[test]
fn not_started_metrics_are_null_instead_of_zero_or_a_join() {
    let (_temp, prepared) = fixture();
    let fields = serde_json::to_value(CaptureSettlementTiming::observed(
        &origin(prepared.root),
        None,
        false,
        1234,
    ))
    .unwrap();
    assert_eq!(fields["producer_elapsed_ms"], 37);
    assert_eq!(fields["runtime_reservation_ms"], 4000);
    for field in [
        "capture_worker_started_queue_ms",
        "capture_worker_completed_queue_ms",
        "capture_join_observed_queue_ms",
        "capture_worker_elapsed_ms",
    ] {
        assert!(fields[field].is_null());
    }
}

#[test]
fn process_local_clock_never_mints_a_prepared_job_or_authority() {
    let (temp, prepared) = fixture();
    let mut store = empty_store(&temp);
    let origin = origin(prepared.root);
    let events_before = store.sequence().unwrap();
    store
        .graph_capture_clocks
        .insert(origin.id.clone(), Instant::now());
    let mut workers = GraphWorkers::default();
    workers.tick(&mut store).unwrap();
    assert!(!workers.has_workers());
    assert_eq!(store.sequence().unwrap(), events_before);
    assert_eq!(
        store
            .conn
            .query_row("SELECT count(*) FROM goal_graph_jobs", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        store
            .conn
            .query_row("SELECT count(*) FROM goal_graph_candidates", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert!(store
        .prepare_graph_capture_worker(&origin, Instant::now())
        .is_err());
    assert_eq!(store.sequence().unwrap(), events_before);
}

#[test]
fn shutdown_with_only_a_clock_hint_cannot_start_capture() {
    let (temp, prepared) = fixture();
    let mut store = empty_store(&temp);
    let origin = origin(prepared.root);
    store
        .graph_capture_clocks
        .insert(origin.id.clone(), Instant::now());
    let events_before = store.sequence().unwrap();
    let mut workers = GraphWorkers::default();
    workers.shutdown(&mut store).unwrap();
    assert!(workers.stopping);
    assert!(!workers.has_workers());
    assert!(store.graph_capture_clocks.contains_key(&origin.id));
    assert_eq!(store.sequence().unwrap(), events_before);
}

#[test]
fn owned_graph_drop_really_cancels_and_joins_the_wrapped_reader() {
    let (temp, _) = fixture();
    let done = Arc::new(AtomicBool::new(false));
    let checked = done.clone();
    let mut workers = GraphWorkers::default();
    workers.install_drop_fixture(temp.path().to_path_buf(), move || {
        checked.store(true, Ordering::SeqCst)
    });
    assert!(workers.has_workers());
    drop(workers);
    assert!(done.load(Ordering::SeqCst));
}

#[test]
fn nonprivate_directory_mode_alone_is_rejected_by_actual_capture() {
    let (temp, prepared) = fixture();
    let before = fs::metadata(temp.path()).unwrap();
    let report = temp.path().join("report.txt");
    let original_bytes = fs::read(&report).unwrap();
    assert_eq!(before.mode() & 0o7777, 0o700);
    assert_eq!(before.uid(), rustix::process::geteuid().as_raw());
    assert_eq!(original_bytes, b"actual capture\n");
    // Alter only this test-owned root's mode, never global umask or production.
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o755)).unwrap();
    let changed_root = fs::metadata(temp.path()).unwrap();
    assert_eq!(changed_root.mode() & 0o7777, 0o755);
    assert_eq!(
        (changed_root.dev(), changed_root.ino(), changed_root.uid()),
        (before.dev(), before.ino(), before.uid())
    );
    assert_eq!(fs::read(&report).unwrap(), original_bytes);
    let outcome = observe_capture_worker(Instant::now(), || prepared.execute());
    assert_eq!(outcome.result.err().unwrap().code, "INPUT_CHANGED");
    assert_eq!(fs::read(&report).unwrap(), original_bytes);
}
