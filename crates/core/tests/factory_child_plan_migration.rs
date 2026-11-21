//! Generated schema13 metadata increment followed by current Store19, exact backup and rollback.
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
    let foreign_keys: bool = db
        .pragma_query_value(None, "foreign_keys", |row| row.get(0))
        .unwrap();
    db.pragma_update(None, "foreign_keys", false).unwrap();
    let generated = (|| -> rusqlite::Result<()> {
        let transaction = db.unchecked_transaction()?;
        schema19_legacy_migration::pre_host_schema::remove_empty_host19_in_transaction(
            &transaction,
        )
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
        pre_factory_schema::remove_empty_current_child_goal_increment(&transaction)?;
        transaction.execute_batch("DROP TABLE factory_child_plan_heads;DROP TABLE factory_child_plan_revisions;PRAGMA user_version=13;CREATE TABLE original_bytes(raw BLOB);INSERT INTO original_bytes VALUES(x'00ffdeadbeef');")?;
        transaction.commit()
    })();
    let restored = db.pragma_update(None, "foreign_keys", foreign_keys);
    generated.unwrap();
    restored.unwrap();
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
    let predecessor = legacy_predecessor(&p, 13);
    let store = Store::open(&p).unwrap();
    assert_eq!(store.sequence().unwrap(), 1);
    drop(store);
    let db = Connection::open(&p).unwrap();
    assert_empty_host19(&db);
    let current = fixture_logical_snapshot(&db);
    assert!(
        predecessor
            .rows
            .iter()
            .all(|table| current.rows.contains(table)),
        "legacy migration changed predecessor typed rows"
    );
    let host_backup = host19_backup(&p, &predecessor);
    assert_eq!(original_metadata(&db), original);
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
    assert!(fixture_logical_snapshot(&backup).rows == predecessor.rows);
    let legacy_backup_bytes = fs::read(&b[0]).unwrap();
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
    assert!(host19_backup(&p, &predecessor) == host_backup);
    let retained = backups(d.path());
    assert_eq!(retained.len(), 1);
    assert!(fs::read(&retained[0]).unwrap() == legacy_backup_bytes);
}
#[test]
fn child_plan_migration_collision_rolls_back_both_tables_and_preserves_old_bytes() {
    let (d, p, raw) = old();
    let keeper = Connection::open(&p).unwrap();
    keeper.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    let original = original_metadata(&keeper);
    keeper.execute_batch("CREATE TABLE factory_child_plan_heads(sentinel BLOB);INSERT INTO factory_child_plan_heads VALUES(x'00ff');").unwrap();
    let predecessor = fixture_logical_snapshot(&keeper);
    let before = database_bytes_with_wal(&p);
    assert_eq!(Store::open(&p).err().unwrap().code, "INTERNAL_ERROR");
    let db = Connection::open(&p).unwrap();
    assert!(
        database_bytes_with_wal(&p) == before,
        "failed migration changed main or WAL bytes"
    );
    assert!(
        fixture_logical_snapshot(&db) == predecessor,
        "failed migration changed the logical database"
    );
    host19_backup(&p, &predecessor);
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
fn future_schema20_refuses_without_mutation_or_backup() {
    let (d, p, _) = old();
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

#[test]
fn generated_fixture_inverse_restores_native_identity_without_factory_authority() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fresh-current.sqlite3");
    drop(Store::open(&path).unwrap());
    let db = Connection::open(&path).unwrap();
    db.pragma_update(None, "foreign_keys", true).unwrap();
    remove_empty_host19(&db).unwrap();
    pre_factory_schema::remove_empty_current_factory_increment(&db).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        12
    );
    for table in ["goal_scopes", "goal_runs"] {
        assert_eq!(db.query_row(&format!(r#"SELECT count(*) FROM pragma_foreign_key_list('{table}') WHERE "from"='id' AND "table"='approval_proposals' AND "to"='id'"#),[],|r|r.get::<_,u32>(0)).unwrap(),1);
    }
    assert!(!db
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .exists([])
        .unwrap());
    assert!(db
        .pragma_query_value::<bool, _>(None, "foreign_keys", |r| r.get(0))
        .unwrap());
    for table in pre_factory_schema::FACTORY_TABLES {
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM sqlite_master WHERE name=?1",
                [table],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            0
        );
    }
}
