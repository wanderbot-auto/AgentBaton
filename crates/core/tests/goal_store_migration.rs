use agentbaton_core::storage::Store;
use rusqlite::{params, Connection};
use std::{
    fs,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
const DROP_GOAL:&str="DROP TABLE goal_run_reservations;DROP TABLE goal_run_nodes;DROP TABLE goal_runs;DROP TABLE data_workspace_receipts;DROP TABLE data_workspace_creations;DROP TABLE data_workspace_previews;DROP TABLE goal_node_evidence;DROP TABLE goal_external_action_journal;DROP TABLE goal_plan_proposals;DROP TABLE goal_plan_revisions;DROP TABLE goal_plan_heads;DROP TABLE goal_scopes;DROP TABLE goal_grant_revisions;DROP TABLE goal_ledgers;";
fn schema8() -> (tempfile::TempDir, PathBuf, String) {
    let dir = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let db = dir.path().join("state.sqlite3");
    drop(
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&db)
            .unwrap(),
    );
    let mut store = Store::open(&db).unwrap();
    let t = store
        .create("local", "Original schema8 record", "original")
        .unwrap();
    drop(store);
    let c = Connection::open(&db).unwrap();
    c.execute_batch(DROP_GOAL).unwrap();
    c.execute_batch("PRAGMA user_version=8;CREATE TABLE original_blob(bytes BLOB);INSERT INTO original_blob VALUES(x'00ffdeadbeef');").unwrap();
    let raw: String = c
        .query_row("SELECT document FROM tasks WHERE id=?1", [&t.id], |r| {
            r.get(0)
        })
        .unwrap();
    let raw =
        serde_json::to_string_pretty(&serde_json::from_str::<serde_json::Value>(&raw).unwrap())
            .unwrap()
            + "\n";
    c.execute(
        "UPDATE tasks SET document=?1 WHERE id=?2",
        params![raw, t.id],
    )
    .unwrap();
    drop(c);
    (dir, db, raw)
}
fn backups(path: &Path) -> Vec<PathBuf> {
    fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().contains("pre-v9-"))
        .collect()
}
#[test]
fn actual_schema8_upgrade_preserves_raw_rows_and_private_reopenable_backup() {
    let (dir, db, raw) = schema8();
    drop(Store::open(&db).unwrap());
    let c = Connection::open(&db).unwrap();
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        11
    );
    assert_eq!(
        c.query_row("SELECT document FROM tasks", [], |r| r.get::<_, String>(0))
            .unwrap()
            .as_bytes(),
        raw.as_bytes()
    );
    assert_eq!(
        c.query_row("SELECT bytes FROM original_blob", [], |r| r
            .get::<_, Vec<u8>>(0))
            .unwrap(),
        [0, 255, 222, 173, 190, 239]
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM command_workspace_fences", [], |r| r
            .get::<_, u32>(
            0
        ))
        .unwrap(),
        0
    );
    drop(c);
    let b = backups(dir.path());
    assert_eq!(b.len(), 1);
    assert_eq!(
        fs::metadata(&b[0]).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let backup = Connection::open(&b[0]).unwrap();
    assert_eq!(
        backup
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        8
    );
    assert_eq!(
        backup
            .query_row("SELECT document FROM tasks", [], |r| r.get::<_, String>(0))
            .unwrap()
            .as_bytes(),
        raw.as_bytes()
    );
    assert_eq!(
        backup
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='goal_scopes'",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
        0
    );
    drop(backup);
    drop(Store::open(&db).unwrap());
    assert_eq!(backups(dir.path()).len(), 1);
}
#[test]
fn migration9_failure_rolls_back_schema_and_preserves_unknown_goal_named_table() {
    let (dir, db, raw) = schema8();
    let c = Connection::open(&db).unwrap();
    c.execute_batch("CREATE TABLE goal_scopes(sentinel TEXT);INSERT INTO goal_scopes VALUES('preserve unknown owner data');").unwrap();
    drop(c);
    assert!(Store::open(&db).is_err());
    let c = Connection::open(&db).unwrap();
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        8
    );
    assert_eq!(
        c.query_row("SELECT sentinel FROM goal_scopes", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "preserve unknown owner data"
    );
    assert_eq!(c.query_row("SELECT count(*) FROM sqlite_master WHERE name IN('goal_grant_revisions','goal_ledgers','goal_plan_heads')",[],|r|r.get::<_,u32>(0)).unwrap(),0);
    assert_eq!(
        c.query_row("SELECT document FROM tasks", [], |r| r.get::<_, String>(0))
            .unwrap(),
        raw
    );
    assert_eq!(backups(dir.path()).len(), 1);
}
#[test]
fn actual_future_schema12_is_rejected_without_modifying_bytes() {
    let (_dir, db, _) = schema8();
    let c = Connection::open(&db).unwrap();
    c.execute_batch("PRAGMA user_version=12;").unwrap();
    drop(c);
    let before = fs::read(&db).unwrap();
    assert_eq!(Store::open(&db).err().unwrap().code, "VERSION_UNSUPPORTED");
    assert_eq!(fs::read(&db).unwrap(), before);
}
