//! Exact synthetic19 inverse, not a genuine historical producer or product downgrade.
use agentbaton_core::{host_registry_migration, storage::Event};
use rusqlite::{types::Value, Connection};
use std::{
    collections::BTreeMap,
    ffi::{c_char, c_int, c_void, CStr},
};
#[path = "fixtures/current19_factory_schema.rs"]
pub mod current19_factory_schema;
#[path = "fixtures/pre_factory_schema.rs"]
pub mod pre_factory_schema;
#[path = "fixtures/pre_host_schema.rs"]
pub mod pre_host_schema;

type Objects = Vec<(String, String, String, Option<String>)>;
#[derive(Debug, PartialEq)]
struct Image {
    objects: Objects,
    rows: BTreeMap<String, Vec<Vec<Value>>>,
    version: i64,
    cookie: i64,
    application: i64,
}
fn image(db: &Connection) -> Image {
    let objects: Objects = db
        .prepare("SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let rows = objects
        .iter()
        .filter(|o| o.0 == "table")
        .map(|o| {
            let quoted = o.1.replace('"', "\"\"");
            let mut s = db
                .prepare(&format!("SELECT rowid,* FROM \"{quoted}\" ORDER BY rowid"))
                .unwrap();
            let width = s.column_count();
            let values = s
                .query_map([], |r| {
                    (0..width)
                        .map(|i| r.get(i))
                        .collect::<rusqlite::Result<Vec<Value>>>()
                })
                .unwrap()
                .map(Result::unwrap)
                .collect();
            (o.1.clone(), values)
        })
        .collect();
    Image {
        objects,
        rows,
        version: db
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap(),
        cookie: db
            .pragma_query_value(None, "schema_version", |r| r.get(0))
            .unwrap(),
        application: db
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .unwrap(),
    }
}
fn fixture(foreign_keys: bool) -> Connection {
    let db = host_registry_migration::schema19_reference().unwrap();
    db.pragma_update(None, "foreign_keys", foreign_keys)
        .unwrap();
    db.pragma_update(None, "application_id", 731).unwrap();
    db.execute("INSERT INTO idempotency(scope,key,payload,result) VALUES('original:bytes','retained',?1,?2)",
        rusqlite::params![vec![0u8,255,128], vec![222u8,173,190,239]]).unwrap();
    db
}
fn fk(db: &Connection) -> bool {
    db.pragma_query_value(None, "foreign_keys", |r| r.get(0))
        .unwrap()
}

#[test]
fn factory19_whole_source_tail_and_sql_pins_refuse_outside_or_tail_edits() {
    let source = include_str!("fixtures/pre_factory_schema.rs");
    let sql = current19_factory_schema::pinned_factory_sql(source).unwrap();
    assert!(sql.ends_with("PRAGMA user_version=12;"));
    for changed in [
        source.to_owned() + "\n",
        source.replace("user_version=12", "user_version=13"),
        source.replacen("DROP TABLE", "DROP  TABLE", 1),
    ] {
        assert!(current19_factory_schema::pinned_factory_sql(&changed).is_err());
    }
}

#[test]
fn factory19_single_transaction_reaches12_and_restores_initial_fk_on_and_off() {
    for initial in [true, false] {
        let db = fixture(initial);
        let bytes = image(&db).rows["idempotency"].clone();
        current19_factory_schema::with_empty_current19_factory(&db, |tx| {
            assert_eq!(
                tx.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                    .unwrap(),
                12
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(image(&db).version, 12);
        assert_eq!(image(&db).rows["idempotency"], bytes);
        assert_eq!(fk(&db), initial);
        assert!(!db
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .exists([])
            .unwrap());
    }
}

#[test]
fn factory19_late_failure_restores_complete_raw_image_and_fk() {
    for initial in [true, false] {
        let db = fixture(initial);
        let before = image(&db);
        let error=current19_factory_schema::with_empty_current19_factory(&db, |tx| {
            tx.execute_batch("PRAGMA application_id=99;DELETE FROM idempotency;CREATE TABLE events(conflict INTEGER);")
        }).unwrap_err();
        assert!(error.operation.is_some());
        assert!(error.restore.is_none());
        assert!(!error.commit_observed);
        assert_eq!(image(&db), before);
        assert_eq!(fk(&db), initial);
    }
}

#[test]
fn factory19_refuses_existing_extension_nonempty_host_future_version_and_nested_transaction() {
    for sql in [
        "CREATE TABLE sentinel(raw BLOB)",
        "DROP INDEX host_registrations_name_id",
        "PRAGMA user_version=20",
    ] {
        let db = fixture(true);
        db.execute_batch(sql).unwrap();
        let before = image(&db);
        let error =
            current19_factory_schema::with_empty_current19_factory(&db, |_| Ok(())).unwrap_err();
        assert!(error.operation.is_some());
        assert!(!error.commit_observed);
        assert_eq!(image(&db), before);
        assert!(fk(&db));
    }
    let db = fixture(true);
    let tx = db.unchecked_transaction().unwrap();
    assert!(current19_factory_schema::with_empty_current19_factory(&tx, |_| Ok(())).is_err());
    tx.rollback().unwrap();
}
unsafe extern "C" fn deny_commit(_: *mut c_void) -> c_int {
    1
}
unsafe extern "C" fn deny_fk_write(
    _: *mut c_void,
    action: c_int,
    first: *const c_char,
    second: *const c_char,
    _: *const c_char,
    _: *const c_char,
) -> c_int {
    if action == rusqlite::ffi::SQLITE_PRAGMA && !first.is_null() && !second.is_null() {
        let name = unsafe { CStr::from_ptr(first) }.to_bytes();
        if name == b"foreign_keys" {
            return rusqlite::ffi::SQLITE_DENY;
        }
    }
    rusqlite::ffi::SQLITE_OK
}
#[test]
fn factory19_commit_hook_abort_rolls_back_complete_image_and_restores_fk() {
    for initial in [true, false] {
        let db = fixture(initial);
        let before = image(&db);
        unsafe {
            rusqlite::ffi::sqlite3_commit_hook(
                db.handle(),
                Some(deny_commit),
                std::ptr::null_mut(),
            );
        }
        let error =
            current19_factory_schema::with_empty_current19_factory(&db, |_| Ok(())).unwrap_err();
        unsafe {
            rusqlite::ffi::sqlite3_commit_hook(db.handle(), None, std::ptr::null_mut());
        }
        assert!(error.operation.is_some());
        assert!(error.restore.is_none());
        assert!(!error.commit_observed);
        assert_eq!(image(&db), before);
        assert_eq!(fk(&db), initial);
    }
}
#[test]
fn factory19_restore_failure_is_explicit_and_never_erases_first_material_error() {
    for initial in [true, false] {
        for fail_operation in [true, false] {
            let db = fixture(initial);
            let before = image(&db);
            let error = current19_factory_schema::with_empty_current19_factory(&db, |tx| {
                // Install only after the initial FK-off operation and inverse work;
                // the static callback denies the captured-policy restoration write.
                unsafe {
                    assert_eq!(
                        rusqlite::ffi::sqlite3_set_authorizer(
                            tx.handle(),
                            Some(deny_fk_write),
                            std::ptr::null_mut()
                        ),
                        rusqlite::ffi::SQLITE_OK
                    );
                }
                if fail_operation {
                    Err(rusqlite::Error::InvalidQuery)
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            unsafe {
                rusqlite::ffi::sqlite3_set_authorizer(db.handle(), None, std::ptr::null_mut());
            }
            assert!(error.restore.is_some());
            assert!(!fk(&db));
            assert_eq!(error.commit_observed, !fail_operation);
            if fail_operation {
                assert!(matches!(
                    error.operation,
                    Some(rusqlite::Error::InvalidQuery)
                ));
                assert_eq!(image(&db), before);
            } else {
                assert!(error.operation.is_none());
                assert_eq!(image(&db).version, 12);
            }
            db.pragma_update(None, "foreign_keys", initial).unwrap();
        }
    }
}

#[test]
fn factory19_nonempty_registered_host_and_linked_history_are_never_erased() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("host.sqlite3");
    let mut store = agentbaton_core::storage::Store::open(&path).unwrap();
    let metadata: agentbaton_remote_host::registration::HostRegistration =
        serde_json::from_str(include_str!("fixtures/host_registration/payload-v1.json")).unwrap();
    store.register_host(&metadata, "live-host").unwrap();
    drop(store);
    let db = Connection::open(&path).unwrap();
    db.pragma_update(None, "foreign_keys", true).unwrap();
    let before = image(&db);
    let error =
        current19_factory_schema::with_empty_current19_factory(&db, |_| Ok(())).unwrap_err();
    assert!(error.operation.is_some());
    assert_eq!(image(&db), before);
    assert!(fk(&db));
}
