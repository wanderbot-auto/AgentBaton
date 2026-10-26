use agentbaton_core::storage::{Store, TaskState};
use rusqlite::Connection;
use serde_json::json;

#[test]
fn persistence_idempotency_scope_and_optimistic_cancellation() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("store.db");
    let mut store = Store::open(&db).unwrap();
    let first = store.create("project-a", "日本語 task", "key").unwrap();
    assert_eq!(
        store.create("project-a", "日本語 task", "key").unwrap().id,
        first.id
    );
    assert_eq!(store.sequence().unwrap(), 1);
    assert_eq!(
        store
            .create("project-a", "different", "key")
            .unwrap_err()
            .code,
        "CONFIG_CONFLICT"
    );
    let second = store.create("project-b", "different", "key").unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(
        store.cancel(&first.id, 9).unwrap_err().code,
        "STALE_REVISION"
    );
    let cancelled = store.cancel(&first.id, 1).unwrap();
    assert_eq!(cancelled.state, TaskState::Cancelled);
    assert_eq!(cancelled.revision, 2);
    assert_eq!(store.cancel(&first.id, 2).unwrap().revision, 2);
    assert_eq!(store.sequence().unwrap(), 3);
    drop(store);
    let store = Store::open(&db).unwrap();
    assert_eq!(store.get(&first.id).unwrap().state, TaskState::Cancelled);
    let batch = store.events(1, 1).unwrap();
    assert_eq!(batch["events"][0]["aggregate_id"], second.id);
    assert_eq!(batch["next_cursor"], 2);
    let batch = store.events(2, 100).unwrap();
    assert_eq!(
        batch["events"][0]["payload"]["stop_evidence"],
        "no_attempt_dispatched"
    );
    assert_eq!(store.events(100, 1).unwrap_err().code, "STALE_REVISION");
    assert_eq!(store.events(0, 0).unwrap_err().code, "INVALID_REQUEST");
    assert_eq!(store.get("missing").unwrap_err().code, "NOT_FOUND");
}
#[test]
fn event_failure_rolls_back_task_and_idempotency_record() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("store.db");
    let mut store = Store::open(&db).unwrap();
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch("CREATE TRIGGER reject_events BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'simulated disk failure'); END;").unwrap();
    assert!(store.create("p", "goal", "k").is_err());
    assert!(store.list().unwrap().is_empty());
    assert_eq!(store.sequence().unwrap(), 0);
    conn.execute_batch("DROP TRIGGER reject_events").unwrap();
    assert!(store.create("p", "changed goal", "k").is_ok());
}
#[test]
fn future_and_unknown_database_are_not_modified() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("store.db");
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch("PRAGMA user_version=3; CREATE TABLE sentinel(value TEXT);")
        .unwrap();
    drop(conn);
    let before = std::fs::read(&db).unwrap();
    assert_eq!(Store::open(&db).err().unwrap().code, "VERSION_UNSUPPORTED");
    assert_eq!(std::fs::read(&db).unwrap(), before);
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch("PRAGMA user_version=0").unwrap();
    drop(conn);
    assert_eq!(Store::open(&db).err().unwrap().code, "VERSION_UNSUPPORTED");
}
#[test]
fn active_cancel_never_claims_executor_stopped() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("store.db");
    let mut store = Store::open(&db).unwrap();
    let task = store.create("p", "goal", "k").unwrap();
    let conn = Connection::open(&db).unwrap();
    let mut running = task.clone();
    running.state = TaskState::Running;
    conn.execute(
        "UPDATE tasks SET document=?1 WHERE id=?2",
        [serde_json::to_string(&running).unwrap(), task.id.clone()],
    )
    .unwrap();
    let cancelled = store.cancel(&task.id, 1).unwrap();
    assert_eq!(cancelled.state, TaskState::CancelRequested);
    assert_eq!(
        store.events(1, 100).unwrap()["events"][0]["payload"]["stop_evidence"],
        json!(null)
    );
}
#[test]
fn invalid_goals_do_not_create_rows() {
    let mut store = Store::open(std::path::Path::new(":memory:")).unwrap();
    for goal in ["   ".to_string(), "x".repeat(16385)] {
        assert_eq!(
            store.create("p", &goal, "k").unwrap_err().code,
            "INVALID_REQUEST"
        );
    }
    assert!(store.list().unwrap().is_empty());
}
#[test]
fn task_limit_returns_explicit_error_instead_of_incomplete_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("store.db");
    let mut store = Store::open(&db).unwrap();
    let task = store.create("p", "goal", "k").unwrap();
    let mut conn = Connection::open(&db).unwrap();
    let tx = conn.transaction().unwrap();
    let doc = serde_json::to_string(&task).unwrap();
    for i in 0..1000 {
        tx.execute(
            "INSERT INTO tasks(id,document) VALUES(?1,?2)",
            rusqlite::params![format!("extra-{i}"), doc],
        )
        .unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(store.list().unwrap_err().code, "RESOURCE_LIMIT");
    assert_eq!(store.get(&task.id).unwrap().id, task.id);
}
