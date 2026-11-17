use super::*;
use crate::host_registry_migration;
use crate::storage::{Event, Store};
use rusqlite::{params, types::Value};
use sha2::{Digest, Sha256};
use std::path::Path;

#[path = "../tests/fixtures/current19_schema.rs"]
mod current19_schema;
#[path = "../tests/fixtures/pre_host_schema.rs"]
mod pre_host_schema;

type RawObject = (String, String, String, Option<String>);
#[derive(Debug, PartialEq)]
struct Image {
    version: i64,
    cookie: i64,
    application: i64,
    objects: Vec<RawObject>,
    temp_objects: Vec<RawObject>,
    databases: Vec<(i64, String, String)>,
    rows: Vec<(String, Vec<Vec<Value>>)>,
}

fn raw_objects(db: &Connection) -> Vec<RawObject> {
    db.prepare("SELECT type,name,tbl_name,sql FROM main.sqlite_schema ORDER BY type,name")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}
fn image(db: &Connection) -> Image {
    let objects = raw_objects(db);
    let temp_objects = db
        .prepare("SELECT type,name,tbl_name,sql FROM temp.sqlite_schema ORDER BY type,name")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let databases = db
        .prepare("PRAGMA database_list")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let rows = objects
        .iter()
        .filter(|o| o.0 == "table")
        .map(|o| {
            let name = o.1.replace('"', "\"\"");
            let mut statement = db
                .prepare(&format!("SELECT rowid,* FROM \"{name}\" ORDER BY rowid"))
                .unwrap();
            let count = statement.column_count();
            let values = statement
                .query_map([], |r| {
                    (0..count)
                        .map(|i| r.get(i))
                        .collect::<rusqlite::Result<Vec<Value>>>()
                })
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap();
            (o.1.clone(), values)
        })
        .collect();
    Image {
        version: db
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap(),
        cookie: db
            .pragma_query_value(None, "schema_version", |r| r.get(0))
            .unwrap(),
        application: db
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .unwrap(),
        objects,
        temp_objects,
        databases,
        rows,
    }
}
fn logical_hash(value: &Image) -> String {
    format!("{:x}", Sha256::digest(format!("{value:?}").as_bytes()))
}
fn unchanged(db: &Connection, before: Image, label: &str) {
    let after = image(db);
    eprintln!(
        "{}",
        serde_json::json!({"case":label,"logical_before_sha256":logical_hash(&before),"logical_after_sha256":logical_hash(&after),"logical_equal":before==after})
    );
    assert_eq!(before, after, "{label}");
}
fn seeded18() -> Connection {
    let mut store = Store::open(Path::new(":memory:")).unwrap();
    store
        .create("local", "preserve original 中文 history", "original:create")
        .unwrap();
    store
        .conn
        .pragma_update(None, "application_id", 731)
        .unwrap();
    store.conn
}
fn seeded19() -> Connection {
    let mut db = seeded18();
    migrate(&mut db).unwrap();
    db
}
fn event(db: &Connection, kind: &str, id: &str) -> (i64, String) {
    creation_event(db, kind, id, "synthetic-create:v1")
}
fn creation_event(db: &Connection, kind: &str, id: &str, key: &str) -> (i64, String) {
    let document = serde_json::to_string(&Event {
        sequence: 0, event_id: uuid::Uuid::new_v4().to_string(), aggregate_id: id.into(), revision: 1,
        kind: kind.into(), occurred_at: "2026-11-18T00:00:00Z".into(),
        payload: serde_json::json!({"payload_version":1,"host_id":id,"revision":1,"creation_key":key,"registration_sha256":"4a7cba703c7db8252649273ddfeca6764efe51526ee3fe4a9eeb625eb891c3d5"}),
        source: "user_action".into(),
    }).unwrap();
    db.execute("INSERT INTO events(document) VALUES(?1)", [&document])
        .unwrap();
    (db.last_insert_rowid(), document)
}
// Synthetic SQL fixtures only, not an implementation/proof of B's typed v1
// canonical encoder or immutable registry API. Registration/record/receipt text
// retains the B4e sample field order; Event.payload remains the existing Value.
const REGISTRATION: &str = r#"{"schema_version":1,"name":"Synthetic remote Host","declared_platform":"linux","endpoint":{"expected_host_identity":"test_host","address":"never-resolve.example.invalid","user":"baton","port":22,"host_key_alias":"approved_alias"},"known_hosts":{"reference":"known-hosts:synthetic-v1","file_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"transport_identity_ref":"unknown:synthetic-transport-identity","configuration":{"reference":"companion-config:synthetic-v1","declared_ownership":"external_read_only"},"directory_mappings":[{"id":"source","remote_root":"/synthetic/remote/source"}]}"#;
fn insert_host(db: &Connection, id: &str, key: &str) -> (i64, String) {
    let (sequence, event) = creation_event(db, HOST_EVENT_KIND, id, key);
    let record = format!(
        r#"{{"record_version":1,"id":"{id}","revision":1,"created_sequence":{sequence},"created_at":"2026-11-18T00:00:00Z","creation_key":"{key}","registration":{REGISTRATION},"creation_facts":{{"local_validation":"configuration_syntax_validated","references":"unresolved","remote_probe":"not_probed","remote_connection":"not_contacted","remote_capabilities":"unknown","execution_enabled":false}}}}"#
    );
    let receipt = format!(
        r#"{{"receipt_version":1,"host_id":"{id}","revision":1,"created_sequence":{sequence},"created_at":"2026-11-18T00:00:00Z","registration":{REGISTRATION}}}"#
    );
    db.execute("INSERT INTO host_registrations(id,name,name_key,revision,created_sequence,document) VALUES(?1,?2,?2,1,?3,?4)", params![id,"Synthetic remote Host",sequence,record]).unwrap();
    db.execute(
        "INSERT INTO idempotency(scope,key,payload,result) VALUES(?1,?2,?3,?4)",
        params![HOST_IDEMPOTENCY_SCOPE, key, REGISTRATION, receipt],
    )
    .unwrap();
    (sequence, event)
}

#[test]
fn ordinary_store_is_still18_and_has_no_host_table() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("ordinary.sqlite3");
    let store = Store::open(&path).unwrap();
    assert_eq!(version(&store.conn).unwrap(), 18);
    assert!(!raw_objects(&store.conn).iter().any(|o| o.1 == HOST_TABLE));
    drop(store);
    let store = Store::open(&path).unwrap();
    assert_eq!(version(&store.conn).unwrap(), 18);
    assert_eq!(
        std::fs::read_dir(directory.path())
            .unwrap()
            .filter_map(std::result::Result::ok)
            .filter(|p| p.file_name().to_string_lossy().contains("pre-v19"))
            .count(),
        0
    );
}

#[test]
fn pinned_ddl_reference_tables_indexes_fk_and_cookie_delta_are_real_sqlite() {
    assert_eq!(
        format!("{:x}", Sha256::digest(HOST_DDL.as_bytes())),
        HOST_DDL_SHA256
    );
    let a = schema18_reference().unwrap();
    let b = schema19_reference().unwrap();
    assert_eq!(
        raw_objects(&a).iter().filter(|o| o.0 == "table").count(),
        63
    );
    assert_eq!(
        raw_objects(&b).iter().filter(|o| o.0 == "table").count(),
        64
    );
    assert_eq!(image(&b).cookie, image(&a).cookie + 2);
    assert_eq!(
        b.prepare("PRAGMA main.table_info(host_registrations)")
            .unwrap()
            .query_map([], |_| Ok(()))
            .unwrap()
            .count(),
        6
    );
    let indexes: Vec<(String, i64)> = b
        .prepare("PRAGMA main.index_list(host_registrations)")
        .unwrap()
        .query_map([], |r| Ok((r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(indexes.len(), 3); // PK/UNIQUE implicit indexes + the named index.
    assert!(indexes.contains(&("host_registrations_name_id".into(), 0)));
    let fk: (String, String, String, String, String) = b
        .query_row(
            "PRAGMA main.foreign_key_list(host_registrations)",
            [],
            |r| Ok((r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )
        .unwrap();
    assert_eq!(
        fk,
        (
            "events".into(),
            "created_sequence".into(),
            "sequence".into(),
            "NO ACTION".into(),
            "NO ACTION".into()
        )
    );
    assert_eq!(
        raw_objects(&b)
            .iter()
            .filter(|o| matches!(o.0.as_str(), "trigger" | "view") && o.2 == HOST_TABLE)
            .count(),
        0
    );
}

#[test]
fn migration_preserves_nonempty_old_history_opaque_rows_and_idempotent_recognition() {
    let mut db = seeded18();
    db.execute_batch("CREATE TABLE \"opaque data\"(id TEXT PRIMARY KEY, raw BLOB, note TEXT);CREATE INDEX opaque_note ON \"opaque data\"(note);INSERT INTO \"opaque data\" VALUES('x',x'00ffdeadbeef','preserve  NOT NULL');CREATE TABLE pragma_table_list(raw BLOB);CREATE TABLE pragma_foreign_key_list(raw BLOB);CREATE TABLE pragma_table_xinfo(raw BLOB);CREATE TABLE pragma_index_list(raw BLOB);CREATE TABLE pragma_index_xinfo(raw BLOB);").unwrap();
    let before = image(&db);
    migrate(&mut db).unwrap();
    let after = image(&db);
    assert_eq!(after.version, 19);
    assert_eq!(after.cookie, before.cookie + 2);
    assert_eq!(after.application, before.application);
    assert_eq!(
        after
            .objects
            .iter()
            .filter(|o| o.2 != HOST_TABLE)
            .collect::<Vec<_>>(),
        before.objects.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        after
            .rows
            .iter()
            .filter(|r| r.0 != HOST_TABLE)
            .collect::<Vec<_>>(),
        before.rows.iter().collect::<Vec<_>>()
    );
    migrate(&mut db).unwrap();
    unchanged(&db, after, "idempotent19_recognition");
    let before = image(&db);
    assert!(pre_host_schema::remove_empty_host19(&db).is_err()); // stricter TEST fixture policy.
    unchanged(&db, before, "opaque_product_data_not_fixture_extension");
}

#[test]
fn real_sql_sequence_unique_fk_no_action_and_repeated_name_semantics() {
    let mut db = seeded19();
    let id = "11111111-1111-4111-8111-111111111111";
    let (sequence, document) = insert_host(&db, id, "synthetic-create:v1");
    assert!(sequence > 0);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&document).unwrap()["sequence"],
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT created_sequence FROM host_registrations WHERE id=?1",
            [id],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        sequence
    );
    assert!(db
        .execute("DELETE FROM events WHERE sequence=?1", [sequence])
        .is_err());
    assert!(db
        .execute(
            "UPDATE events SET sequence=987654 WHERE sequence=?1",
            [sequence]
        )
        .is_err());
    assert!(db
        .execute("UPDATE host_registrations SET revision=2 WHERE id=?1", [id])
        .is_err());
    assert!(db
        .execute(
            "INSERT INTO host_registrations VALUES('duplicate','n','n',1,?1,'{}')",
            [sequence]
        )
        .is_err());
    assert!(db
        .execute(
            "INSERT INTO host_registrations VALUES('orphan','n','n',1,987654,'{}')",
            []
        )
        .is_err());
    insert_host(
        &db,
        "22222222-2222-4222-8222-222222222222",
        "synthetic-create:v2",
    );
    assert_eq!(db.query_row("SELECT count(*) FROM host_registrations WHERE name=name_key AND name='Synthetic remote Host'",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    let before = image(&db);
    migrate(&mut db).unwrap();
    unchanged(&db, before, "nonempty19_structural_recognition");
    let before = image(&db);
    assert!(pre_host_schema::remove_empty_host19(&db).is_err());
    unchanged(&db, before, "nonempty_host_never_downgraded");
}

#[test]
fn unsupported_versions_and_foreign_key_policy_refuse_without_changes() {
    for v in [-1i64, 0, 17, 20] {
        let mut db = seeded18();
        db.pragma_update(None, "user_version", v).unwrap();
        let before = image(&db);
        assert!(migrate(&mut db).is_err());
        unchanged(&db, before, &format!("unsupported_{v}"));
    }
    let mut db = seeded18();
    db.pragma_update(None, "foreign_keys", false).unwrap();
    let before = image(&db);
    assert!(migrate(&mut db).is_err());
    unchanged(&db, before, "fk_policy_not_silently_enabled");
    assert!(!db
        .pragma_query_value(None, "foreign_keys", |r| r.get::<_, bool>(0))
        .unwrap());
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert!(migrate(&mut db).is_err());
    db.execute_batch("ROLLBACK").unwrap();
}

#[test]
fn predecessor_and_opaque_schema_mutations_fail_before_destructive_work() {
    for sql in [
        "ALTER TABLE tasks ADD COLUMN unreviewed TEXT",
        "CREATE INDEX unreviewed ON tasks(id)",
        "CREATE VIEW unknown_view AS SELECT * FROM events",
        "CREATE TRIGGER unknown_trigger AFTER INSERT ON events BEGIN SELECT 1; END",
        "CREATE TABLE opaque(id TEXT, event INTEGER REFERENCES events(sequence))",
        "CREATE TABLE opaque(id TEXT, generated TEXT GENERATED ALWAYS AS(id) VIRTUAL)",
        "CREATE TABLE opaque(id TEXT);CREATE INDEX opaque_expression ON opaque(lower(id))",
        "CREATE TABLE opaque(id TEXT);CREATE INDEX opaque_partial ON opaque(id) WHERE id IS NOT NULL",
        "CREATE TABLE integration_future(id TEXT)",
        "CREATE TABLE host_registrations(id TEXT)",
    ] {
        let mut db=seeded18(); db.execute_batch(sql).unwrap(); let before=image(&db);
        assert!(migrate(&mut db).is_err(),"{sql}"); unchanged(&db,before,sql);
    }
}

#[test]
fn host_ddl_index_fk_and_constraint_mutations_refuse_idempotence_and_inverse() {
    let mutations = [
        HOST_DDL.replace("revision=1", "revision>=1"),
        HOST_DDL.replace("name TEXT NOT NULL", "name TEXTNOTNULL"),
        HOST_DDL.replace(
            "REFERENCES events(sequence)",
            "REFERENCES events(sequence) ON DELETE CASCADE",
        ),
        HOST_DDL.replace("(name_key,id)", "(name_key)"),
        HOST_DDL.replace(
            "CREATE INDEX host_registrations_name_id",
            "CREATE UNIQUE INDEX host_registrations_name_id",
        ),
        HOST_DDL.replace(
            "created_sequence INTEGER NOT NULL UNIQUE",
            "created_sequence INTEGER NOT NULL",
        ),
    ];
    for ddl in mutations {
        let mut db = seeded19();
        db.execute_batch("DROP TABLE host_registrations").unwrap();
        db.execute_batch(&ddl).unwrap();
        let before = image(&db);
        assert!(migrate(&mut db).is_err());
        unchanged(&db, before, "changed_host_ddl");
        let before = image(&db);
        assert!(pre_host_schema::remove_empty_host19(&db).is_err());
        unchanged(&db, before, "changed_host_inverse_ddl");
    }
    for sql in [
        "DROP INDEX host_registrations_name_id",
        "DROP TABLE host_registrations",
    ] {
        let db = seeded19();
        db.execute_batch(sql).unwrap();
        let before = image(&db);
        assert!(pre_host_schema::remove_empty_host19(&db).is_err());
        unchanged(&db, before, sql);
    }
}

#[test]
fn inverse_refuses_unknown_extensions_dependencies_and_mixed_versions() {
    for sql in [
        "CREATE TABLE host_future(raw BLOB)",
        "CREATE TABLE unrelated_sentinel(raw BLOB)",
        "CREATE TABLE dependent(id TEXT,host TEXT REFERENCES host_registrations(id))",
        "CREATE INDEX extra_host_index ON host_registrations(name)",
        "CREATE VIEW external_dependency AS SELECT * FROM host_registrations",
        "CREATE TRIGGER external_dependency AFTER INSERT ON host_registrations BEGIN SELECT 1; END",
        "CREATE TEMP VIEW hidden_dependency AS SELECT * FROM main.host_registrations",
        "ATTACH DATABASE ':memory:' AS unrecognized",
        "PRAGMA user_version=18",
        "PRAGMA user_version=20",
    ] {
        let db = seeded19();
        db.execute_batch(sql).unwrap();
        let before = image(&db);
        assert!(pre_host_schema::remove_empty_host19(&db).is_err(), "{sql}");
        unchanged(&db, before, sql);
    }
}

#[test]
fn inverse_refuses_orphan_unknown_and_malformed_host_event_receipt_evidence() {
    for kind in [
        HOST_EVENT_KIND,
        "host.future.v2",
        "HOST.future",
        "hosts.registered",
        "host",
    ] {
        let db = seeded19();
        event(&db, kind, "11111111-1111-4111-8111-111111111111");
        let before = image(&db);
        assert!(pre_host_schema::remove_empty_host19(&db).is_err());
        unchanged(&db, before, kind);
    }
    for scope in [
        HOST_IDEMPOTENCY_SCOPE,
        "hosts.register:v2",
        "HOSTS.future",
        "host.unknown",
    ] {
        let db = seeded19();
        db.execute(
            "INSERT INTO idempotency VALUES(?1,'orphan','bad-json','bad-json')",
            [scope],
        )
        .unwrap();
        let before = image(&db);
        assert!(pre_host_schema::remove_empty_host19(&db).is_err());
        unchanged(&db, before, scope);
    }
    for raw in [
        "{",
        r#"{"kind":1}"#,
        r#"{"unexpected":"host.registered"}"#,
        r#"{"kind":"host.registered","kind":"project.imported"}"#,
    ] {
        let db = seeded19();
        db.execute("INSERT INTO events(document) VALUES(?1)", [raw])
            .unwrap();
        let before = image(&db);
        assert!(pre_host_schema::remove_empty_host19(&db).is_err());
        unchanged(&db, before, "malformed_event_classification");
    }
}

#[test]
fn inverse_checks_row_and_byte_bounds_without_silent_truncation() {
    let db = seeded19();
    db.execute(
        "INSERT INTO events(document) VALUES(?1)",
        ["x".repeat(262_145)],
    )
    .unwrap();
    let before = image(&db);
    assert!(pre_host_schema::remove_empty_host19(&db).is_err());
    unchanged(&db, before, "oversized_event");
    let db = seeded19();
    db.execute_batch("WITH RECURSIVE n(i) AS(VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<10001) INSERT INTO idempotency(scope,key,payload,result) SELECT 'other',CAST(i AS TEXT),'{}','{}' FROM n;").unwrap();
    let before = image(&db);
    assert!(pre_host_schema::remove_empty_host19(&db).is_err());
    unchanged(&db, before, "namespace_row_bound");
}

#[test]
fn broken_old_foreign_keys_and_corrupt_database_are_not_empty_proof() {
    let db = seeded19();
    db.pragma_update(None, "foreign_keys", false).unwrap();
    db.execute_batch(
        "INSERT INTO integration_policies VALUES('bad','missing',1,'missing','missing',1,'{}')",
    )
    .unwrap();
    db.pragma_update(None, "foreign_keys", true).unwrap();
    let before = image(&db);
    assert!(validate_current19(&db).is_err());
    assert!(pre_host_schema::remove_empty_host19(&db).is_err());
    unchanged(&db, before, "broken_old_fk");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("corrupt.sqlite3");
    let original = b"this is not a SQLite database";
    std::fs::write(&path, original).unwrap();
    let mut db = Connection::open(&path).unwrap();
    assert!(migrate(&mut db).is_err());
    assert!(pre_host_schema::remove_empty_host19(&db).is_err());
    drop(db);
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn strict_inverse_preserves_old_rows_and_original_helpers_keep_exact_versions() {
    let db = seeded19();
    let before = image(&db);
    assert!(current19_schema::pre_conversation_schema::verify_empty_conversation18(&db).is_err());
    assert!(current19_schema::pre_integration_schema::verify_empty_integration17(&db).is_err());
    assert!(
        current19_schema::pre_integration_schema::verify_empty_integration17_current18(&db)
            .is_err()
    );
    assert!(current19_schema::pre_conversation_schema::remove_empty_conversation18(&db).is_err());
    assert!(current19_schema::pre_integration_schema::remove_empty_integration17(&db).is_err());
    unchanged(&db, before, "old_helpers_refuse19");
    let before = image(&db);
    pre_host_schema::remove_empty_host19(&db).unwrap();
    let after = image(&db);
    assert_eq!(after.version, 18);
    assert_eq!(
        after.rows,
        before
            .rows
            .into_iter()
            .filter(|r| r.0 != HOST_TABLE)
            .collect::<Vec<_>>()
    );
    current19_schema::pre_integration_schema::verify_empty_integration17_current18(&db).unwrap();
    current19_schema::pre_conversation_schema::remove_empty_conversation18(&db).unwrap();
    current19_schema::pre_integration_schema::remove_empty_integration17(&db).unwrap();
    assert_eq!(version(&db).unwrap(), 16);
}

#[test]
fn current19_readonly_proof_and_atomic_original18_17_16_composition() {
    let db = seeded19();
    let before = image(&db);
    db.pragma_update(None, "query_only", true).unwrap();
    current19_schema::verify_empty_current19(&db).unwrap(); // any temporary PRAGMA write would fail.
    unchanged(&db, before, "current19_pure_readonly");
    db.pragma_update(None, "query_only", false).unwrap();
    let before = image(&db);
    {
        let tx = Transaction::new_unchecked(&db, TransactionBehavior::Immediate).unwrap();
        assert!(pre_host_schema::remove_empty_host19(&tx).is_err());
        assert!(current19_schema::remove_empty_current19(&tx).is_err());
        current19_schema::remove_empty_current19_in_transaction(&tx).unwrap();
        assert_eq!(version(&tx).unwrap(), 16);
        assert!(tx
            .execute_batch("CREATE TABLE events(late_collision TEXT)")
            .is_err());
        // Drop rolls back ALL Host/Conversation/Integration DDL and version.
    }
    unchanged(&db, before, "late_inverse_caller_failure_rolls_back19");
    current19_schema::remove_empty_current19(&db).unwrap();
    assert_eq!(version(&db).unwrap(), 16);
    assert!(db
        .pragma_query_value(None, "foreign_keys", |r| r.get::<_, bool>(0))
        .unwrap());
}

#[test]
fn late_forward_migration_failure_rolls_back_entire_extension() {
    let db = seeded18();
    let before = image(&db);
    {
        let tx = Transaction::new_unchecked(&db, TransactionBehavior::Immediate).unwrap();
        migrate_in_transaction(&tx).unwrap();
        assert_eq!(version(&tx).unwrap(), 19);
        assert!(tx
            .execute_batch("CREATE TABLE events(late_collision TEXT)")
            .is_err());
    }
    unchanged(&db, before, "late_forward_caller_failure");
    assert!(!raw_objects(&db).iter().any(|o| o.1 == HOST_TABLE));
}

#[test]
fn on_disk_refusal_measures_logical_and_raw_main_preservation_separately() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("private.sqlite3");
    let mut store = Store::open(&path).unwrap();
    store
        .create("local", "preserved disk history", "disk:create")
        .unwrap();
    store
        .conn
        .pragma_update(None, "journal_mode", "DELETE")
        .unwrap();
    migrate(&mut store.conn).unwrap();
    event(
        &store.conn,
        "host.future.v2",
        "11111111-1111-4111-8111-111111111111",
    );
    let logical = image(&store.conn);
    let raw = std::fs::read(&path).unwrap();
    assert!(pre_host_schema::remove_empty_host19(&store.conn).is_err());
    unchanged(&store.conn, logical, "disk_refusal_logical");
    let after = std::fs::read(&path).unwrap();
    assert_eq!(raw, after);
    eprintln!(
        "{}",
        serde_json::json!({"case":"disk_refusal_raw_main","journal_mode":"delete","before_sha256":format!("{:x}",Sha256::digest(&raw)),"after_sha256":format!("{:x}",Sha256::digest(&after)),"raw_equal":true,"wal":"not_present_not_tested"})
    );
}
