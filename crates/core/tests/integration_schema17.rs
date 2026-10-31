//! Store17 persistence contract. Synthetic malformed predecessors below are
//! negatives only; genuine source16 data is tested by integration_actual16_migration.
//! Opaque compatibility inputs below are controlled generated schema16/17 inputs,
//! never claimed as genuine historical producers.
#[path = "fixtures/pre_factory_schema.rs"]
pub mod pre_factory_schema;
use agentbaton_core::storage::Store;
use rusqlite::{types::Value as SqlValue, Connection, OpenFlags};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

const NEW_TABLES: [&str; 4] = [
    "integration_policies",
    "integrations",
    "integration_attempts",
    "integration_ref_fences",
];
type Schema = Vec<(String, String, String, Option<String>)>;
#[derive(PartialEq)]
struct Image {
    schema: Schema,
    rows: BTreeMap<String, Vec<Vec<SqlValue>>>,
    version: u32,
    schema_version: u32,
    application_id: u32,
    journal: String,
}
fn ro(path: &Path) -> Connection {
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    db.pragma_update(None, "query_only", true).unwrap();
    db
}
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
        let quoted = name.replace('"', "\"\"");
        let mut query = db
            .prepare(&format!("SELECT rowid,* FROM \"{quoted}\" ORDER BY rowid"))
            .unwrap();
        let width = query.column_count();
        let values = query
            .query_map([], |r| {
                (0..width)
                    .map(|i| r.get::<_, SqlValue>(i))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        rows.insert(name.clone(), values);
    }
    Image {
        schema,
        rows,
        version: db
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap(),
        schema_version: db
            .pragma_query_value(None, "schema_version", |r| r.get(0))
            .unwrap(),
        application_id: db
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .unwrap(),
        journal: db
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .unwrap(),
    }
}
struct Retained(Option<tempfile::TempDir>);
impl Retained {
    fn path(&self) -> &Path {
        self.0.as_ref().unwrap().path()
    }
}
impl Drop for Retained {
    fn drop(&mut self) {
        if std::thread::panicking() {
            if let Some(root) = self.0.take() {
                eprintln!(
                    "Retained failed Store17 schema test: {}",
                    root.keep().display()
                );
            }
        }
    }
}
fn fixture() -> (Retained, PathBuf) {
    let root = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let path = root.path().join("state.sqlite3");
    drop(
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap(),
    );
    (Retained(Some(root)), path)
}
fn backups(root: &Path) -> Vec<PathBuf> {
    let mut result: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            name.starts_with("state.pre-v17-") && name.ends_with(".sqlite3")
        })
        .collect();
    result.sort();
    result
}
fn zero_authority(store: &Store, db: &Connection) {
    assert_eq!(
        serde_json::to_value(store.integration_record_counts().unwrap()).unwrap(),
        json!({"policies":0,"runs":0,"attempts":0,"ref_fences":0})
    );
    for table in NEW_TABLES.into_iter().chain([
        "approval_proposals",
        "execution_grants",
        "sessions",
        "attempts",
        "goal_scopes",
        "goal_runs",
        "factory_scopes",
        "factory_runs",
    ]) {
        let count: i64 = db
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(u64::try_from(count).expect("Negative domain row count"), 0);
    }
    assert!(!db
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .exists([])
        .unwrap());
}
fn preserved_backup(before: &Image, root: &Path) {
    let copies = backups(root);
    assert_eq!(copies.len(), 1);
    let metadata = fs::symlink_metadata(&copies[0]).unwrap();
    assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    assert_eq!(metadata.mode() & 0o7777, 0o600);
    assert_eq!(metadata.nlink(), 1);
    assert_eq!(metadata.uid(), rustix::process::geteuid().as_raw());
    let after = image(&ro(&copies[0]));
    // SQLite backup intentionally invalidates the destination schema cookie;
    // journal mode is a connection/backup setting, not predecessor row content.
    assert!(before.schema == after.schema && before.rows == after.rows);
    assert_eq!(before.version, after.version);
    assert_eq!(before.application_id, after.application_id);
}

#[test]
fn fresh_store17_is_additive_empty_authority_and_reopen_preserves_real_store_rows() {
    let (root, path) = fixture();
    let mut store = Store::open(&path).unwrap();
    let task = store
        .create("local", "Schema17 generated persistence only", "new-task")
        .unwrap();
    assert!(task.code_workspace_binding.is_none());
    let before = image(&ro(&path));
    assert_eq!(before.version, 17);
    assert_eq!(before.rows.len(), 59); // complete55 predecessor tables + four17 tables
    zero_authority(&store, &ro(&path));
    drop(store);
    let reopened = Store::open(&path).unwrap();
    assert_eq!(
        serde_json::to_value(reopened.get(&task.id).unwrap()).unwrap(),
        serde_json::to_value(&task).unwrap()
    );
    assert!(
        before == image(&ro(&path)),
        "Reopen rewrote persisted history"
    );
    zero_authority(&reopened, &ro(&path));
    assert!(backups(root.path()).is_empty());
}

#[test]
fn future18_refuses_before_backup_or_recovery_with_committed_wal_preserved() {
    let (root, path) = fixture();
    drop(Store::open(&path).unwrap());
    let keeper = Connection::open(&path).unwrap();
    keeper.execute_batch("PRAGMA wal_autocheckpoint=0; CREATE TABLE future_private(bytes BLOB); INSERT INTO future_private VALUES(x'00ff0123'); PRAGMA user_version=18;").unwrap();
    let wal = path.with_file_name("state.sqlite3-wal");
    let before = image(&keeper);
    let raw = fs::read(&path).unwrap();
    let wal_raw = fs::read(&wal).unwrap();
    assert!(wal_raw.len() > 32);
    assert_eq!(
        Store::open(&path).err().unwrap().code,
        "VERSION_UNSUPPORTED"
    );
    assert!(before == image(&keeper));
    assert_eq!(fs::read(&path).unwrap(), raw);
    assert_eq!(fs::read(&wal).unwrap(), wal_raw);
    assert!(backups(root.path()).is_empty());
}

#[test]
fn corrupt_non_sqlite_input_is_not_replaced_by_a_fresh_empty17_database() {
    let (root, path) = fixture();
    let original = b"This is retained invalid SQLite input.\0\xff";
    fs::write(&path, original).unwrap();
    assert!(Store::open(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(backups(root.path()).is_empty());
}

#[test]
fn synthetic_incomplete_label16_refuses_without_committing_partial17_authority() {
    let (root, path) = fixture();
    let db = Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE private_sentinel(bytes BLOB); INSERT INTO private_sentinel VALUES(x'00ffdeadbeef'); PRAGMA user_version=16;").unwrap();
    let before = image(&db);
    drop(db);
    let raw = fs::read(&path).unwrap();
    assert!(Store::open(&path).is_err());
    assert!(
        before == image(&ro(&path)),
        "Malformed predecessor partially migrated"
    );
    assert_eq!(fs::read(&path).unwrap(), raw);
    preserved_backup(&before, root.path());
}

#[test]
fn synthetic_late_ddl_conflict_rolls_back_all_prior17_ddl_version_and_wal_bytes() {
    let (root, path) = fixture();
    drop(Store::open(&path).unwrap());
    let keeper = Connection::open(&path).unwrap();
    // Explicitly synthetic structural negative, NEVER a genuine16 producer.
    // Collision at the final domain table must not leave the first three added.
    keeper.execute_batch("PRAGMA wal_autocheckpoint=0; DROP TABLE integration_ref_fences; DROP TABLE integration_attempts; DROP TABLE integrations; DROP TABLE integration_policies; CREATE TABLE integration_ref_fences(sentinel BLOB); INSERT INTO integration_ref_fences VALUES(x'00ff1234'); PRAGMA user_version=16;").unwrap();
    let before = image(&keeper);
    let wal = path.with_file_name("state.sqlite3-wal");
    let raw = fs::read(&path).unwrap();
    let wal_raw = fs::read(&wal).unwrap();
    assert!(wal_raw.len() > 32);
    assert!(Store::open(&path).is_err());
    assert!(
        before == image(&keeper),
        "DDL failure changed predecessor image"
    );
    assert_eq!(fs::read(&path).unwrap(), raw);
    assert_eq!(fs::read(&wal).unwrap(), wal_raw);
    for name in [
        "integration_policies",
        "integrations",
        "integration_attempts",
    ] {
        assert!(!image(&keeper).rows.contains_key(name));
    }
    preserved_backup(&before, root.path());
}

// These are generated test-only inverses, never genuine historical producers.
fn empty_current_fixture() -> (Retained, PathBuf, Connection) {
    let (root, path) = fixture();
    let mut store = Store::open(&path).unwrap();
    store
        .create("local", "Preserve original Task bytes", "inverse-original")
        .unwrap();
    drop(store);
    let db = Connection::open(&path).unwrap();
    let raw: String = db
        .query_row("SELECT document FROM tasks LIMIT 1", [], |row| row.get(0))
        .unwrap();
    let pretty =
        serde_json::to_string_pretty(&serde_json::from_str::<serde_json::Value>(&raw).unwrap())
            .unwrap()
            + "\n";
    db.execute("UPDATE tasks SET document=?1", [&pretty])
        .unwrap();
    db.pragma_update(None, "foreign_keys", true).unwrap();
    (root, path, db)
}
fn inverse_settings(db: &Connection) -> Vec<SqlValue> {
    [
        "foreign_keys",
        "defer_foreign_keys",
        "query_only",
        "synchronous",
    ]
    .into_iter()
    .map(|name| db.pragma_query_value(None, name, |row| row.get(0)).unwrap())
    .collect()
}
#[test]
fn controlled_empty17_inverse_preserves_all_predecessor_rows_and_exact_schema_then_upgrades() {
    use pre_factory_schema::pre_integration_schema::{
        remove_empty_integration17, verify_empty_integration17, INTEGRATION17_TABLES,
    };
    let (root, path, db) = empty_current_fixture();
    let before = image(&db);
    db.pragma_update(None, "query_only", true).unwrap();
    verify_empty_integration17(&db).unwrap();
    assert!(
        image(&db) == before,
        "readonly proof cannot modify even a schema cookie"
    );
    db.pragma_update(None, "query_only", false).unwrap();
    let settings = inverse_settings(&db);
    remove_empty_integration17(&db).unwrap();
    let after = image(&db);
    let expected_schema: Schema = before
        .schema
        .iter()
        .filter(|row| !INTEGRATION17_TABLES.contains(&row.2.as_str()))
        .cloned()
        .collect();
    let expected_rows: BTreeMap<_, _> = before
        .rows
        .iter()
        .filter(|(name, _)| !INTEGRATION17_TABLES.contains(&name.as_str()))
        .map(|(name, rows)| (name.clone(), rows.clone()))
        .collect();
    assert_eq!(after.version, 16);
    assert!(
        after.schema == expected_schema && after.rows == expected_rows,
        "exactly the four empty domains may disappear"
    );
    assert_eq!(after.application_id, before.application_id);
    assert_eq!(after.journal, before.journal);
    assert_eq!(inverse_settings(&db), settings);
    assert!(db.is_autocommit());
    drop(db);
    let store = Store::open(&path).unwrap();
    let db = ro(&path);
    let restored = image(&db);
    assert_eq!(restored.version, 17);
    assert!(
        restored.schema == before.schema && restored.rows == before.rows,
        "known synthetic16 roundtrip preserves all raw rows and original DDL"
    );
    verify_empty_integration17(&db).unwrap();
    assert_eq!(store.integration_record_counts().unwrap().policies, 0);
    assert_eq!(backups(root.path()).len(), 1);
}
#[test]
fn controlled_empty17_inverse_refuses_wrong_version_changed_domain_and_incoming_dependencies() {
    use pre_factory_schema::pre_integration_schema::{
        remove_empty_integration17, INTEGRATION17_TABLES,
    };
    let mut variants: Vec<(String, String)> = vec![];
    for version in [0, 15, 16, 18] {
        variants.push((
            format!("version{version}"),
            format!("PRAGMA user_version={version};"),
        ));
    }
    for table in INTEGRATION17_TABLES {
        variants.push((format!("missing:{table}"), format!("DROP TABLE {table};")));
        variants.push((
            format!("column:{table}"),
            format!("ALTER TABLE {table} ADD COLUMN unknown_fixture_field TEXT;"),
        ));
        variants.push((
            format!("index:{table}"),
            format!("CREATE INDEX unknown_domain_index ON {table}(rowid);"),
        ));
        variants.push((format!("trigger:{table}"),format!("CREATE TRIGGER unknown_domain_trigger AFTER INSERT ON {table} BEGIN SELECT 1; END;")));
    }
    for index in [
        "integration_policies_task",
        "integrations_task",
        "integration_attempts_root",
        "integration_attempts_run",
    ] {
        variants.push((
            format!("missing_index:{index}"),
            format!("DROP INDEX {index};"),
        ));
    }
    variants.extend([
        ("external_view".into(),"CREATE VIEW fixture_external_view AS SELECT * FROM integrations;".into()),
        ("incoming_fk".into(),"CREATE TABLE fixture_incoming(id TEXT REFERENCES integrations(id));".into()),
        ("incoming_fk_ascii_case".into(),"CREATE TABLE fixture_incoming(id TEXT REFERENCES INTEGRATIONS(id));".into()),
        ("unknown_increment".into(),"CREATE TABLE integration_future_extension(raw BLOB);".into()),
        ("nonempty:policies".into(),"INSERT INTO integration_policies VALUES('p','t',1,'s','a',1,'{}');".into()),
        ("nonempty:runs".into(),"INSERT INTO integrations VALUES('r','p','t',1,'queued',1,'{}');".into()),
        ("nonempty:attempts".into(),"INSERT INTO integration_attempts VALUES('a','r',1,'preparation','integration','t','g','p','prepared','{}');".into()),
        ("nonempty:fences".into(),"INSERT INTO integration_ref_fences VALUES('repo','refs/heads/generated','r',1,'held','{}');".into()),
    ]);
    for (variant, sql) in variants {
        let (_root, path, db) = empty_current_fixture();
        // Malformed rows here are bounded refusal inputs only, never authority.
        db.pragma_update(None, "foreign_keys", false).unwrap();
        let sql = if variant.starts_with("index:") {
            sql.replace("(rowid)", "(document)")
        } else {
            sql
        };
        db.execute_batch(&sql).unwrap();
        db.pragma_update(None, "foreign_keys", true).unwrap();
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        let before = image(&db);
        let settings = inverse_settings(&db);
        let main = fs::read(&path).unwrap();
        let error = remove_empty_integration17(&db).expect_err(&variant);
        if variant.starts_with("nonempty:") {
            assert_eq!(error, "nonempty Integration17 history");
        }
        assert!(
            image(&db) == before,
            "refused inverse changed logical image: {variant}"
        );
        assert_eq!(inverse_settings(&db), settings, "{variant}");
        assert!(
            fs::read(&path).unwrap() == main,
            "refused inverse changed main bytes: {variant}"
        );
        assert!(db.is_autocommit(), "{variant}");
    }
}
#[test]
fn controlled_current17_then_original_factory_inverse_keeps_one_rollback_boundary() {
    let (_root, path, db) = empty_current_fixture();
    db.execute_batch(
        "CREATE TABLE fixture_caller_collision(sentinel TEXT);PRAGMA wal_checkpoint(TRUNCATE);",
    )
    .unwrap();
    let before = image(&db);
    let main = fs::read(&path).unwrap();
    let settings = inverse_settings(&db);
    db.pragma_update(None, "foreign_keys", false).unwrap();
    let mut reached_late_collision = false;
    let generated = (|| -> rusqlite::Result<()> {
        let transaction = db.unchecked_transaction()?;
        pre_factory_schema::remove_empty_current_child_goal_increment(&transaction)?;
        assert_eq!(
            transaction.pragma_query_value::<u32, _>(None, "user_version", |row| row.get(0))?,
            14
        );
        reached_late_collision = true;
        transaction.execute_batch("CREATE TABLE fixture_caller_collision(sentinel TEXT);")?;
        transaction.commit()
    })();
    db.pragma_update(None, "foreign_keys", true).unwrap();
    assert!(
        reached_late_collision,
        "both known inverse increments must run before the deliberate caller fault"
    );
    let exact_collision = match generated {
        Err(rusqlite::Error::SqliteFailure(error, Some(message))) => {
            error.extended_code == rusqlite::ffi::SQLITE_ERROR
                && message == "table fixture_caller_collision already exists"
        }
        Err(rusqlite::Error::SqlInputError {
            error,
            msg,
            sql,
            offset,
        }) => {
            error.extended_code == rusqlite::ffi::SQLITE_ERROR
                && msg == "table fixture_caller_collision already exists"
                && sql == "CREATE TABLE fixture_caller_collision(sentinel TEXT);"
                && offset == 13
        }
        _ => false,
    };
    assert!(exact_collision);
    assert!(
        image(&db) == before,
        "caller failure must restore17, every Integration object and old raw history"
    );
    assert_eq!(inverse_settings(&db), settings);
    assert!(fs::read(&path).unwrap() == main);
    assert!(db.is_autocommit());
}
#[test]
fn controlled_empty17_outer_refuses_nested_and_inner_refuses_committed_transactions() {
    use pre_factory_schema::pre_integration_schema::{
        remove_empty_integration17, remove_empty_integration17_in_transaction,
    };
    let (_root, _path, db) = empty_current_fixture();
    let transaction = db.unchecked_transaction().unwrap();
    let before = image(&transaction);
    assert!(remove_empty_integration17(&transaction).is_err());
    assert!(image(&transaction) == before);
    transaction.execute_batch("COMMIT").unwrap();
    let before = image(&transaction);
    assert!(remove_empty_integration17_in_transaction(&transaction).is_err());
    assert!(image(&transaction) == before);
}

#[test]
fn controlled_empty17_inverse_preserves_unrelated_unknown_schema_and_typed_bytes() {
    use pre_factory_schema::pre_integration_schema::{
        remove_empty_integration17, INTEGRATION17_TABLES,
    };
    let (_root, _path, db) = empty_current_fixture();
    db.execute_batch("CREATE TABLE fixture_original(raw BLOB,txt TEXT,whole INTEGER,real_value REAL,nothing_value TEXT);INSERT INTO fixture_original VALUES(x'00ffdeadbeef','untouched',37,1.25,NULL);").unwrap();
    let before = image(&db);
    remove_empty_integration17(&db).unwrap();
    let after = image(&db);
    let expected_schema: Schema = before
        .schema
        .into_iter()
        .filter(|row| !INTEGRATION17_TABLES.contains(&row.2.as_str()))
        .collect();
    let expected_rows: BTreeMap<_, _> = before
        .rows
        .into_iter()
        .filter(|(name, _)| !INTEGRATION17_TABLES.contains(&name.as_str()))
        .collect();
    assert!(after.schema == expected_schema && after.rows == expected_rows);
    // This extra table is a controlled generated preservation input only, not
    // a genuine historical predecessor. Production formerly rejected all extra
    // tables; the separate opaque compatibility tests cover the new policy.
    assert_eq!(after.version, 16);
}

#[test]
fn controlled16_opaque_blob_and_own_indexes_survive_upgrade_backup_and17_reopen() {
    use pre_factory_schema::pre_integration_schema::remove_empty_integration17;
    let (root, path, db) = empty_current_fixture();
    remove_empty_integration17(&db).unwrap();
    // Generated compatibility input, not a genuine historical producer. Data
    // remains uninterpreted even when it resembles permission-bearing JSON.
    db.execute_batch(r#"
        CREATE TABLE original_blob(bytes BLOB, raw TEXT UNIQUE, n INTEGER, r REAL, absent TEXT);
        CREATE INDEX original_blob_bytes ON original_blob(bytes);
        INSERT INTO original_blob(rowid,bytes,raw,n,r,absent)
          VALUES(47,x'00ffdeadbeef',' {"authority_kind":"integration","approved":true} ',37,1.25,NULL);
        CREATE TABLE "opaque""; SELECT"(bytes BLOB);
        INSERT INTO "opaque""; SELECT"(rowid,bytes) VALUES(83,x'ff000102');
    "#).unwrap();
    let before = image(&db);
    assert_eq!(before.version, 16);
    drop(db);
    let store = Store::open(&path).unwrap();
    let after = image(&ro(&path));
    assert_eq!(after.version, 17);
    let old_schema: Schema = after
        .schema
        .iter()
        .filter(|row| !NEW_TABLES.contains(&row.2.as_str()))
        .cloned()
        .collect();
    let old_rows: BTreeMap<_, _> = after
        .rows
        .iter()
        .filter(|(name, _)| !NEW_TABLES.contains(&name.as_str()))
        .map(|(name, rows)| (name.clone(), rows.clone()))
        .collect();
    assert!(before.schema == old_schema && before.rows == old_rows);
    zero_authority(&store, &ro(&path));
    preserved_backup(&before, root.path());
    let backup_raw = fs::read(&backups(root.path())[0]).unwrap();
    drop(store);
    let reopened = Store::open(&path).unwrap();
    assert!(
        image(&ro(&path)) == after,
        "reopen changed opaque or known history"
    );
    zero_authority(&reopened, &ro(&path));
    assert_eq!(backups(root.path()).len(), 1);
    assert_eq!(fs::read(&backups(root.path())[0]).unwrap(), backup_raw);
}

#[test]
fn opaque_compatibility_never_substitutes_for_known_objects_or_accepts_dependencies() {
    use pre_factory_schema::pre_integration_schema::remove_empty_integration17;
    let variants = [
        ("changed_known_ddl", "ALTER TABLE tasks ADD COLUMN unknown_fixture_field TEXT;"),
        ("missing_known_index", "DROP INDEX goal_one_active_scope_per_task;"),
        ("changed_known_index", "DROP INDEX goal_one_active_scope_per_task; CREATE UNIQUE INDEX goal_one_active_scope_per_task ON goal_scopes(task_id);"),
        ("missing_known_trigger", "DROP TRIGGER command_fence_attempt;"),
        ("changed_known_trigger", "DROP TRIGGER command_fence_attempt; CREATE TRIGGER command_fence_attempt BEFORE INSERT ON attempts BEGIN SELECT 1; END;"),
        ("missing_known_table", "DROP TABLE command_workspace_fences;"),
        ("attached_index", "CREATE INDEX opaque_attached ON tasks(id);"),
        ("attached_trigger", "CREATE TRIGGER opaque_attached AFTER INSERT ON tasks BEGIN SELECT 1; END;"),
        ("opaque_trigger", "CREATE TABLE opaque_data(raw BLOB); CREATE TRIGGER opaque_attached AFTER INSERT ON opaque_data BEGIN SELECT 1; END;"),
        ("opaque_view", "CREATE VIEW opaque_view AS SELECT * FROM tasks;"),
        ("reserved_table", "CREATE TABLE InTeGrAtIoN_future(raw BLOB);"),
        ("reserved_index", "CREATE TABLE opaque_data(raw BLOB); CREATE INDEX InTeGrAtIoN_future ON opaque_data(raw);"),
        ("known_name_other_kind", "CREATE TRIGGER TaSkS AFTER INSERT ON tasks BEGIN SELECT 1; END;"),
        ("incoming_authority_fk", "CREATE TABLE opaque_data(id TEXT REFERENCES TaSkS(id));"),
        ("incoming_future_fk", "CREATE TABLE opaque_data(id TEXT REFERENCES InTeGrAtIoNs(id));"),
        ("opaque_cross_fk", "CREATE TABLE opaque_a(id TEXT PRIMARY KEY); CREATE TABLE opaque_b(id TEXT REFERENCES opaque_a(id));"),
        ("generated_column", "CREATE TABLE opaque_data(raw TEXT,derived TEXT GENERATED ALWAYS AS (raw) VIRTUAL);"),
        ("stored_generated_column", "CREATE TABLE opaque_data(raw TEXT,derived TEXT GENERATED ALWAYS AS (raw) STORED);"),
        ("expression_index", "CREATE TABLE opaque_data(raw TEXT); CREATE INDEX opaque_expression ON opaque_data(lower(raw));"),
        ("partial_index", "CREATE TABLE opaque_data(raw BLOB); CREATE INDEX opaque_partial ON opaque_data(raw) WHERE raw IS NOT NULL;"),
    ];
    for version in [16, 17] {
        for (variant, sql) in variants {
            let (root, path, db) = empty_current_fixture();
            if version == 16 {
                remove_empty_integration17(&db).unwrap();
            }
            db.pragma_update(None, "foreign_keys", false).unwrap();
            db.execute_batch(sql).unwrap();
            db.execute_batch("PRAGMA wal_autocheckpoint=0; PRAGMA wal_checkpoint(TRUNCATE);")
                .unwrap();
            // Retain a committed WAL frame too: a refusal must not quietly
            // checkpoint/rewrite the input, raw Task text, or partial17 DDL.
            db.execute_batch("UPDATE tasks SET document=document || ' '; ")
                .unwrap();
            let before = image(&db);
            let raw = fs::read(&path).unwrap();
            let wal = path.with_file_name("state.sqlite3-wal");
            let wal_raw = fs::read(&wal).unwrap();
            let error = Store::open(&path).err().expect(variant);
            assert_eq!(error.code, "INPUT_CHANGED", "version{version}:{variant}");
            assert!(
                before == image(&db),
                "logical rollback version{version}:{variant}"
            );
            assert_eq!(
                fs::read(&path).unwrap(),
                raw,
                "main rollback version{version}:{variant}"
            );
            assert_eq!(
                fs::read(&wal).unwrap(),
                wal_raw,
                "WAL rollback version{version}:{variant}"
            );
            if version == 16 {
                preserved_backup(&before, root.path());
            } else {
                assert!(backups(root.path()).is_empty());
            }
        }
    }
}

#[test]
fn opaque_virtual_and_shadow_tables_are_refused_before_rows_are_evaluated() {
    use pre_factory_schema::pre_integration_schema::remove_empty_integration17;
    for version in [16, 17] {
        let (root, path, db) = empty_current_fixture();
        if version == 16 {
            remove_empty_integration17(&db).unwrap();
        }
        db.execute_batch("CREATE VIRTUAL TABLE opaque_search USING fts5(raw); INSERT INTO opaque_search VALUES('uninterpreted'); PRAGMA wal_checkpoint(TRUNCATE);").unwrap();
        // FTS shadow tables can be WITHOUT ROWID; avoid the ordinary-row image
        // helper and prove exact source bytes/DDL plus opaque contents instead.
        let schema: Schema = db
            .prepare("SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let raw = fs::read(&path).unwrap();
        let error = Store::open(&path)
            .err()
            .expect("virtual/shadow schema must refuse");
        assert_eq!(error.code, "INPUT_CHANGED");
        assert_eq!(fs::read(&path).unwrap(), raw);
        let after: Schema = db
            .prepare("SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert!(schema == after);
        if version == 16 {
            let copies = backups(root.path());
            assert_eq!(copies.len(), 1);
            let backup = ro(&copies[0]);
            assert_eq!(
                backup
                    .pragma_query_value::<u32, _>(None, "user_version", |r| r.get(0))
                    .unwrap(),
                16
            );
            assert_eq!(
                backup
                    .query_row("SELECT raw FROM opaque_search", [], |r| r
                        .get::<_, String>(0))
                    .unwrap(),
                "uninterpreted"
            );
        } else {
            assert!(backups(root.path()).is_empty());
        }
    }
}

#[test]
fn opaque16_at_object_budget_refuses_additive17_overflow_atomically() {
    use pre_factory_schema::pre_integration_schema::remove_empty_integration17;
    let (root, path, db) = empty_current_fixture();
    remove_empty_integration17(&db).unwrap();
    let count: u32 = db
        .query_row("SELECT count(*) FROM sqlite_schema", [], |r| r.get(0))
        .unwrap();
    assert!(count < 256);
    for number in count..256 {
        // Numeric generated identifiers, not user-controlled SQL fragments.
        db.execute_batch(&format!("CREATE TABLE opaque_budget_{number}(raw BLOB);"))
            .unwrap();
    }
    db.execute_batch("PRAGMA wal_autocheckpoint=0; PRAGMA wal_checkpoint(TRUNCATE); UPDATE tasks SET document=document || ' ';").unwrap();
    let before = image(&db);
    let raw = fs::read(&path).unwrap();
    let wal = path.with_file_name("state.sqlite3-wal");
    let wal_raw = fs::read(&wal).unwrap();
    let error = Store::open(&path)
        .err()
        .expect("new17 objects exceed unchanged budget");
    assert_eq!(error.code, "INPUT_CHANGED");
    assert!(before == image(&db));
    assert_eq!(fs::read(&path).unwrap(), raw);
    assert_eq!(fs::read(&wal).unwrap(), wal_raw);
    preserved_backup(&before, root.path());
}
