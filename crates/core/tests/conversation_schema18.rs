//! Controlled schema18 -> exact17 checks plus actual migrations through Store19.
//! The old inverse still receives schema18. Fixture data is locally generated.
#[path = "fixtures/conversation_current19.rs"]
mod current19;
#[path = "fixtures/pre_conversation_schema.rs"]
pub mod predecessor;
use agentbaton_core::{host_registry_migration, storage::Store};
use rusqlite::{types::Value, Connection, OpenFlags};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};
type Schema = Vec<(String, String, String, Option<String>)>;
type Image = (Schema, BTreeMap<String, Vec<Vec<Value>>>, u32);
fn image(db: &Connection) -> Image {
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
fn backup(root: &Path, version: u32, expected: &Image) -> (PathBuf, Vec<u8>) {
    let prefix = format!("state.pre-v{version}-");
    // macOS temp roots can contain the /var alias. Resolve the owned parent
    // directory before applying SQLite NOFOLLOW to the actual backup entry.
    let root = root.canonicalize().unwrap();
    let paths: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(&prefix)
                && path.extension().is_some_and(|ext| ext == "sqlite3")
        })
        .collect();
    assert_eq!(paths.len(), 1, "backup count for {prefix}");
    let path = paths.into_iter().next().unwrap();
    let metadata = fs::symlink_metadata(&path).unwrap();
    assert!(metadata.is_file());
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    let db = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .unwrap();
    assert_eq!(&image(&db), expected);
    drop(db);
    let bytes = fs::read(&path).unwrap();
    (path, bytes)
}

fn prepare_empty_conversation18(db: &Connection) {
    let before19 = image(db);
    current19::prepare_empty_conversation18(db).unwrap();
    current19::assert_host19_delta(&image(db), &before19);
}

#[test]
fn generated17_upgrade_preserves_every_old_row_and_private_backup_then_reopens_without_rewrite() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("state.sqlite3");
    let mut store = Store::open(&path).unwrap();
    let task = store.create("local", "retained task", "original").unwrap();
    drop(store);
    let db = Connection::open(&path).unwrap();
    prepare_empty_conversation18(&db);
    db.execute_batch(
        "CREATE TABLE original_blob(raw BLOB);INSERT INTO original_blob VALUES(x'00ffdeadbeef');",
    )
    .unwrap();
    predecessor::verify_empty_conversation18(&db).unwrap();
    let controlled18 = image(&db);
    assert_eq!(controlled18.2, 18);
    assert_eq!(controlled18.1.len(), 64);
    predecessor::remove_empty_conversation18(&db).unwrap();
    let before = image(&db);
    assert_eq!(before.2, 17);
    assert_eq!(before.1.len(), 60);
    drop(db);
    let store = Store::open(&path).unwrap();
    assert_eq!(store.get(&task.id).unwrap().goal, "retained task");
    let db = Connection::open(&path).unwrap();
    host_registry_migration::validate_current19(&db).unwrap();
    let after = image(&db);
    assert_eq!(after.1.len(), 65);
    current19::assert_host19_delta(&controlled18, &after);
    assert_eq!(
        controlled18
            .0
            .iter()
            .filter(|r| !predecessor::CONVERSATION18_TABLES.contains(&r.2.as_str()))
            .cloned()
            .collect::<Schema>(),
        before.0
    );
    assert_eq!(
        controlled18
            .1
            .iter()
            .filter(|(n, _)| !predecessor::CONVERSATION18_TABLES.contains(&n.as_str()))
            .map(|(n, v)| (n.clone(), v.clone()))
            .collect::<BTreeMap<_, _>>(),
        before.1
    );
    let before18_backup = backup(root.path(), 18, &before);
    let before19_backup = backup(root.path(), 19, &before);
    assert_ne!(before18_backup.0, before19_backup.0);
    drop(db);
    drop(store);
    let reopened = Store::open(&path).unwrap();
    assert!(reopened.list_conversations().unwrap().is_empty());
    assert_eq!(image(&Connection::open(&path).unwrap()), after);
    assert_eq!(backup(root.path(), 18, &before), before18_backup);
    assert_eq!(backup(root.path(), 19, &before), before19_backup);
}

#[test]
fn controlled18_migrates_to_current19_with_only_the_host_delta_and_preserves_its_backup() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("state.sqlite3");
    let mut store = Store::open(&path).unwrap();
    let task = store
        .create("local", "retained schema18 task", "schema18")
        .unwrap();
    drop(store);
    let db = Connection::open(&path).unwrap();
    prepare_empty_conversation18(&db);
    let before18 = image(&db);
    assert_eq!(before18.1.len(), 63);
    drop(db);

    let store = Store::open(&path).unwrap();
    assert_eq!(store.get(&task.id).unwrap().goal, "retained schema18 task");
    assert!(store.list_conversations().unwrap().is_empty());
    let db = Connection::open(&path).unwrap();
    host_registry_migration::validate_current19(&db).unwrap();
    let after19 = image(&db);
    assert_eq!(after19.1.len(), 64);
    current19::assert_host19_delta(&before18, &after19);
    let old_backup = backup(root.path(), 19, &before18);
    assert!(!fs::read_dir(root.path()).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("state.pre-v18-")
    }));
    drop(db);
    drop(store);

    let reopened = Store::open(&path).unwrap();
    assert_eq!(
        reopened.get(&task.id).unwrap().goal,
        "retained schema18 task"
    );
    assert!(reopened.list_conversations().unwrap().is_empty());
    assert_eq!(image(&Connection::open(&path).unwrap()), after19);
    assert_eq!(backup(root.path(), 19, &before18), old_backup);
}

#[test]
fn exact18_inverse_refuses_wrong_version_modified_domain_nonempty_or_external_dependency() {
    for (sql, reason) in [
        (
            "PRAGMA user_version=17;",
            "controlled empty Conversation18 inverse refused",
        ),
        (
            "ALTER TABLE conversation_turns ADD COLUMN unknown TEXT;",
            "changed Conversation18 DDL or index set",
        ),
        (
            "CREATE INDEX conversation_extra ON conversations(document);",
            "changed Conversation18 DDL or index set",
        ),
        (
            "CREATE VIEW outside AS SELECT * FROM conversations;",
            "external Conversation18 dependency",
        ),
        (
            "CREATE TABLE outside(id TEXT REFERENCES conversations(id));",
            "incoming Conversation18 foreign key",
        ),
        (
            "CREATE TABLE conversation_future(raw BLOB);",
            "unknown Conversation18 domain object",
        ),
        (
            "INSERT INTO conversation_budgets VALUES('untrusted','p','g','{}');",
            "nonempty Conversation18 history",
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("state.sqlite3");
        drop(Store::open(&path).unwrap());
        let db = Connection::open(&path).unwrap();
        prepare_empty_conversation18(&db);
        db.pragma_update(None, "foreign_keys", false).unwrap();
        db.execute_batch(sql).unwrap();
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        let before = image(&db);
        let raw = fs::read(&path).unwrap();
        assert_eq!(
            predecessor::remove_empty_conversation18(&db).unwrap_err(),
            reason,
            "{sql}"
        );
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
    prepare_empty_conversation18(&db);
    let before = image(&db);
    assert_eq!(before.2, 18);
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

#[test]
fn exact18_inverse_refuses_current_store19_without_relabel_or_change() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("state.sqlite3");
    drop(Store::open(&path).unwrap());
    let db = Connection::open(&path).unwrap();
    let before = image(&db);
    assert_eq!(before.2, 19);
    assert!(before.1.contains_key("host_registrations"));
    let raw = fs::read(&path).unwrap();
    assert_eq!(
        predecessor::remove_empty_conversation18(&db).unwrap_err(),
        "controlled empty Conversation18 inverse refused",
    );
    assert_eq!(image(&db), before);
    assert_eq!(fs::read(&path).unwrap(), raw);
}

#[test]
fn strict_current19_adapter_refuses_host_rows_and_inconsistent_foreign_keys_without_change() {
    for sql in [
        "INSERT INTO host_registrations VALUES('host','Generated','generated',1,1,'{}');",
        "INSERT INTO conversation_budgets VALUES('untrusted','p','g','{}');",
    ] {
        let error = "controlled empty Host19 inverse refused";
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("state.sqlite3");
        let mut store = Store::open(&path).unwrap();
        store
            .create("local", "Generated event for schema validation", "schema19")
            .unwrap();
        drop(store);
        let db = Connection::open(&path).unwrap();
        db.pragma_update(None, "foreign_keys", false).unwrap();
        db.execute_batch(sql).unwrap();
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        let before = image(&db);
        let raw = fs::read(&path).unwrap();
        assert_eq!(before.2, 19);
        assert_eq!(
            current19::pre_host_schema::remove_empty_host19(&db).unwrap_err(),
            error
        );
        assert_eq!(
            current19::prepare_empty_conversation18(&db).unwrap_err(),
            error
        );
        assert_eq!(image(&db), before);
        assert_eq!(fs::read(&path).unwrap(), raw);
    }
}
