//! Controlled generated SQLite12->13 increment followed by current Store19.
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
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::PathBuf,
};
fn prior() -> (tempfile::TempDir, PathBuf, String) {
    let d = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let p = d.path().join("schema12.sqlite3");
    drop(
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&p)
            .unwrap(),
    );
    let mut s = Store::open(&p).unwrap();
    let task = s
        .create("local", "preserve original bytes", "original")
        .unwrap();
    drop(s);
    let c = Connection::open(&p).unwrap();
    remove_empty_host19(&c).unwrap();
    pre_factory_schema::remove_empty_current_factory_increment(&c).unwrap();
    c.execute_batch("PRAGMA user_version=12;CREATE TABLE original_bytes(raw BLOB);INSERT INTO original_bytes VALUES(x'00ffdeadbeef');").unwrap();
    let raw: String = c
        .query_row("SELECT document FROM tasks WHERE id=?1", [task.id], |r| {
            r.get(0)
        })
        .unwrap();
    let raw =
        serde_json::to_string_pretty(&serde_json::from_str::<serde_json::Value>(&raw).unwrap())
            .unwrap()
            + "\n";
    c.execute("UPDATE tasks SET document=?1", [&raw]).unwrap();
    drop(c);
    (d, p, raw)
}
fn backups(d: &std::path::Path) -> Vec<PathBuf> {
    fs::read_dir(d)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("pre-v13-")
                && p.extension().is_some_and(|x| x == "sqlite3")
        })
        .collect()
}
#[test]
fn schema12_upgrade_preserves_exact_task_bytes_and_private_readable_backup_without_factory_authority(
) {
    let (d, p, raw) = prior();
    let predecessor = legacy_predecessor(&p, 12);
    let store = Store::open(&p).unwrap();
    assert_eq!(store.sequence().unwrap(), 1);
    drop(store);
    let c = Connection::open(&p).unwrap();
    assert_empty_host19(&c);
    let current = fixture_logical_snapshot(&c);
    assert!(
        predecessor
            .rows
            .iter()
            .all(|table| current.rows.contains(table)),
        "legacy migration changed predecessor typed rows"
    );
    let host_backup = host19_backup(&p, &predecessor);
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        19
    );
    assert_eq!(
        c.query_row("SELECT document FROM tasks", [], |r| r.get::<_, String>(0))
            .unwrap()
            .as_bytes(),
        raw.as_bytes()
    );
    assert_eq!(
        c.query_row("SELECT raw FROM original_bytes", [], |r| r
            .get::<_, Vec<u8>>(0))
            .unwrap(),
        [0, 255, 222, 173, 190, 239]
    );
    for table in [
        "factory_scopes",
        "factory_runs",
        "factory_task_lineage",
        "factory_task_bindings",
        "factory_delegations",
        "factory_attempts",
        "factory_scoped_handles",
        "factory_handoffs",
        "factory_budget_reservations",
    ] {
        assert_eq!(
            c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
    let b = backups(d.path());
    assert_eq!(b.len(), 1);
    assert_eq!(
        fs::metadata(&b[0]).unwrap().permissions().mode() & 0o7777,
        0o600
    );
    let old = Connection::open(&b[0]).unwrap();
    assert!(fixture_logical_snapshot(&old).rows == predecessor.rows);
    let legacy_backup_bytes = fs::read(&b[0]).unwrap();
    assert_eq!(
        old.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        12
    );
    assert_eq!(
        old.query_row("SELECT document FROM tasks", [], |r| r.get::<_, String>(0))
            .unwrap()
            .as_bytes(),
        raw.as_bytes()
    );
    drop(old);
    drop(c);
    drop(Store::open(&p).unwrap());
    assert!(host19_backup(&p, &predecessor) == host_backup);
    let retained = backups(d.path());
    assert_eq!(retained.len(), 1);
    assert!(fs::read(&retained[0]).unwrap() == legacy_backup_bytes);
}
#[test]
fn factory_migration_collision_rolls_back_all_new_tables_and_preserves_unknown_original_rows() {
    let (d, p, _) = prior();
    let keeper = Connection::open(&p).unwrap();
    keeper.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    keeper
        .execute_batch(
            "CREATE TABLE factory_runs(sentinel BLOB);INSERT INTO factory_runs VALUES(x'00ff');",
        )
        .unwrap();
    let predecessor = fixture_logical_snapshot(&keeper);
    let before = database_bytes_with_wal(&p);
    assert_eq!(Store::open(&p).err().unwrap().code, "INTERNAL_ERROR");
    let c = Connection::open(&p).unwrap();
    assert!(
        database_bytes_with_wal(&p) == before,
        "failed migration changed main or WAL bytes"
    );
    assert!(
        fixture_logical_snapshot(&c) == predecessor,
        "failed migration changed the logical database"
    );
    host19_backup(&p, &predecessor);
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        12
    );
    assert_eq!(
        c.query_row("SELECT sentinel FROM factory_runs", [], |r| r
            .get::<_, Vec<u8>>(0))
            .unwrap(),
        [0, 255]
    );
    assert_eq!(c.query_row("SELECT count(*) FROM sqlite_master WHERE name IN('factory_scopes','factory_task_lineage','factory_scoped_handles')",[],|r|r.get::<_,u32>(0)).unwrap(),0);
    assert_eq!(backups(d.path()).len(), 1);
}
#[test]
fn future_schema20_is_refused_without_mutation_or_factory_backup() {
    let (d, p, _) = prior();
    let keeper = Connection::open(&p).unwrap();
    keeper.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    // Use an inert fixture label in a committed WAL frame to reach the actual
    // current Store version guard.
    keeper.pragma_update(None, "user_version", 20).unwrap();
    let before = database_bytes_with_wal(&p);
    assert_future20_refused(&p);
    assert!(database_bytes_with_wal(&p) == before);
    assert!(backups(d.path()).is_empty());
}
