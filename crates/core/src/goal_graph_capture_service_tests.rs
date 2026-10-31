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

// These handles perform real bounded FD reads. Their origin deliberately has no
// approved Graph/producer receipt: successful I/O must not manufacture authority.
fn retained_real_capture(
    prepared: evidence::PreparedCandidateCapture,
    wait_for_release: bool,
) -> (
    GraphWorkers,
    mpsc::Sender<()>,
    mpsc::Receiver<()>,
    Arc<AtomicBool>,
) {
    let origin = origin(prepared.root.clone());
    let queued_at = Instant::now();
    let deadline = prepared.deadline;
    let cancel = prepared.cancelled.clone();
    let thread_cancel = cancel.clone();
    let (release, wait) = mpsc::channel();
    let (entered, ready) = mpsc::channel();
    let done = Arc::new(AtomicBool::new(false));
    let thread_done = done.clone();
    let thread = std::thread::spawn(move || {
        entered.send(()).unwrap();
        if wait_for_release {
            // Bound even a regression that accidentally joins this gated
            // thread before the test can send its release; normal service
            // needs no elapsed-time threshold or sleep to pass.
            let until = Instant::now() + Duration::from_secs(5);
            while !thread_cancel.load(Ordering::Relaxed) && Instant::now() < until {
                match wait.recv_timeout(Duration::from_millis(10)) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        }
        let observed = observe_capture_worker(queued_at, || prepared.execute());
        thread_done.store(true, Ordering::SeqCst);
        observed
    });
    let mut workers = GraphWorkers::default();
    workers.captures.insert(
        origin.id.clone(),
        CaptureWorker {
            origin,
            cancel,
            thread,
            deadline,
            unknown_recorded: false,
            queued_at,
        },
    );
    (workers, release, ready, done)
}
fn wait_for_real_capture_completion(workers: &GraphWorkers) {
    let until = Instant::now() + Duration::from_secs(5);
    while !workers.captures.values().all(|w| w.thread.is_finished()) {
        assert!(
            Instant::now() < until,
            "bounded owned test reader must finish"
        );
        std::thread::yield_now();
    }
}
fn no_capture_authority_or_settlement(store: &Store, sequence: u64) {
    assert_eq!(store.sequence().unwrap(), sequence);
    for table in [
        "goal_graph_candidates",
        "goal_graph_jobs",
        "goal_run_reservations",
    ] {
        assert_eq!(
            store
                .conn
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0,
            "real thread completion is not a {table} authority"
        );
    }
}

#[test]
fn finished_real_capture_is_joined_once_but_cannot_settle_without_original_authority() {
    let (temp, prepared) = fixture();
    let mut store = empty_store(&temp);
    let sequence = store.sequence().unwrap();
    let (mut workers, _release, ready, done) = retained_real_capture(prepared, false);
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    wait_for_real_capture_completion(&workers);
    assert!(done.load(Ordering::SeqCst));
    // The production poll really joins/removes the handle, then the unchanged
    // fresh Store verifier refuses the deliberately unauthorised origin.
    let error = workers.poll_finished_captures(&mut store).err().unwrap();
    assert_eq!(error.code, "INTERNAL_ERROR");
    assert!(workers.captures.is_empty());
    no_capture_authority_or_settlement(&store, sequence);
    // No handle remains to rejoin or call settlement a second time.
    workers.poll_finished_captures(&mut store).unwrap();
    no_capture_authority_or_settlement(&store, sequence);
}

#[test]
fn unfinished_owned_capture_poll_returns_without_waiting_or_releasing_the_handle() {
    let (temp, prepared) = fixture();
    let mut store = empty_store(&temp);
    let sequence = store.sequence().unwrap();
    let (mut workers, release, ready, done) = retained_real_capture(prepared, true);
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    // The thread cannot finish until the later release; a blocking join here
    // would prevent this same test thread from ever issuing that release.
    workers.poll_finished_captures(&mut store).unwrap();
    assert!(workers.has_workers());
    assert!(!done.load(Ordering::SeqCst));
    assert!(!workers
        .captures
        .values()
        .next()
        .unwrap()
        .thread
        .is_finished());
    no_capture_authority_or_settlement(&store, sequence);
    release.send(()).unwrap();
    wait_for_real_capture_completion(&workers);
    assert_eq!(
        workers
            .poll_finished_captures(&mut store)
            .err()
            .unwrap()
            .code,
        "INTERNAL_ERROR"
    );
    assert!(workers.captures.is_empty());
    no_capture_authority_or_settlement(&store, sequence);
}

#[test]
fn failed_start_receipt_keeps_even_a_finished_owned_handle_for_fatal_drop() {
    let (temp, prepared) = fixture();
    let mut store = empty_store(&temp);
    let sequence = store.sequence().unwrap();
    let (mut workers, _release, ready, done) = retained_real_capture(prepared, false);
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    wait_for_real_capture_completion(&workers);
    let id = workers.captures.keys().next().unwrap().clone();
    // Call the production mark-then-poll path, not a test copy of its branch.
    // No original Graph/job/stop provenance exists, so mark must fail before
    // the new completion service can remove or settle this finished handle.
    assert_eq!(
        workers
            .mark_started_capture_and_poll(&mut store, &id)
            .err()
            .unwrap()
            .code,
        "INTERNAL_ERROR"
    );
    assert!(workers.captures.contains_key(&id));
    no_capture_authority_or_settlement(&store, sequence);
    drop(workers);
    assert!(done.load(Ordering::SeqCst));
    no_capture_authority_or_settlement(&store, sequence);
}

#[test]
fn cancelled_real_capture_poll_cannot_create_a_candidate_or_an_extra_settlement() {
    let (temp, prepared) = fixture();
    prepared.cancelled.store(true, Ordering::Relaxed);
    let mut store = empty_store(&temp);
    let sequence = store.sequence().unwrap();
    let (mut workers, _release, ready, done) = retained_real_capture(prepared, false);
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    wait_for_real_capture_completion(&workers);
    assert!(done.load(Ordering::SeqCst));
    assert_eq!(
        workers
            .poll_finished_captures(&mut store)
            .err()
            .unwrap()
            .code,
        "INTERNAL_ERROR"
    );
    assert!(!workers.has_workers());
    no_capture_authority_or_settlement(&store, sequence);
    workers.poll_finished_captures(&mut store).unwrap();
    no_capture_authority_or_settlement(&store, sequence);
}
