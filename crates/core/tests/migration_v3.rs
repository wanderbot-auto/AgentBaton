use agentbaton_core::storage::Store;
use rusqlite::Connection;
use std::os::unix::fs::PermissionsExt;
#[test]
fn v2_upgrade_keeps_tasks_intents_and_private_backup_reopens() {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("db");
    let mut store = Store::open(&db).unwrap();
    let task = store
        .create("local", "preserve during upgrade", "key")
        .unwrap();
    drop(store);
    let c = Connection::open(&db).unwrap();
    c.execute_batch("DROP TRIGGER command_fence_attempt;DROP TRIGGER command_fence_lease_insert;DROP TRIGGER command_fence_lease_update;DROP TABLE command_workspace_fences;DROP TABLE command_validation_runs;DROP TABLE command_validation_policies;DROP TABLE data_validation_runs;DROP TABLE data_validation_policies;DROP TABLE validation_runs;DROP TABLE validation_policies;DROP TABLE session_runtime_snapshots;DROP TABLE approval_proposals;DROP TABLE session_turns;DROP TABLE sessions;DROP TABLE execution_grants;DROP TABLE connections;DROP TABLE workspace_leases;DROP TABLE managed_workspaces;DROP TABLE projects;PRAGMA user_version=2;").unwrap();
    drop(c);
    let upgraded = Store::open(&db).unwrap();
    assert_eq!(
        upgraded.get(&task.id).unwrap().goal,
        "preserve during upgrade"
    );
    assert!(upgraded.list_projects().unwrap().is_empty());
    drop(upgraded);
    let backups: Vec<_> = std::fs::read_dir(d.path())
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("pre-v7")
                .then_some(p)
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
        2
    );
    assert_eq!(
        backup
            .query_row("SELECT count(*) FROM tasks", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    drop(backup);
    Store::open(&db).unwrap();
    assert_eq!(
        std::fs::read_dir(d.path())
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("pre-v7"))
            .count(),
        1
    );
}
#[test]
fn failed_v3_migration_leaves_version_and_legacy_state_recoverable() {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("db");
    let mut store = Store::open(&db).unwrap();
    let task = store.create("local", "must remain", "key").unwrap();
    drop(store);
    let c = Connection::open(&db).unwrap();
    c.execute_batch("DROP TRIGGER command_fence_attempt;DROP TRIGGER command_fence_lease_insert;DROP TRIGGER command_fence_lease_update;DROP TABLE command_workspace_fences;DROP TABLE command_validation_runs;DROP TABLE command_validation_policies;DROP TABLE data_validation_runs;DROP TABLE data_validation_policies;DROP TABLE validation_runs;DROP TABLE validation_policies;DROP TABLE session_runtime_snapshots;DROP TABLE approval_proposals;DROP TABLE session_turns;DROP TABLE sessions;DROP TABLE execution_grants;DROP TABLE connections;DROP TABLE workspace_leases;DROP TABLE managed_workspaces;DROP TABLE projects;CREATE TABLE projects(sentinel TEXT);PRAGMA user_version=2;").unwrap();
    drop(c);
    assert!(Store::open(&db).is_err());
    let c = Connection::open(&db).unwrap();
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        2
    );
    let json: String = c
        .query_row("SELECT document FROM tasks WHERE id=?1", [task.id], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(json.contains("must remain"));
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='managed_workspaces'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}
