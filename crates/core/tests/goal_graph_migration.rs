#[path = "fixtures/pre_factory_schema.rs"]
pub mod pre_factory_schema;
#[path = "fixtures/schema19_legacy_migration.rs"]
pub mod schema19_legacy_migration;
use agentbaton_core::storage::Store;
use rusqlite::Connection;
use schema19_legacy_migration::{
    assert_empty_host19, assert_future20_refused, database_bytes_with_wal,
    fixture_logical_snapshot, host19_backup, legacy_predecessor, remove_empty_host19,
};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};
const DROP_GRAPH:&str="DROP TRIGGER graph_fence_lease_insert; DROP TRIGGER graph_fence_lease_update; DROP TABLE goal_graph_acceptances;DROP TABLE goal_graph_owner_decisions;DROP TABLE goal_graph_validations;DROP TABLE goal_graph_candidates;DROP TABLE goal_graph_jobs;DROP TABLE goal_graph_capabilities;PRAGMA user_version=11;";
fn old() -> (tempfile::TempDir, PathBuf, String) {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("schema11.sqlite3");
    let mut s = Store::open(&path).unwrap();
    let task = s
        .create(
            "local",
            "Preserve prior task and exact original bytes",
            "original",
        )
        .unwrap();
    drop(s);
    let db = Connection::open(&path).unwrap();
    let raw: String = db
        .query_row("SELECT document FROM tasks WHERE id=?1", [task.id], |r| {
            r.get(0)
        })
        .unwrap();
    remove_empty_host19(&db).unwrap();
    pre_factory_schema::remove_empty_current_factory_increment(&db).unwrap();
    db.execute_batch(DROP_GRAPH).unwrap();
    db.execute_batch(
        "CREATE TABLE original_blob(raw BLOB);INSERT INTO original_blob VALUES(x'00ffdeadbeef');",
    )
    .unwrap();
    drop(db);
    (d, path, raw)
}
fn backups(d: &Path) -> Vec<PathBuf> {
    fs::read_dir(d)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("pre-v12-")
                && p.extension().is_some_and(|x| x == "sqlite3")
        })
        .collect()
}
#[test]
fn actual_schema11_upgrade_keeps_raw_old_rows_private_backup_and_no_graph_authority() {
    let (d, path, raw) = old();
    let predecessor = legacy_predecessor(&path, 11);
    let store = Store::open(&path).unwrap();
    assert_eq!(store.sequence().unwrap(), 1);
    drop(store);
    let db = Connection::open(&path).unwrap();
    assert_empty_host19(&db);
    let current = fixture_logical_snapshot(&db);
    assert!(
        predecessor
            .rows
            .iter()
            .all(|table| current.rows.contains(table)),
        "legacy migration changed predecessor typed rows"
    );
    let host_backup = host19_backup(&path, &predecessor);
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        19
    );
    assert_eq!(
        db.query_row("SELECT document FROM tasks", [], |r| r.get::<_, String>(0))
            .unwrap(),
        raw
    );
    for table in [
        "goal_graph_capabilities",
        "goal_graph_jobs",
        "goal_graph_candidates",
        "goal_graph_validations",
        "goal_graph_owner_decisions",
        "goal_graph_acceptances",
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
    assert!(fixture_logical_snapshot(&old).rows == predecessor.rows);
    let legacy_backup_bytes = fs::read(&images[0]).unwrap();
    assert_eq!(
        old.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        11
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
            "SELECT count(*) FROM sqlite_master WHERE name LIKE 'goal_graph_%'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    drop(Store::open(&path).unwrap());
    assert!(host19_backup(&path, &predecessor) == host_backup);
    let retained = backups(d.path());
    assert_eq!(retained.len(), 1);
    assert!(fs::read(&retained[0]).unwrap() == legacy_backup_bytes);
}
#[test]
fn graph_schema12_collision_rolls_back_all_new_tables_and_retains_unknown_data() {
    let (d, path, raw) = old();
    let keeper = Connection::open(&path).unwrap();
    keeper.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    keeper.execute_batch("CREATE TABLE goal_graph_jobs(sentinel BLOB);INSERT INTO goal_graph_jobs VALUES(x'00ffaa');").unwrap();
    let predecessor = fixture_logical_snapshot(&keeper);
    let before = database_bytes_with_wal(&path);
    assert_eq!(Store::open(&path).err().unwrap().code, "INTERNAL_ERROR");
    let db = Connection::open(&path).unwrap();
    assert!(
        database_bytes_with_wal(&path) == before,
        "failed migration changed main or WAL bytes"
    );
    assert!(
        fixture_logical_snapshot(&db) == predecessor,
        "failed migration changed the logical database"
    );
    host19_backup(&path, &predecessor);
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        11
    );
    assert_eq!(
        db.query_row("SELECT sentinel FROM goal_graph_jobs", [], |r| r
            .get::<_, Vec<u8>>(0))
            .unwrap(),
        [0, 255, 170]
    );
    assert_eq!(
        db.query_row("SELECT document FROM tasks", [], |r| r.get::<_, String>(0))
            .unwrap(),
        raw
    );
    assert_eq!(db.query_row("SELECT count(*) FROM sqlite_master WHERE name IN('goal_graph_capabilities','goal_graph_candidates','goal_graph_validations','goal_graph_owner_decisions','goal_graph_acceptances')",[],|r|r.get::<_,u32>(0)).unwrap(),0);
    assert_eq!(backups(d.path()).len(), 1);
}
#[test]
fn schema20_is_refused_without_modifying_any_bytes_or_creating_backup() {
    let (d, path, _) = old();
    let keeper = Connection::open(&path).unwrap();
    keeper.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    // Use an inert fixture label in a committed WAL frame to reach the actual
    // current Store version guard.
    keeper.pragma_update(None, "user_version", 20).unwrap();
    let before = database_bytes_with_wal(&path);
    assert_future20_refused(&path);
    assert!(database_bytes_with_wal(&path) == before);
    assert!(backups(d.path()).is_empty());
}
