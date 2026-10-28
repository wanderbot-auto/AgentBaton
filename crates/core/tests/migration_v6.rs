use agentbaton_core::storage::Store;
use rusqlite::Connection;
use std::{fs, os::unix::fs::PermissionsExt};

#[test]
fn v5_validation_upgrade_preserves_task_bytes_and_private_reopenable_backup() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("state.sqlite3");
    let mut store = Store::open(&database).unwrap();
    let task = store
        .create("local", "Keep original execution state", "original")
        .unwrap();
    drop(store);
    let connection = Connection::open(&database).unwrap();
    connection
        .execute_batch(
            "DROP TABLE goal_run_reservations;DROP TABLE goal_run_nodes;DROP TABLE goal_runs;DROP TABLE data_workspace_receipts;DROP TABLE data_workspace_creations;DROP TABLE data_workspace_previews;DROP TABLE goal_node_evidence;DROP TABLE goal_external_action_journal;DROP TABLE goal_plan_proposals;DROP TABLE goal_plan_revisions;DROP TABLE goal_plan_heads;DROP TABLE goal_scopes;DROP TABLE goal_grant_revisions;DROP TABLE goal_ledgers;DROP TRIGGER command_fence_attempt;DROP TRIGGER command_fence_lease_insert;DROP TRIGGER command_fence_lease_update;DROP TABLE command_workspace_fences;DROP TABLE command_validation_runs;DROP TABLE command_validation_policies;DROP TABLE data_validation_runs;DROP TABLE data_validation_policies;DROP TABLE validation_runs; DROP TABLE validation_policies; PRAGMA user_version=5;",
        )
        .unwrap();
    // A genuine v5 task document predates the distinct specification revision.
    connection
        .execute(
            "UPDATE tasks SET document=json_remove(document,'$.spec_revision') WHERE id=?1",
            [&task.id],
        )
        .unwrap();
    let original: String = connection
        .query_row(
            "SELECT document FROM tasks WHERE id=?1",
            [&task.id],
            |row| row.get(0),
        )
        .unwrap();
    drop(connection);
    let upgraded = Store::open(&database).unwrap();
    assert_eq!(upgraded.get(&task.id).unwrap().spec_revision, 1);
    assert_eq!(upgraded.get(&task.id).unwrap().revision, task.revision);
    drop(upgraded);
    assert!(serde_json::from_str::<serde_json::Value>(&original)
        .unwrap()
        .get("spec_revision")
        .is_none());
    let connection = Connection::open(&database).unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        11
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT document FROM tasks WHERE id=?1",
                [&task.id],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        original
    );
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM validation_policies", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    let backups: Vec<_> = fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("pre-v7")
                && path
                    .extension()
                    .is_some_and(|extension| extension == "sqlite3")
        })
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(
        fs::metadata(&backups[0]).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let backup = Connection::open(&backups[0]).unwrap();
    assert_eq!(
        backup
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        5
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
                "SELECT count(*) FROM sqlite_master WHERE name='validation_policies'",
                [],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
        0
    );
    drop(Store::open(&database).unwrap());
    assert_eq!(
        fs::read_dir(directory.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(
                |entry| entry.file_name().to_string_lossy().contains("pre-v7")
                    && entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "sqlite3")
            )
            .count(),
        1
    );
}

#[test]
fn failed_validation_upgrade_preserves_old_schema_and_unknown_records() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("state.sqlite3");
    drop(Store::open(&database).unwrap());
    let connection = Connection::open(&database).unwrap();
    connection.execute_batch("DROP TABLE goal_run_reservations;DROP TABLE goal_run_nodes;DROP TABLE goal_runs;DROP TABLE data_workspace_receipts;DROP TABLE data_workspace_creations;DROP TABLE data_workspace_previews;DROP TABLE goal_node_evidence;DROP TABLE goal_external_action_journal;DROP TABLE goal_plan_proposals;DROP TABLE goal_plan_revisions;DROP TABLE goal_plan_heads;DROP TABLE goal_scopes;DROP TABLE goal_grant_revisions;DROP TABLE goal_ledgers;DROP TRIGGER command_fence_attempt;DROP TRIGGER command_fence_lease_insert;DROP TRIGGER command_fence_lease_update;DROP TABLE command_workspace_fences;DROP TABLE command_validation_runs;DROP TABLE command_validation_policies;DROP TABLE data_validation_runs;DROP TABLE data_validation_policies;DROP TABLE validation_runs; DROP TABLE validation_policies; CREATE TABLE validation_policies(sentinel TEXT); INSERT INTO validation_policies VALUES('do not delete this record'); PRAGMA user_version=5;").unwrap();
    drop(connection);
    assert!(Store::open(&database).is_err());
    let connection = Connection::open(&database).unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        5
    );
    assert_eq!(
        connection
            .query_row("SELECT sentinel FROM validation_policies", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
        "do not delete this record"
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='validation_runs'",
                [],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
        0
    );
}
