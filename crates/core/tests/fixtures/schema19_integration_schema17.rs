//! Adapter for owned, generated Store19 test fixtures.
use super::{image, Image, Schema};
use agentbaton_core::{host_registry_migration, storage::Event, CoreError, Result};
use rusqlite::Connection;
use std::path::Path;

#[path = "pre_host_schema.rs"]
pub mod pre_host_schema;

/// Check exact Host19 DDL and emptiness before projecting its known increment
/// out of the image. Keep unrelated opaque schema objects and row values in
/// the projection so that callers compare them to their original input.
pub fn without_host19(mut current: Image) -> Image {
    let table = host_registry_migration::HOST_TABLE;
    let reference = image(&host_registry_migration::schema19_reference().unwrap());
    let host_objects = |schema: &Schema| -> Schema {
        schema
            .iter()
            .filter(|object| object.2 == table)
            .cloned()
            .collect()
    };
    assert_eq!(current.version, 19);
    let expected = host_objects(&reference.schema);
    assert_eq!(
        expected
            .iter()
            .map(|object| object.1.as_str())
            .collect::<Vec<_>>(),
        [
            "host_registrations_name_id",
            "sqlite_autoindex_host_registrations_1",
            "sqlite_autoindex_host_registrations_2",
            "host_registrations",
        ]
    );
    assert_eq!(host_objects(&current.schema), expected);
    assert!(current.rows.remove(table).unwrap().is_empty());
    current.schema.retain(|object| object.2 != table);
    current
}

// Frozen from the accepted Store18 production source:
// a4e03303b54bd5a5544131e95ad39662f8310e29:crates/core/src/storage.rs
// Git blob f9e1682c46cca41bbfbd3b0b902d82f33fc5e1e5, lines 57–68 and 99–110.
// Raw excerpts (mapper then open body) SHA-256 441a63157dac5258feefcf71c3ff0bb8a6b9d64ce48ade617ddef680b98813da.
// The test compiles and executes the open prefix through the actual refusal
// branch. The adapter only drops the binding mutability needed by the later
// migration code and returns the version on fallthrough for a positive control.
pub(crate) fn db_error(error: rusqlite::Error) -> CoreError {
    let mut e = CoreError::new(
        "INTERNAL_ERROR",
        "Local database operation failed; reconcile before retrying writes",
    );
    e.side_effect_state = "unknown".into();
    // Numeric diagnostics only; no SQL, paths, parameter values or engine text.
    if let rusqlite::Error::SqliteFailure(code, _) = error {
        e.details_ref = Some(format!("sqlite-code:{}", code.extended_code));
    }
    e
}
pub fn historical_open_version(path: &Path) -> Result<u32> {
    let conn = Connection::open(path).map_err(db_error)?;
    conn.busy_timeout(std::time::Duration::from_secs(3))
        .map_err(db_error)?;
    let version: u32 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(db_error)?;
    if version > 18 {
        return Err(CoreError::new(
            "VERSION_UNSUPPORTED",
            "Database was created by a newer application",
        ));
    }
    Ok(version)
}
