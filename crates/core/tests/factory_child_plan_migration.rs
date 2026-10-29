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
    // Recorded original Grant/shared-meter rows contain no child-planning
    // capability. Preserve historical usage, including active reservations;
    // this migration fixture does not claim to recover their original workers.
    let prior: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/factory_actual_legacy_ledger_rows.json"
    ))
    .unwrap();
    for p in prior["tables"]["projects"].as_array().unwrap() {
        db.execute(
            "INSERT INTO projects(id,document,device,inode,root) VALUES(?1,?2,?3,?4,?5)",
            rusqlite::params![
                p["id"].as_str(),
                p["document"].as_str(),
                p["device"].as_str(),
                p["inode"].as_str(),
                p["root"].as_str()
            ],
        )
        .unwrap();
    }
    for g in prior["tables"]["execution_grants"].as_array().unwrap() {
        let _: agentbaton_core::policy::ExecutionGrant =
            serde_json::from_str(g["document"].as_str().unwrap()).unwrap();
        db.execute(
            "INSERT INTO execution_grants(id,project_id,parent_id,document) VALUES(?1,?2,?3,?4)",
            rusqlite::params![
                g["id"].as_str(),
                g["project_id"].as_str(),
                g["parent_id"].as_str(),
                g["document"].as_str()
            ],
        )
        .unwrap();
    }
    for b in prior["tables"]["budgets"].as_array().unwrap() {
        db.execute("INSERT INTO budgets(scope,total_limit,parallel_limit,consumed,active) VALUES(?1,?2,?3,?4,?5)",rusqlite::params![b["scope"].as_str(),b["total_limit"].as_i64(),b["parallel_limit"].as_i64(),b["consumed"].as_i64(),b["active"].as_i64()]).unwrap();
    }
    for l in prior["tables"]["goal_ledgers"].as_array().unwrap() {
        db.execute(
            "INSERT INTO goal_ledgers(scope,document) VALUES(?1,?2)",
            rusqlite::params![l["scope"].as_str(), l["document"].as_str()],
        )
        .unwrap();
    }
    drop(db);
    (d, p, raw)
}
fn original_metadata(db: &Connection) -> Vec<String> {
    [
        "SELECT json_group_array(json_array(id,document,device,inode,root)) FROM (SELECT * FROM projects ORDER BY id)",
        "SELECT json_group_array(json_array(id,project_id,parent_id,document)) FROM (SELECT * FROM execution_grants ORDER BY id)",
        "SELECT json_group_array(json_array(scope,total_limit,parallel_limit,consumed,active)) FROM (SELECT * FROM budgets ORDER BY scope)",
        "SELECT json_group_array(json_array(scope,document)) FROM (SELECT * FROM goal_ledgers ORDER BY scope)",
        "SELECT json_group_array(json_array(sequence,document)) FROM (SELECT * FROM events ORDER BY sequence)",
        "SELECT json_group_array(json_array(scope,key,payload,result)) FROM (SELECT * FROM idempotency ORDER BY scope,key)",
    ].iter().map(|sql|db.query_row(sql,[],|r|r.get(0)).unwrap()).collect()
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
    let original = original_metadata(&Connection::open(&p).unwrap());
    drop(Store::open(&p).unwrap());
    let db = Connection::open(&p).unwrap();
    assert_eq!(original_metadata(&db), original);
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        15
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
    assert_eq!(original_metadata(&backup), original);
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
    let original = original_metadata(&db);
    db.execute_batch("CREATE TABLE factory_child_plan_heads(sentinel BLOB);INSERT INTO factory_child_plan_heads VALUES(x'00ff');").unwrap();
    drop(db);
    assert_eq!(Store::open(&p).err().unwrap().code, "INTERNAL_ERROR");
    let db = Connection::open(&p).unwrap();
    assert_eq!(original_metadata(&db), original);
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
fn future_schema16_refuses_without_mutation_or_backup() {
    let (d, p, _) = old();
    let db = Connection::open(&p).unwrap();
    db.execute_batch("PRAGMA user_version=16;").unwrap();
    drop(db);
    let before = fs::read(&p).unwrap();
    assert_eq!(Store::open(&p).err().unwrap().code, "VERSION_UNSUPPORTED");
    assert_eq!(before, fs::read(&p).unwrap());
    assert!(backups(d.path()).is_empty());
}
