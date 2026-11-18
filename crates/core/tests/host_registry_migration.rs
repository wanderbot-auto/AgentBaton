//! Normal Core19 activation API tests; structural predecessor cases are synthetic.
use agentbaton_core::{host_registry_migration, storage::Store};
use agentbaton_remote_host::registration::HostRegistration;
use rusqlite::{types::Value, Connection};
use std::{collections::BTreeMap, fs, os::unix::fs::OpenOptionsExt};
fn rows(db: &Connection) -> BTreeMap<String, Vec<Vec<Value>>> {
    let names: Vec<String> = db
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    names
        .into_iter()
        .map(|name| {
            let mut s = db
                .prepare(&format!(
                    "SELECT rowid,* FROM \"{}\" ORDER BY rowid",
                    name.replace('"', "\"\"")
                ))
                .unwrap();
            let n = s.column_count();
            let data = s
                .query_map([], |r| {
                    (0..n)
                        .map(|i| r.get(i))
                        .collect::<rusqlite::Result<Vec<Value>>>()
                })
                .unwrap()
                .map(Result::unwrap)
                .collect();
            (name, data)
        })
        .collect()
}
#[test]
fn normal19_registration_reopen_audits_receipt_and_preserves_creation_replay() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("normal.sqlite3");
    let metadata: HostRegistration =
        serde_json::from_str(include_str!("fixtures/host_registration/payload-v1.json")).unwrap();
    let mut store = Store::open(&path).unwrap();
    let receipt = store
        .register_host(&metadata, "normal19-registration")
        .unwrap();
    drop(store);
    let before = {
        let db = Connection::open(&path).unwrap();
        rows(&db)
    };
    let mut store = Store::open(&path).unwrap();
    assert_eq!(
        store
            .find_host_registration("normal19-registration")
            .unwrap(),
        Some(receipt.clone())
    );
    assert_eq!(
        store
            .register_host(&metadata, "normal19-registration")
            .unwrap(),
        receipt
    );
    drop(store);
    let db = Connection::open(&path).unwrap();
    assert_eq!(rows(&db), before);
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        19
    );
    assert_eq!(
        fs::read_dir(root.path())
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("pre-v19-"))
            .count(),
        0
    );
}
#[test]
fn structural18_increment_preserves_raw_opaque_idempotency_and_exact_host_objects() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("structural18.sqlite3");
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&path)
        .unwrap();
    let reference = host_registry_migration::schema18_reference().unwrap();
    reference.backup(rusqlite::MAIN_DB, &path, None).unwrap();
    let db = Connection::open(&path).unwrap();
    db.execute(
        "INSERT INTO idempotency(scope,key,payload,result) VALUES('original:opaque','bytes',?1,?2)",
        rusqlite::params![vec![0u8, 255], vec![222u8, 173]],
    )
    .unwrap();
    let before = rows(&db);
    let cookie: i64 = db
        .pragma_query_value(None, "schema_version", |r| r.get(0))
        .unwrap();
    drop(db);
    drop(Store::open(&path).unwrap());
    let db = Connection::open(&path).unwrap();
    host_registry_migration::validate_current19(&db).unwrap();
    let after = rows(&db);
    for (name, value) in before {
        assert_eq!(after[&name], value);
    }
    assert_eq!(
        db.pragma_query_value(None, "schema_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        cookie + 2
    );
    assert!(after["host_registrations"].is_empty());
    assert_eq!(
        fs::read_dir(root.path())
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".pre-v19-")
                && e.as_ref()
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|e| e == "sqlite3"))
            .count(),
        1
    );
}
#[test]
fn malformed_predeclared_host18_is_backed_up_but_never_partially_migrated() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("refused.sqlite3");
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&path)
        .unwrap();
    host_registry_migration::schema18_reference()
        .unwrap()
        .backup(rusqlite::MAIN_DB, &path, None)
        .unwrap();
    let db = Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE host_registrations(raw TEXT);")
        .unwrap();
    let before = rows(&db);
    drop(db);
    assert!(Store::open(&path).is_err());
    let db = Connection::open(&path).unwrap();
    assert_eq!(rows(&db), before);
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        18
    );
}
