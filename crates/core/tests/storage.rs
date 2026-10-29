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
    let error = store.create("p", "goal", "k").unwrap_err();
    let diagnostic = error.details_ref.as_deref().unwrap();
    assert!(diagnostic
        .strip_prefix("sqlite-code:")
        .unwrap()
        .parse::<i32>()
        .is_ok());
    let serialized = serde_json::to_string(&error).unwrap();
    assert!(!serialized.contains("simulated disk failure"));
    assert!(!serialized.contains("reject_events"));
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
    conn.execute_batch("PRAGMA user_version=15; CREATE TABLE sentinel(value TEXT);")
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

#[test]
fn large_journal_pages_bound_bytes_and_preserve_every_sequence() {
    use agentbaton_core::storage::{Event, MAX_EVENT_PAGE_BYTES};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bounded-journal.db");
    let store = Store::open(&path).unwrap();
    let db = Connection::open(&path).unwrap();
    for sequence in 1..=24 {
        let event = Event {
            sequence: 0,
            event_id: format!("event-{sequence}"),
            aggregate_id: "fixture".into(),
            revision: sequence,
            kind: "session.provider_update".into(),
            occurred_at: "2026-10-26T00:00:00Z".into(),
            payload: serde_json::json!({"text":"界".repeat(220_000),"index":sequence}),
            source: "generated_fixture".into(),
        };
        db.execute(
            "INSERT INTO events(document) VALUES(?1)",
            [serde_json::to_string(&event).unwrap()],
        )
        .unwrap();
    }
    let mut cursor = 0;
    let mut sequences = Vec::new();
    let mut pages = 0;
    while cursor < 24 {
        let page = store.events(cursor, 500).unwrap();
        assert_eq!(page["sequence"], 24);
        assert_eq!(page["gap"], false);
        assert!(page["page_bytes"].as_u64().unwrap() <= MAX_EVENT_PAGE_BYTES as u64);
        assert!(page.to_string().len() < 2 * MAX_EVENT_PAGE_BYTES);
        let events = page["events"].as_array().unwrap();
        assert!(!events.is_empty());
        for event in events {
            let sequence = event["sequence"].as_u64().unwrap();
            assert_eq!(
                sequence,
                cursor + 1,
                "Byte pagination must not skip a sequence"
            );
            assert_eq!(event["payload"]["index"], sequence);
            sequences.push(sequence);
            cursor = sequence;
        }
        assert_eq!(page["next_cursor"], cursor);
        assert_eq!(page["byte_limited"], cursor < 24);
        pages += 1;
    }
    assert!(pages >= 4);
    assert_eq!(sequences, (1..=24).collect::<Vec<_>>());
    assert!(store.events(cursor, 500).unwrap()["events"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn oversized_single_journal_record_is_rejected_before_decoding_and_not_skipped() {
    use agentbaton_core::storage::MAX_EVENT_PAGE_BYTES;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("oversized-journal.db");
    let store = Store::open(&path).unwrap();
    let db = Connection::open(&path).unwrap();
    // Invalid content deliberately proves the byte guard precedes JSON decode.
    db.execute(
        "INSERT INTO events(document) VALUES(?1)",
        ["x".repeat(MAX_EVENT_PAGE_BYTES + 1)],
    )
    .unwrap();
    for limit in [1, 500] {
        let error = store.events(0, limit).unwrap_err();
        assert_eq!(error.code, "RESOURCE_LIMIT");
        assert_eq!(error.details_ref.as_deref(), Some("event-sequence:1"));
    }
    assert_eq!(store.sequence().unwrap(), 1);
}
