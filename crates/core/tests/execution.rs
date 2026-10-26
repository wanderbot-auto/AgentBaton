use agentbaton_core::{
    execution::{ExecutionBinding, IntentState, StopEvidence},
    storage::{Store, TaskState},
};
use rusqlite::{params, Connection};
use std::path::Path;
fn binding() -> ExecutionBinding {
    ExecutionBinding {
        connection_id: "codex-work".into(),
        environment_id: "managed-1".into(),
        identity_ref: "native-secret-handle".into(),
        host_id: "local".into(),
        workspace_id: "workspace-1".into(),
        config_revision: "sha256:config".into(),
        input_revision: "commit:abc".into(),
        capability_snapshot_id: "caps-1".into(),
        policy_revision: "grant-1".into(),
    }
}
fn budgets(store: &mut Store, task_id: &str) {
    store.configure_budget(true, "p", 3, 1).unwrap();
    store.configure_budget(false, task_id, 2, 1).unwrap();
}
#[test]
fn prepare_is_atomic_idempotent_and_binding_immutable() {
    let mut store = Store::open(Path::new(":memory:")).unwrap();
    let task = store.create("p", "goal", "k").unwrap();
    budgets(&mut store, &task.id);
    let intent = store
        .prepare_attempt(&task.id, 1, binding(), "dispatch-k")
        .unwrap();
    assert_eq!(intent.state, IntentState::Prepared);
    assert_eq!(store.get(&task.id).unwrap().revision, 2);
    assert_eq!(
        store
            .prepare_attempt(&task.id, 1, binding(), "dispatch-k")
            .unwrap()
            .id,
        intent.id
    );
    assert_eq!(store.budget(true, "p").unwrap().unwrap().consumed, 1);
    let mut changed = binding();
    changed.identity_ref = "different-account".into();
    assert_eq!(
        store
            .prepare_attempt(&task.id, 2, changed, "dispatch-k")
            .unwrap_err()
            .code,
        "CONFIG_CONFLICT"
    );
    assert_eq!(
        store
            .prepare_attempt(&task.id, 2, binding(), "second")
            .unwrap_err()
            .code,
        "WORKSPACE_BUSY"
    );
    let state = store.get(&task.id).unwrap();
    assert_eq!(state.state, TaskState::Draft);
    let cancelled = store.cancel(&task.id, 2).unwrap();
    assert_eq!(cancelled.state, TaskState::CancelRequested);
    assert_eq!(
        store
            .mark_dispatch_started(&intent.id, "process-instance")
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
    store
        .confirm_stop(&intent.id, StopEvidence::NeverDispatched)
        .unwrap();
    assert_eq!(store.get(&task.id).unwrap().state, TaskState::Cancelled);
    assert_eq!(store.budget(true, "p").unwrap().unwrap().active, 0);
}
#[test]
fn crash_does_not_replay_sent_or_release_unknown_budget() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = Store::open(&path).unwrap();
    let task = store.create("p", "goal", "k").unwrap();
    budgets(&mut store, &task.id);
    let intent = store
        .prepare_attempt(&task.id, 1, binding(), "dispatch-k")
        .unwrap();
    store
        .mark_dispatch_started(&intent.id, "process-start-nonce")
        .unwrap();
    drop(store);
    let mut store = Store::open(&path).unwrap();
    assert_eq!(
        store.intent(&intent.id).unwrap().state,
        IntentState::OutcomeUnknown
    );
    assert_eq!(
        store.get(&task.id).unwrap().state,
        TaskState::OutcomeUnknown
    );
    assert_eq!(
        store
            .mark_dispatch_started(&intent.id, "new-process")
            .unwrap_err()
            .code,
        "OUTCOME_UNKNOWN"
    );
    assert_eq!(store.budget(true, "p").unwrap().unwrap().active, 1);
    assert_eq!(
        store
            .confirm_stop(&intent.id, StopEvidence::NeverDispatched)
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
    assert_eq!(
        store
            .confirm_stop(
                &intent.id,
                StopEvidence::ConfirmedStopped {
                    executor_instance: "reused-pid".into(),
                    receipt_ref: "receipt".into()
                }
            )
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
    let task = store.get(&task.id).unwrap();
    store.cancel(&task.id, task.revision).unwrap();
    store
        .confirm_stop(
            &intent.id,
            StopEvidence::ConfirmedStopped {
                executor_instance: "process-start-nonce".into(),
                receipt_ref: "supervisor-record-1".into(),
            },
        )
        .unwrap();
    assert_eq!(store.get(&task.id).unwrap().state, TaskState::Cancelled);
    assert_eq!(store.budget(true, "p").unwrap().unwrap().active, 0);
    assert_eq!(store.budget(true, "p").unwrap().unwrap().consumed, 1);
    store
        .confirm_stop(&intent.id, StopEvidence::NeverDispatched)
        .unwrap();
    assert_eq!(store.budget(true, "p").unwrap().unwrap().active, 0);
}
#[test]
fn prepared_intent_survives_restart_without_claiming_sent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = Store::open(&path).unwrap();
    let task = store.create("p", "goal", "k").unwrap();
    budgets(&mut store, &task.id);
    let intent = store
        .prepare_attempt(&task.id, 1, binding(), "dispatch-k")
        .unwrap();
    drop(store);
    let store = Store::open(&path).unwrap();
    assert_eq!(
        store.intent(&intent.id).unwrap().state,
        IntentState::Prepared
    );
    assert_eq!(store.get(&task.id).unwrap().state, TaskState::Draft);
}
#[test]
fn root_missing_budget_rolls_back_project_reservation_and_journal() {
    let mut store = Store::open(Path::new(":memory:")).unwrap();
    let task = store.create("p", "goal", "k").unwrap();
    store.configure_budget(true, "p", 2, 1).unwrap();
    let sequence = store.sequence().unwrap();
    assert_eq!(
        store
            .prepare_attempt(&task.id, 1, binding(), "dispatch-k")
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
    let budget = store.budget(true, "p").unwrap().unwrap();
    assert_eq!((budget.active, budget.consumed), (0, 0));
    assert_eq!(store.sequence().unwrap(), sequence);
}
#[test]
fn project_parallel_and_total_caps_cannot_be_widened_or_refunded() {
    let mut store = Store::open(Path::new(":memory:")).unwrap();
    let first = store.create("p", "first", "1").unwrap();
    let second = store.create("p", "second", "2").unwrap();
    store.configure_budget(true, "p", 1, 1).unwrap();
    store.configure_budget(false, &first.id, 1, 1).unwrap();
    store.configure_budget(false, &second.id, 1, 1).unwrap();
    let intent = store
        .prepare_attempt(&first.id, 1, binding(), "i1")
        .unwrap();
    assert_eq!(
        store
            .prepare_attempt(&second.id, 1, binding(), "i2")
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
    store
        .confirm_stop(&intent.id, StopEvidence::NeverDispatched)
        .unwrap();
    assert_eq!(
        store
            .prepare_attempt(&second.id, 1, binding(), "i2")
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
    assert_eq!(
        store.configure_budget(true, "p", 10, 10).unwrap_err().code,
        "PERMISSION_DENIED"
    );
}
#[test]
fn event_failure_rolls_back_attempt_and_both_budget_reservations() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut store = Store::open(&path).unwrap();
    let task = store.create("p", "goal", "k").unwrap();
    budgets(&mut store, &task.id);
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TRIGGER reject_events BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'failure'); END;").unwrap();
    assert!(store
        .prepare_attempt(&task.id, 1, binding(), "intent")
        .is_err());
    for (project, id) in [(true, "p"), (false, task.id.as_str())] {
        let b = store.budget(project, id).unwrap().unwrap();
        assert_eq!((b.active, b.consumed), (0, 0));
    }
    assert_eq!(
        conn.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(store.get(&task.id).unwrap().revision, 1);
}
#[test]
fn v1_migration_preserves_records_and_produces_private_restorable_backup() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE tasks(id TEXT PRIMARY KEY,document TEXT NOT NULL);CREATE TABLE events(sequence INTEGER PRIMARY KEY AUTOINCREMENT,document TEXT NOT NULL);CREATE TABLE idempotency(scope TEXT,key TEXT,payload TEXT,result TEXT,PRIMARY KEY(scope,key)); PRAGMA user_version=1;").unwrap();
    let task = serde_json::json!({"id":"legacy","project_id":"p","goal":"keep me","state":"draft","revision":1,"created_at":"2026-10-27T00:00:00Z","updated_at":"2026-10-27T00:00:00Z"});
    conn.execute(
        "INSERT INTO tasks VALUES(?1,?2)",
        params!["legacy", task.to_string()],
    )
    .unwrap();
    drop(conn);
    let store = Store::open(&path).unwrap();
    assert_eq!(store.get("legacy").unwrap().goal, "keep me");
    let backups: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| {
            let e = e.unwrap();
            e.file_name()
                .to_string_lossy()
                .contains("pre-v3")
                .then_some(e.path())
        })
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(
        std::fs::metadata(&backups[0]).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let backup = Connection::open(&backups[0]).unwrap();
    assert_eq!(
        backup
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        backup
            .query_row("SELECT count(*) FROM tasks", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}
