//! Real approved producer/stop/capture paths plus an unrelated corrupt frontier.
//! No injected permission, stop receipt, candidate or successful settlement.
use super::*;
use std::sync::atomic::AtomicBool;

#[derive(Clone, Copy, Debug)]
enum CapturePath {
    ReadHeldUntilFatalDrop,
    NoPreparedJob,
    MissingClock,
    ExpiredClock,
}

fn frontier_fault_after_capture_path(path: CapturePath) {
    let mut f = Fixture::new(false);
    let (run, session) = f.goal_session_with_graph(true);
    f.wait_turn(&session.id);
    f.daemon.service_hinted_goal_stops().unwrap();
    f.wait_stopped(&session.id);
    f.daemon.service_hinted_goal_stops().unwrap();
    let job: String = f
        .daemon
        .store
        .conn
        .query_row(
            "SELECT id FROM goal_graph_jobs WHERE run_id=?1 AND state='prepared'",
            [&run.id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(f.daemon.store.graph_capture_clocks.contains_key(&job));
    assert!(!f.daemon.graph_workers.has_workers());
    let read_done = Arc::new(AtomicBool::new(false));
    let joined_before_unlock = Arc::new(AtomicBool::new(false));
    match path {
        CapturePath::ReadHeldUntilFatalDrop => {
            let read = read_done.clone();
            let joined = joined_before_unlock.clone();
            let runtime = f._directory.path().join("r");
            let (read_ready, original_read) = std::sync::mpsc::channel();
            f.daemon.graph_workers.install_capture_read_probe(
                original_read,
                move |success, cancel| {
                    assert!(
                        success,
                        "the original approved capture must actually read its files"
                    );
                    read.store(true, Ordering::SeqCst);
                    read_ready.send(()).unwrap();
                    // Keep this real completed read owned until fatal teardown. No
                    // sleep chooses a race; this alarm only bounds a broken test.
                    let until = Instant::now() + Duration::from_secs(5);
                    while !cancel.load(Ordering::Relaxed) {
                        assert!(
                            Instant::now() < until,
                            "fatal teardown must cancel its owned reader"
                        );
                        std::thread::yield_now();
                    }
                    assert_eq!(Daemon::bind(&runtime).err().unwrap().code, "WORKSPACE_BUSY");
                    // Only set after all worker assertions, so an ignored join
                    // panic cannot masquerade as successful owned cleanup.
                    joined.store(true, Ordering::SeqCst);
                },
            );
        }
        CapturePath::NoPreparedJob => {
            assert_eq!(
                f.daemon
                    .store
                    .conn
                    .execute(
                        "UPDATE goal_graph_jobs SET state='outcome_unknown' WHERE id=?1",
                        [&job],
                    )
                    .unwrap(),
                1
            );
        }
        CapturePath::MissingClock => {
            assert!(f.daemon.store.graph_capture_clocks.remove(&job).is_some());
        }
        CapturePath::ExpiredClock => {
            // Negative timeout injection only. Never advance a clock or alter
            // a Grant/Origin/stop/lease to grant additional runtime or work.
            *f.daemon.store.graph_capture_clocks.get_mut(&job).unwrap() =
                Instant::now().checked_sub(Duration::from_secs(5)).unwrap();
        }
    }
    // An unconfirmed, ordinary Grant proposal supplies a real FK parent only.
    // The deliberately malformed OTHER run/capability must never confer power.
    let foreign = f
        .daemon
        .store
        .propose_grant(
            &run.project_id,
            &f.workspace.id,
            &f.connection,
            4,
            1,
            30_000,
            "unconfirmed-frontier-fault-parent",
        )
        .unwrap();
    f.daemon.store.conn.execute(
        "INSERT INTO goal_runs(id,scope_id,project_id,task_id,revision,state,document) VALUES(?1,?2,?3,?4,1,'awaiting_acceptance','{}')",
        rusqlite::params![foreign.id, run.scope_id, run.project_id, run.task_id],
    ).unwrap();
    f.daemon
        .store
        .conn
        .execute(
            "INSERT INTO goal_graph_capabilities VALUES(?1,?1,1,'{}')",
            [&foreign.id],
        )
        .unwrap();
    assert!(!f
        .daemon
        .store
        .conn
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .exists([])
        .unwrap());
    let attempts: i64 = f
        .daemon
        .store
        .conn
        .query_row("SELECT count(*) FROM attempts", [], |r| r.get(0))
        .unwrap();
    let Fixture {
        daemon, _directory, ..
    } = f;
    let runtime = _directory.path().join("r");
    let error = daemon.run(Arc::new(AtomicBool::new(false))).unwrap_err();
    assert_eq!(
        error.code, "INTERNAL_ERROR",
        "the original full frontier refusal must propagate for {path:?}"
    );
    let db = rusqlite::Connection::open_with_flags(
        runtime.join("baton.sqlite3"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        attempts,
        "no later Actor dispatch"
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM goal_graph_candidates", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let started: i64 = db.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='goal.capture_worker_started' AND json_extract(document,'$.payload.job_id')=?1", [&job], |r| r.get(0)).unwrap();
    let settled: i64 = db.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='goal.capture_settled' AND json_extract(document,'$.payload.job_id')=?1", [&job], |r| r.get(0)).unwrap();
    let state: String = db
        .query_row(
            "SELECT state FROM goal_graph_jobs WHERE id=?1",
            [&job],
            |r| r.get(0),
        )
        .unwrap();
    let reservation: String = db
        .query_row(
            "SELECT document FROM goal_run_reservations WHERE attempt_id=?1",
            [&session.attempt_id],
            |r| r.get(0),
        )
        .unwrap();
    let reservation: Value = serde_json::from_str(&reservation).unwrap();
    match path {
        CapturePath::ReadHeldUntilFatalDrop => {
            assert!(read_done.load(Ordering::SeqCst));
            assert!(joined_before_unlock.load(Ordering::SeqCst));
            assert_eq!((started, settled, state.as_str()), (1, 0, "running"));
            assert_eq!(reservation["settled"], false);
        }
        CapturePath::NoPreparedJob | CapturePath::MissingClock => {
            assert_eq!(
                (started, settled, state.as_str()),
                (0, 0, "outcome_unknown")
            );
            assert_eq!(reservation["settled"], false);
        }
        CapturePath::ExpiredClock => {
            assert_eq!((started, settled, state.as_str()), (0, 1, "failed"));
            assert_eq!(reservation["settled"], true);
            assert!(reservation["actual_runtime_ms"].as_u64().unwrap() >= 5000);
        }
    }
    assert_eq!(
        db.query_row(
            "SELECT json_extract(document,'$.consumed') FROM approval_proposals WHERE id=?1",
            [&foreign.id],
            |r| r.get::<_, bool>(0)
        )
        .unwrap(),
        false
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM goal_graph_acceptances", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    drop(db);
    // The original owned drop has fully released the writer only after join.
    // This also invokes ordinary unknown-outcome recovery; it is not a
    // replay of any failed fixture or proof of a successful capture receipt.
    let successor = Daemon::bind(&runtime).unwrap();
    assert_eq!(
        successor
            .store
            .conn
            .query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        attempts
    );
    drop(successor);
}

#[test]
fn actual_capture_read_then_late_frontier_error_joins_before_writer_handoff() {
    frontier_fault_after_capture_path(CapturePath::ReadHeldUntilFatalDrop);
}
#[test]
fn actual_capture_nonfatal_returns_still_propagate_same_turn_frontier_error() {
    for path in [
        CapturePath::NoPreparedJob,
        CapturePath::MissingClock,
        CapturePath::ExpiredClock,
    ] {
        frontier_fault_after_capture_path(path);
    }
}
