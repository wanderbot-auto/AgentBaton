use agentbaton_core::storage::Store;
use rusqlite::{params, Connection};
use serde_json::{json, Value};

#[test]
fn task_transcript_pages_by_scoped_sequence_and_does_not_include_unrelated_events() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("baton.sqlite3");
    let mut store = Store::open(&path).unwrap();
    let task = store.create("local", "Task A", "history-task-a").unwrap();
    let other = store.create("local", "Task B", "history-task-b").unwrap();
    let fixture = Connection::open(&path).unwrap();
    fixture.pragma_update(None, "foreign_keys", "OFF").unwrap();
    for (session, task_id) in [("sa", &task.id), ("sa2", &task.id), ("sb", &other.id)] {
        // Populate only the legacy session-to-task association required by the reader.
        fixture.execute(
            "INSERT INTO sessions(id,task_id,project_id,workspace_id,connection_id,grant_id,root_grant_id,state,document,idempotency_key,payload)
             VALUES(?1,?2,'p','w','c','g','g','stopped','{}',?1,'{}')",
            params![session, task_id],
        ).unwrap();
    }
    let add =
        |session: &str, kind: &str, payload: Value| {
            fixture.execute("INSERT INTO events(document) VALUES(?1)", [json!({
            "sequence":0,"event_id":uuid::Uuid::new_v4().to_string(),"aggregate_id":session,
            "revision":1,"kind":kind,"occurred_at":"2026-01-01T00:00:00Z","payload":payload,
            "source":"fixture"
        }).to_string()]).unwrap();
            fixture.last_insert_rowid() as u64
        };
    let old = add(
        "sa",
        "session.message_queued",
        json!({"text":"Old task message"}),
    );
    let middle = add(
        "sa2",
        "session.update",
        json!({"update":{"kind":{"kind":"agent_text","text":"Stored reply"}}}),
    );
    add(
        "sa",
        "session.update",
        json!({"update":{"kind":{"kind":"tool_call"}}}),
    );
    let recent = add(
        "sa",
        "session.message_queued",
        json!({"text":"New task message"}),
    );
    for _ in 0..120 {
        add(
            "sb",
            "session.message_queued",
            json!({"text":"Unrelated message"}),
        );
    }

    let sequences = |page: &Value| -> Vec<u64> {
        page["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|event| event["sequence"].as_u64().unwrap())
            .collect()
    };
    let latest = store.task_history(&task.id, None, None, 2).unwrap();
    assert_eq!(latest["task_id"], task.id);
    assert_eq!(sequences(&latest), [middle, recent]);
    assert_eq!(latest["next_cursor"], middle);
    let earlier = store.task_history(&task.id, Some(middle), None, 2).unwrap();
    assert_eq!(sequences(&earlier), [old]);
    assert!(earlier["next_cursor"].is_null());

    let first_update = store.task_history(&task.id, None, Some(old), 1).unwrap();
    assert_eq!(sequences(&first_update), [middle]);
    assert_eq!(first_update["next_cursor"], middle);
    let last_update = store.task_history(&task.id, None, Some(middle), 1).unwrap();
    assert_eq!(sequences(&last_update), [recent]);
    assert!(last_update["next_cursor"].is_null());
    assert!(store
        .task_history(&task.id, Some(old), Some(recent), 1)
        .is_err());
    assert!(store.task_history(&task.id, None, None, 101).is_err());
    assert!(store.task_history("missing", None, None, 1).is_err());
}
