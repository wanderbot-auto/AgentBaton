//! Current Store19 adapter for isolated, generated legacy migration tests.

use agentbaton_core::{
    host_registry_migration,
    storage::{Event, Store},
};
use rusqlite::{Connection, OpenFlags};
use std::{
    fs,
    io::ErrorKind,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

#[path = "pre_host_schema.rs"]
pub mod pre_host_schema;
pub use pre_host_schema::remove_empty_host19;

// No Debug implementation: a failed equality must not dump raw fixture rows.
type FixtureSchemaObject = (String, String, String, Option<String>);
pub type FixtureTableRows = (String, Vec<Vec<rusqlite::types::Value>>);
#[derive(PartialEq)]
pub struct FixtureLogicalSnapshot {
    schema: Vec<FixtureSchemaObject>,
    pub rows: Vec<FixtureTableRows>,
    pragmas: Vec<(&'static str, rusqlite::types::Value)>,
    autocommit: bool,
}

pub fn fixture_logical_snapshot(connection: &Connection) -> FixtureLogicalSnapshot {
    let schema: Vec<FixtureSchemaObject> = connection
        .prepare("SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .map(|row| row.unwrap())
        .collect();
    let mut rows = Vec::new();
    for (_, name, _, _) in schema.iter().filter(|row| row.0 == "table") {
        // All tables in these isolated fixtures have rowids. Include internal
        // sqlite_sequence too, and quote even the generated identifiers.
        let quoted = format!("\"{}\"", name.replace('"', "\"\""));
        let mut statement = connection
            .prepare(&format!("SELECT rowid,* FROM {quoted} ORDER BY rowid"))
            .unwrap();
        let columns = statement.column_count();
        let data = statement
            .query_map([], |row| {
                (0..columns)
                    .map(|column| row.get::<_, rusqlite::types::Value>(column))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap()
            .map(|row| row.unwrap())
            .collect();
        rows.push((name.clone(), data));
    }
    let pragmas = [
        "user_version",
        "schema_version",
        "application_id",
        "foreign_keys",
        "journal_mode",
        "defer_foreign_keys",
        "query_only",
        "synchronous",
    ]
    .into_iter()
    .map(|name| {
        let value = connection
            .pragma_query_value(None, name, |row| row.get::<_, rusqlite::types::Value>(0))
            .unwrap();
        (name, value)
    })
    .collect();
    FixtureLogicalSnapshot {
        schema,
        rows,
        pragmas,
        autocommit: connection.is_autocommit(),
    }
}

/// Read an owned generated predecessor before the real forward Store open.
pub fn legacy_predecessor(path: &Path, version: u32) -> FixtureLogicalSnapshot {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_eq!(
        connection
            .pragma_query_value::<u32, _>(None, "user_version", |row| row.get(0))
            .unwrap(),
        version
    );
    fixture_logical_snapshot(&connection)
}

/// Verify the real current Store output using the production schema recognizer.
pub fn assert_empty_host19(connection: &Connection) {
    host_registry_migration::validate_current19(connection).unwrap();
    let empty: bool = connection
        .query_row(
            "SELECT NOT EXISTS(SELECT 1 FROM host_registrations)
            AND NOT EXISTS(SELECT 1 FROM events WHERE json_extract(document,'$.kind')=?1)
            AND NOT EXISTS(SELECT 1 FROM idempotency WHERE scope=?2)",
            [
                host_registry_migration::HOST_EVENT_KIND,
                host_registry_migration::HOST_IDEMPOTENCY_SCOPE,
            ],
            |row| row.get(0),
        )
        .unwrap();
    assert!(empty, "legacy migration created Host authority");
}

#[derive(PartialEq)]
pub struct PublishedBackup {
    path: PathBuf,
    inode: u64,
    bytes: Vec<u8>,
}

pub fn host19_backup(path: &Path, before: &FixtureLogicalSnapshot) -> PublishedBackup {
    let parent = path.parent().unwrap();
    let prefix = format!("{}.pre-v19-", path.file_stem().unwrap().to_str().unwrap());
    let copies: Vec<_> = fs::read_dir(parent)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|copy| {
            copy.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with(&prefix)
                && copy
                    .extension()
                    .is_some_and(|extension| extension == "sqlite3")
        })
        .collect();
    assert_eq!(copies.len(), 1);
    let copy = &copies[0];
    let metadata = fs::symlink_metadata(copy).unwrap();
    assert!(metadata.is_file());
    assert_eq!(metadata.uid(), rustix::process::geteuid().as_raw());
    assert_eq!(metadata.mode() & 0o7777, 0o600);
    assert_eq!(metadata.nlink(), 1);
    let backup = Connection::open_with_flags(copy, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let after = fixture_logical_snapshot(&backup);
    assert!(
        after.schema == before.schema && after.rows == before.rows,
        "Host backup differs from predecessor DDL or typed rows"
    );
    for pragma in ["user_version", "application_id"] {
        let actual = |image: &FixtureLogicalSnapshot| {
            image
                .pragmas
                .iter()
                .find(|(name, _)| *name == pragma)
                .unwrap()
                .1
                .clone()
        };
        assert_eq!(actual(&after), actual(before), "{pragma}");
    }
    PublishedBackup {
        path: copy.clone(),
        inode: metadata.ino(),
        bytes: fs::read(copy).unwrap(),
    }
}

#[derive(PartialEq)]
pub struct RawDatabase {
    main: Vec<u8>,
    wal: Option<Vec<u8>>,
}

pub fn database_bytes(path: &Path) -> RawDatabase {
    let mut wal = path.as_os_str().to_os_string();
    wal.push("-wal");
    let wal = match fs::read(wal) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => panic!("cannot read test fixture WAL: {error}"),
    };
    RawDatabase {
        main: fs::read(path).unwrap(),
        wal,
    }
}

pub fn database_bytes_with_wal(path: &Path) -> RawDatabase {
    let bytes = database_bytes(path);
    assert!(bytes.wal.as_ref().is_some_and(|wal| wal.len() > 32));
    bytes
}

pub fn assert_future20_refused(path: &Path) {
    let before = database_bytes(path);
    let error = Store::open(path).err().unwrap();
    assert_eq!(error.code, "VERSION_UNSUPPORTED");
    assert_eq!(error.message, "Database was created by a newer application");
    assert!(
        database_bytes(path) == before,
        "future refusal changed SQLite bytes"
    );
    assert!(
        fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .all(|name| !name.to_string_lossy().contains(".pre-v")),
        "future refusal wrote a migration backup"
    );
}
