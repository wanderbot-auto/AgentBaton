#[path = "fixtures/pre_factory_schema.rs"]
pub mod pre_factory_schema;
#[path = "fixtures/schema19_legacy_migration.rs"]
pub mod schema19_legacy_migration;
use agentbaton_core::storage::Store;
use rusqlite::{Connection, OpenFlags};
use schema19_legacy_migration::{
    assert_empty_host19, database_bytes, fixture_logical_snapshot, host19_backup,
    legacy_predecessor, remove_empty_host19,
};
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
    remove_empty_host19(&c).unwrap();
    pre_factory_schema::remove_empty_current_factory_increment(&c).unwrap();
    c.execute_batch("DROP TRIGGER graph_fence_lease_insert;DROP TRIGGER graph_fence_lease_update;DROP TABLE goal_graph_acceptances;DROP TABLE goal_graph_owner_decisions;DROP TABLE goal_graph_validations;DROP TABLE goal_graph_candidates;DROP TABLE goal_graph_jobs;DROP TABLE goal_graph_capabilities;DROP TABLE goal_run_reservations;DROP TABLE goal_run_nodes;DROP TABLE goal_runs;DROP TABLE data_workspace_receipts;DROP TABLE data_workspace_creations;DROP TABLE data_workspace_previews;DROP TABLE goal_node_evidence;DROP TABLE goal_external_action_journal;DROP TABLE goal_plan_proposals;DROP TABLE goal_plan_revisions;DROP TABLE goal_plan_heads;DROP TABLE goal_scopes;DROP TABLE goal_grant_revisions;DROP TABLE goal_ledgers;DROP TRIGGER command_fence_attempt;DROP TRIGGER command_fence_lease_insert;DROP TRIGGER command_fence_lease_update;DROP TABLE command_workspace_fences;DROP TABLE command_validation_runs;DROP TABLE command_validation_policies;DROP TABLE data_validation_runs;DROP TABLE data_validation_policies;DROP TABLE validation_runs;DROP TABLE validation_policies;DROP TABLE session_runtime_snapshots;DROP TABLE approval_proposals;DROP TABLE session_turns;DROP TABLE sessions;DROP TABLE execution_grants;DROP TABLE connections;DROP TABLE workspace_leases;DROP TABLE managed_workspaces;DROP TABLE projects;PRAGMA user_version=2;").unwrap();
    drop(c);
    let predecessor = legacy_predecessor(&db, 2);
    let upgraded = Store::open(&db).unwrap();
    assert_eq!(
        upgraded.get(&task.id).unwrap().goal,
        "preserve during upgrade"
    );
    assert!(upgraded.list_projects().unwrap().is_empty());
    drop(upgraded);
    let current = Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_empty_host19(&current);
    drop(current);
    let host_backup = host19_backup(&db, &predecessor);
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
    assert!(host19_backup(&db, &predecessor) == host_backup);
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
    remove_empty_host19(&c).unwrap();
    pre_factory_schema::remove_empty_current_factory_increment(&c).unwrap();
    c.execute_batch("DROP TRIGGER graph_fence_lease_insert;DROP TRIGGER graph_fence_lease_update;DROP TABLE goal_graph_acceptances;DROP TABLE goal_graph_owner_decisions;DROP TABLE goal_graph_validations;DROP TABLE goal_graph_candidates;DROP TABLE goal_graph_jobs;DROP TABLE goal_graph_capabilities;DROP TABLE goal_run_reservations;DROP TABLE goal_run_nodes;DROP TABLE goal_runs;DROP TABLE data_workspace_receipts;DROP TABLE data_workspace_creations;DROP TABLE data_workspace_previews;DROP TABLE goal_node_evidence;DROP TABLE goal_external_action_journal;DROP TABLE goal_plan_proposals;DROP TABLE goal_plan_revisions;DROP TABLE goal_plan_heads;DROP TABLE goal_scopes;DROP TABLE goal_grant_revisions;DROP TABLE goal_ledgers;DROP TRIGGER command_fence_attempt;DROP TRIGGER command_fence_lease_insert;DROP TRIGGER command_fence_lease_update;DROP TABLE command_workspace_fences;DROP TABLE command_validation_runs;DROP TABLE command_validation_policies;DROP TABLE data_validation_runs;DROP TABLE data_validation_policies;DROP TABLE validation_runs;DROP TABLE validation_policies;DROP TABLE session_runtime_snapshots;DROP TABLE approval_proposals;DROP TABLE session_turns;DROP TABLE sessions;DROP TABLE execution_grants;DROP TABLE connections;DROP TABLE workspace_leases;DROP TABLE managed_workspaces;DROP TABLE projects;CREATE TABLE projects(sentinel TEXT);PRAGMA user_version=2;").unwrap();
    drop(c);
    // Measure the closed fixture before the read-only snapshot opens WAL sidecars.
    let before = database_bytes(&db);
    let predecessor = legacy_predecessor(&db, 2);
    let error = Store::open(&db)
        .err()
        .expect("v2 migration must refuse the conflicting projects table");
    assert_eq!(error.code, "INTERNAL_ERROR");
    assert_eq!(error.side_effect_state, "unknown");
    assert!(database_bytes(&db) == before);
    let c = Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert!(fixture_logical_snapshot(&c) == predecessor);
    host19_backup(&db, &predecessor);
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

    // Change only the conflicting table in a separate SQLite backup.
    let control_dir = tempfile::tempdir().unwrap();
    let control = control_dir.path().join("db");
    c.backup(rusqlite::MAIN_DB, &control, None).unwrap();
    let copy = Connection::open(&control).unwrap();
    copy.execute_batch("DROP TABLE projects").unwrap();
    drop(copy);
    drop(Store::open(&control).unwrap());
    let copy = Connection::open_with_flags(&control, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_empty_host19(&copy);
}
