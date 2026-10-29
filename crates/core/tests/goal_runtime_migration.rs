use agentbaton_core::storage::Store;
use rusqlite::Connection;
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

fn schema10() -> (tempfile::TempDir, PathBuf, String) {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("schema10.sqlite3");
    let mut store = Store::open(&path).unwrap();
    let task = store
        .create("local", "Keep original history", "original")
        .unwrap();
    drop(store);
    let db = Connection::open(&path).unwrap();
    let raw: String = db
        .query_row("SELECT document FROM tasks WHERE id=?1", [&task.id], |r| {
            r.get(0)
        })
        .unwrap();
    db.execute_batch("DROP TRIGGER graph_fence_lease_insert;DROP TRIGGER graph_fence_lease_update;DROP TABLE goal_graph_acceptances;DROP TABLE goal_graph_owner_decisions;DROP TABLE goal_graph_validations;DROP TABLE goal_graph_candidates;DROP TABLE goal_graph_jobs;DROP TABLE goal_graph_capabilities;DROP TABLE goal_run_reservations;DROP TABLE goal_run_nodes;DROP TABLE goal_runs;CREATE TABLE original_blob(raw BLOB);INSERT INTO original_blob VALUES(x'00ffdeadbeef');PRAGMA user_version=10;").unwrap();
    (d, path, raw)
}
fn backups(d: &std::path::Path) -> Vec<PathBuf> {
    fs::read_dir(d)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("pre-v11-")
                && p.extension().is_some_and(|x| x == "sqlite3")
        })
        .collect()
}
#[test]
fn schema10_upgrade_preserves_original_rows_in_private_logical_backup_without_seeding_execution() {
    let (d, path, raw) = schema10();
    drop(Store::open(&path).unwrap());
    let db = Connection::open(&path).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        13
    );
    assert_eq!(
        db.query_row("SELECT document FROM tasks", [], |r| r.get::<_, String>(0))
            .unwrap(),
        raw
    );
    for table in [
        "goal_runs",
        "goal_run_nodes",
        "goal_run_reservations",
        "budgets",
    ] {
        assert_eq!(
            db.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
    let images = backups(d.path());
    assert_eq!(images.len(), 1);
    assert_eq!(
        fs::metadata(&images[0]).unwrap().permissions().mode() & 0o7777,
        0o600
    );
    let old = Connection::open(&images[0]).unwrap();
    assert_eq!(
        old.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        10
    );
    assert_eq!(
        old.query_row("SELECT document FROM tasks", [], |r| r.get::<_, String>(0))
            .unwrap(),
        raw
    );
    assert_eq!(
        old.query_row("SELECT raw FROM original_blob", [], |r| r
            .get::<_, Vec<u8>>(0))
            .unwrap(),
        [0, 255, 222, 173, 190, 239]
    );
    assert_eq!(
        old.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name LIKE 'goal_run%'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    drop(Store::open(&path).unwrap());
    assert_eq!(backups(d.path()).len(), 1);
}
#[test]
fn schema11_collision_rolls_back_all_runtime_tables_without_deleting_unknown_rows() {
    let (d, path, raw) = schema10();
    let db = Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE goal_run_nodes(sentinel BLOB);INSERT INTO goal_run_nodes VALUES(x'00ffaa');",
    )
    .unwrap();
    drop(db);
    assert_eq!(Store::open(&path).err().unwrap().code, "INTERNAL_ERROR");
    let db = Connection::open(&path).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        10
    );
    assert_eq!(
        db.query_row("SELECT sentinel FROM goal_run_nodes", [], |r| r
            .get::<_, Vec<u8>>(0))
            .unwrap(),
        [0, 255, 170]
    );
    assert_eq!(
        db.query_row("SELECT document FROM tasks", [], |r| r.get::<_, String>(0))
            .unwrap(),
        raw
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name IN('goal_runs','goal_run_reservations')",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(backups(d.path()).len(), 1);
}
#[test]
fn future_schema13_is_refused_without_modification_or_migration_backup() {
    let (d, path, _) = schema10();
    Connection::open(&path)
        .unwrap()
        .execute_batch("PRAGMA user_version=13")
        .unwrap();
    let before = fs::read(&path).unwrap();
    assert_eq!(
        Store::open(&path).err().unwrap().code,
        "VERSION_UNSUPPORTED"
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(backups(d.path()).is_empty());
}
