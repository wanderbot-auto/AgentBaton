use agentbaton_core::storage::Store;
use rusqlite::Connection;
use std::{
    fs,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
fn version9() -> (tempfile::TempDir, PathBuf, Vec<u8>) {
    let d = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let path = d.path().join("schema9.sqlite3");
    drop(
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap(),
    );
    let mut s = Store::open(&path).unwrap();
    s.create("local", "preserve exact original bytes", "original")
        .unwrap();
    drop(s);
    let db = Connection::open(&path).unwrap();
    db.execute_batch("DROP TRIGGER graph_fence_lease_insert;DROP TRIGGER graph_fence_lease_update;DROP TABLE goal_graph_acceptances;DROP TABLE goal_graph_owner_decisions;DROP TABLE goal_graph_validations;DROP TABLE goal_graph_candidates;DROP TABLE goal_graph_jobs;DROP TABLE goal_graph_capabilities;DROP TABLE goal_run_reservations;DROP TABLE goal_run_nodes;DROP TABLE goal_runs;DROP TABLE data_workspace_receipts;DROP TABLE data_workspace_creations;DROP TABLE data_workspace_previews;CREATE TABLE original_blob(raw BLOB);INSERT INTO original_blob VALUES(x'00ffdeadbeef');PRAGMA user_version=9;").unwrap();
    drop(db);
    let before = fs::read(&path).unwrap();
    (d, path, before)
}
fn backups(path: &Path) -> Vec<PathBuf> {
    fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("pre-v10-")
        })
        .collect()
}
#[test]
fn actual_schema9_upgrade_has_private_exact_old_bytes_backup_and_does_not_seed_model_budget() {
    let (d, path, before) = version9();
    drop(Store::open(&path).unwrap());
    let db = Connection::open(&path).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        13
    );
    assert_eq!(
        db.query_row("SELECT raw FROM original_blob", [], |r| r
            .get::<_, Vec<u8>>(0))
            .unwrap(),
        [0, 255, 222, 173, 190, 239]
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM budgets", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        0
    );
    let backup = backups(d.path());
    assert_eq!(backup.len(), 1);
    assert_eq!(
        fs::metadata(&backup[0]).unwrap().permissions().mode() & 0o7777,
        0o600
    );
    let old = Connection::open(&backup[0]).unwrap();
    assert_eq!(
        old.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        9
    );
    // SQLite backup is a logical image; compare all original raw values, not the
    // pager header/change counter which VACUUM INTO is allowed to regenerate.
    assert_eq!(
        old.query_row("SELECT raw FROM original_blob", [], |r| r
            .get::<_, Vec<u8>>(0))
            .unwrap(),
        [0, 255, 222, 173, 190, 239]
    );
    let before_path = d.path().join("exact-original.sqlite3");
    fs::write(&before_path, &before).unwrap();
    let prior = Connection::open(before_path).unwrap();
    assert_eq!(
        old.query_row("SELECT document FROM tasks", [], |r| r.get::<_, String>(0))
            .unwrap()
            .as_bytes(),
        prior
            .query_row("SELECT document FROM tasks", [], |r| r.get::<_, String>(0))
            .unwrap()
            .as_bytes()
    );
    drop(db);
    drop(old);
    drop(prior);
    drop(Store::open(&path).unwrap());
    assert_eq!(backups(d.path()).len(), 1);
}
#[test]
fn schema10_migration_collision_rolls_back_every_new_table_and_preserves_unknown_rows() {
    let (d, path, _) = version9();
    let db = Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE data_workspace_creations(sentinel BLOB);INSERT INTO data_workspace_creations VALUES(x'00ffaa');").unwrap();
    drop(db);
    assert_eq!(Store::open(&path).err().unwrap().code, "INTERNAL_ERROR");
    let db = Connection::open(&path).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        9
    );
    assert_eq!(
        db.query_row("SELECT sentinel FROM data_workspace_creations", [], |r| r
            .get::<_, Vec<
            u8,
        >>(
            0
        ))
        .unwrap(),
        [0, 255, 170]
    );
    assert_eq!(db.query_row("SELECT count(*) FROM sqlite_master WHERE name IN('data_workspace_previews','data_workspace_receipts')",[],|r|r.get::<_,u32>(0)).unwrap(),0);
    assert_eq!(backups(d.path()).len(), 1);
}
#[test]
fn future_schema14_is_refused_without_touching_bytes_or_creating_migration_backup() {
    let (d, path, _) = version9();
    let db = Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA user_version=14;").unwrap();
    drop(db);
    let before = fs::read(&path).unwrap();
    assert_eq!(
        Store::open(&path).err().unwrap().code,
        "VERSION_UNSUPPORTED"
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(backups(d.path()).is_empty());
}
