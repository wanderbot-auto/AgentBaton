#[path = "fixtures/pre_factory_schema.rs"]
mod pre_factory_schema;
use agentbaton_core::storage::Store;
use rusqlite::Connection;
use std::os::unix::fs::PermissionsExt;

#[test]
fn v4_runtime_upgrade_preserves_original_tasks_and_private_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db.sqlite3");
    let mut store = Store::open(&path).unwrap();
    let task = store
        .create("local", "Preserve v4 task", "original-key")
        .unwrap();
    drop(store);
    let db = Connection::open(&path).unwrap();
    pre_factory_schema::remove_empty_factory_increment(&db).unwrap();
    db.execute_batch("DROP TRIGGER graph_fence_lease_insert;DROP TRIGGER graph_fence_lease_update;DROP TABLE goal_graph_acceptances;DROP TABLE goal_graph_owner_decisions;DROP TABLE goal_graph_validations;DROP TABLE goal_graph_candidates;DROP TABLE goal_graph_jobs;DROP TABLE goal_graph_capabilities;DROP TABLE goal_run_reservations;DROP TABLE goal_run_nodes;DROP TABLE goal_runs;DROP TABLE data_workspace_receipts;DROP TABLE data_workspace_creations;DROP TABLE data_workspace_previews;DROP TABLE goal_node_evidence;DROP TABLE goal_external_action_journal;DROP TABLE goal_plan_proposals;DROP TABLE goal_plan_revisions;DROP TABLE goal_plan_heads;DROP TABLE goal_scopes;DROP TABLE goal_grant_revisions;DROP TABLE goal_ledgers;DROP TRIGGER command_fence_attempt;DROP TRIGGER command_fence_lease_insert;DROP TRIGGER command_fence_lease_update;DROP TABLE command_workspace_fences;DROP TABLE command_validation_runs;DROP TABLE command_validation_policies;DROP TABLE data_validation_runs;DROP TABLE data_validation_policies;DROP TABLE validation_runs;DROP TABLE validation_policies;DROP TABLE session_runtime_snapshots; PRAGMA user_version=4;")
        .unwrap();
    let original: String = db
        .query_row(
            "SELECT document FROM tasks WHERE id=?1",
            [&task.id],
            |row| row.get(0),
        )
        .unwrap();
    drop(db);
    let store = Store::open(&path).unwrap();
    assert_eq!(store.get(&task.id).unwrap().goal, "Preserve v4 task");
    drop(store);
    let db = Connection::open(&path).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        13
    );
    assert_eq!(
        db.query_row(
            "SELECT document FROM tasks WHERE id=?1",
            [&task.id],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        original
    );
    let backup = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("pre-v7")
        })
        .unwrap();
    assert_eq!(
        std::fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let backup = Connection::open(backup).unwrap();
    assert_eq!(
        backup
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        4
    );
    assert_eq!(
        backup
            .query_row(
                "SELECT document FROM tasks WHERE id=?1",
                [&task.id],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        original
    );
    assert_eq!(
        backup
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='session_runtime_snapshots'",
                [],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn failed_runtime_migration_preserves_v4_version_and_unknown_table() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db.sqlite3");
    drop(Store::open(&path).unwrap());
    let db = Connection::open(&path).unwrap();
    pre_factory_schema::remove_empty_factory_increment(&db).unwrap();
    db.execute_batch("DROP TRIGGER graph_fence_lease_insert;DROP TRIGGER graph_fence_lease_update;DROP TABLE goal_graph_acceptances;DROP TABLE goal_graph_owner_decisions;DROP TABLE goal_graph_validations;DROP TABLE goal_graph_candidates;DROP TABLE goal_graph_jobs;DROP TABLE goal_graph_capabilities;DROP TABLE goal_run_reservations;DROP TABLE goal_run_nodes;DROP TABLE goal_runs;DROP TABLE data_workspace_receipts;DROP TABLE data_workspace_creations;DROP TABLE data_workspace_previews;DROP TABLE goal_node_evidence;DROP TABLE goal_external_action_journal;DROP TABLE goal_plan_proposals;DROP TABLE goal_plan_revisions;DROP TABLE goal_plan_heads;DROP TABLE goal_scopes;DROP TABLE goal_grant_revisions;DROP TABLE goal_ledgers;DROP TRIGGER command_fence_attempt;DROP TRIGGER command_fence_lease_insert;DROP TRIGGER command_fence_lease_update;DROP TABLE command_workspace_fences;DROP TABLE command_validation_runs;DROP TABLE command_validation_policies;DROP TABLE data_validation_runs;DROP TABLE data_validation_policies;DROP TABLE validation_runs;DROP TABLE validation_policies;DROP TABLE session_runtime_snapshots; CREATE TABLE session_runtime_snapshots(sentinel TEXT); INSERT INTO session_runtime_snapshots VALUES('preserve unknown data'); PRAGMA user_version=4;").unwrap();
    drop(db);
    assert!(Store::open(&path).is_err());
    let db = Connection::open(&path).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        4
    );
    assert_eq!(
        db.query_row(
            "SELECT sentinel FROM session_runtime_snapshots",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "preserve unknown data"
    );
}
