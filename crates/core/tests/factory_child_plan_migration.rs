//! Real schema13->14 metadata migration, exact backup, rollback and future refusal.
use agentbaton_core::storage::Store;
use rusqlite::Connection;
use std::{
    fs,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
fn old() -> (tempfile::TempDir, PathBuf, String) {
    let d = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let p = d.path().join("prior13.sqlite3");
    drop(
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&p)
            .unwrap(),
    );
    let mut s = Store::open(&p).unwrap();
    let task = s.create("local", "preserve original task", "old").unwrap();
    drop(s);
    let db = Connection::open(&p).unwrap();
    db.execute_batch("DROP TABLE factory_child_plan_heads;DROP TABLE factory_child_plan_revisions;PRAGMA user_version=13;CREATE TABLE original_bytes(raw BLOB);INSERT INTO original_bytes VALUES(x'00ffdeadbeef');").unwrap();
    let raw: String = db
        .query_row("SELECT document FROM tasks WHERE id=?1", [task.id], |r| {
            r.get(0)
        })
        .unwrap();
    let raw =
        serde_json::to_string_pretty(&serde_json::from_str::<serde_json::Value>(&raw).unwrap())
            .unwrap()
            + "\n";
    db.execute("UPDATE tasks SET document=?1", [&raw]).unwrap();
    drop(db);
    (d, p, raw)
}
fn backups(p: &Path) -> Vec<PathBuf> {
    fs::read_dir(p)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("pre-v14-")
                && p.extension().is_some_and(|v| v == "sqlite3")
        })
        .collect()
}
#[test]
fn schema13_upgrade_keeps_raw_history_and_private_backup_and_adds_no_authority() {
    let (d, p, raw) = old();
    drop(Store::open(&p).unwrap());
    let db = Connection::open(&p).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        14
    );
    assert_eq!(
        db.query_row("SELECT document FROM tasks", [], |r| r.get::<_, String>(0))
            .unwrap(),
        raw
    );
    assert_eq!(
        db.query_row("SELECT raw FROM original_bytes", [], |r| r
            .get::<_, Vec<u8>>(0))
            .unwrap(),
        [0, 255, 222, 173, 190, 239]
    );
    for table in [
        "factory_child_plan_heads",
        "factory_child_plan_revisions",
        "factory_scopes",
    ] {
        assert_eq!(
            db.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        )
    }
    drop(db);
    let b = backups(d.path());
    assert_eq!(b.len(), 1);
    assert_eq!(
        fs::metadata(&b[0]).unwrap().permissions().mode() & 0o7777,
        0o600
    );
    let backup = Connection::open(&b[0]).unwrap();
    assert_eq!(
        backup
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        13
    );
    assert_eq!(
        backup
            .query_row("SELECT document FROM tasks", [], |r| r.get::<_, String>(0))
            .unwrap(),
        raw
    );
    drop(backup);
    drop(Store::open(&p).unwrap());
    assert_eq!(backups(d.path()).len(), 1);
}
#[test]
fn child_plan_migration_collision_rolls_back_both_tables_and_preserves_old_bytes() {
    let (d, p, raw) = old();
    let db = Connection::open(&p).unwrap();
    db.execute_batch("CREATE TABLE factory_child_plan_heads(sentinel BLOB);INSERT INTO factory_child_plan_heads VALUES(x'00ff');").unwrap();
    drop(db);
    assert_eq!(Store::open(&p).err().unwrap().code, "INTERNAL_ERROR");
    let db = Connection::open(&p).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        13
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='factory_child_plan_revisions'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT sentinel FROM factory_child_plan_heads", [], |r| r
            .get::<_, Vec<
            u8,
        >>(
            0
        ))
        .unwrap(),
        [0, 255]
    );
    assert_eq!(
        db.query_row("SELECT document FROM tasks", [], |r| r.get::<_, String>(0))
            .unwrap(),
        raw
    );
    assert_eq!(backups(d.path()).len(), 1);
}
#[test]
fn future_schema15_refuses_without_mutation_or_backup() {
    let (d, p, _) = old();
    let db = Connection::open(&p).unwrap();
    db.execute_batch("PRAGMA user_version=15;").unwrap();
    drop(db);
    let before = fs::read(&p).unwrap();
    assert_eq!(Store::open(&p).err().unwrap().code, "VERSION_UNSUPPORTED");
    assert_eq!(before, fs::read(&p).unwrap());
    assert!(backups(d.path()).is_empty());
}
