//! Synthetic current18 -> exact17 controls, never a genuine historical producer.
#[path = "fixtures/pre_conversation_schema.rs"]
pub mod predecessor;
use agentbaton_core::storage::Store;
use rusqlite::{types::Value, Connection};
use std::{collections::BTreeMap, fs, os::unix::fs::PermissionsExt};
type Schema = Vec<(String, String, String, Option<String>)>;
fn image(db: &Connection) -> (Schema, BTreeMap<String, Vec<Vec<Value>>>, u32) {
    let schema: Schema = db
        .prepare("SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut rows = BTreeMap::new();
    for (_, name, _, _) in schema.iter().filter(|r| r.0 == "table") {
        let mut q = db
            .prepare(&format!(
                "SELECT rowid,* FROM \"{}\" ORDER BY rowid",
                name.replace('"', "\"\"")
            ))
            .unwrap();
        let width = q.column_count();
        let values = q
            .query_map([], |r| {
                (0..width)
                    .map(|i| r.get(i))
                    .collect::<rusqlite::Result<Vec<Value>>>()
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        rows.insert(name.clone(), values);
    }
    (
        schema,
        rows,
        db.pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap(),
    )
}
#[test]
fn generated17_upgrade_preserves_every_old_row_and_private_backup_then_reopens_without_rewrite() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("state.sqlite3");
    let mut store = Store::open(&path).unwrap();
    let task = store.create("local", "retained task", "original").unwrap();
    drop(store);
    let db = Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE original_blob(raw BLOB);INSERT INTO original_blob VALUES(x'00ffdeadbeef');",
    )
    .unwrap();
    predecessor::remove_empty_conversation18(&db).unwrap();
    let before = image(&db);
    assert_eq!(before.2, 17);
    assert_eq!(before.1.len(), 60);
    drop(db);
    let store = Store::open(&path).unwrap();
    assert_eq!(store.get(&task.id).unwrap().goal, "retained task");
    let db = Connection::open(&path).unwrap();
    predecessor::verify_empty_conversation18(&db).unwrap();
    let after = image(&db);
    assert_eq!(after.2, 18);
    assert_eq!(after.1.len(), 64);
    assert_eq!(
        after
            .0
            .iter()
            .filter(|r| !predecessor::CONVERSATION18_TABLES.contains(&r.2.as_str()))
            .cloned()
            .collect::<Schema>(),
        before.0
    );
    assert_eq!(
        after
            .1
            .iter()
            .filter(|(n, _)| !predecessor::CONVERSATION18_TABLES.contains(&n.as_str()))
            .map(|(n, v)| (n.clone(), v.clone()))
            .collect::<BTreeMap<_, _>>(),
        before.1
    );
    let backups: Vec<_> = fs::read_dir(root.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("state.pre-v18-")
                && p.extension().is_some_and(|e| e == "sqlite3")
        })
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(
        fs::metadata(&backups[0]).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let backup = Connection::open(&backups[0]).unwrap();
    assert_eq!(image(&backup), before);
    drop(backup);
    drop(db);
    drop(store);
    let reopened = Store::open(&path).unwrap();
    assert!(reopened.list_conversations().unwrap().is_empty());
    assert_eq!(image(&Connection::open(&path).unwrap()), after);
}
#[test]
fn exact18_inverse_refuses_wrong_version_modified_domain_nonempty_or_external_dependency() {
    for sql in [
        "PRAGMA user_version=17;",
        "ALTER TABLE conversation_turns ADD COLUMN unknown TEXT;",
        "CREATE INDEX conversation_extra ON conversations(title);",
        "CREATE VIEW outside AS SELECT * FROM conversations;",
        "CREATE TABLE outside(id TEXT REFERENCES conversations(id));",
        "CREATE TABLE conversation_future(raw BLOB);",
        "INSERT INTO conversation_budgets VALUES('untrusted','p','g','{}');",
    ] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("state.sqlite3");
        drop(Store::open(&path).unwrap());
        let db = Connection::open(&path).unwrap();
        db.pragma_update(None, "foreign_keys", false).unwrap();
        let sql = sql.replace("conversations(title)", "conversations(document)");
        db.execute_batch(&sql).unwrap();
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        let before = image(&db);
        let raw = fs::read(&path).unwrap();
        assert!(predecessor::remove_empty_conversation18(&db).is_err());
        assert_eq!(image(&db), before);
        assert_eq!(fs::read(&path).unwrap(), raw);
    }
}
#[test]
fn inverse_participates_in_callers_atomic_boundary_without_accepting_nested_outer_call() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("state.sqlite3");
    drop(Store::open(&path).unwrap());
    let db = Connection::open(&path).unwrap();
    let before = image(&db);
    {
        let tx = db.unchecked_transaction().unwrap();
        assert!(predecessor::remove_empty_conversation18(&tx).is_err());
        predecessor::remove_empty_conversation18_in_transaction(&tx).unwrap();
        assert_eq!(
            tx.pragma_query_value::<u32, _>(None, "user_version", |r| r.get(0))
                .unwrap(),
            17
        );
    }
    assert_eq!(image(&db), before);
}
