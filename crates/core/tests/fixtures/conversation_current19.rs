//! Test adapter for owned, generated current Store data only.
//! The existing strict Host19 inverse validates the source before any shape
//! change. The Conversation18 proof runs inside the same rollback boundary.
//! This fixture is not a historical executable or database producer.

use super::{image, predecessor, Image};
use agentbaton_core::{host_registry_migration, storage::Event};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use std::collections::BTreeMap;

#[path = "pre_host_schema.rs"]
pub mod pre_host_schema;

pub fn prepare_empty_conversation18(db: &Connection) -> Result<(), String> {
    if !db.is_autocommit() {
        return Err("Controlled Conversation18 preparation requires autocommit".into());
    }
    let tx = Transaction::new_unchecked(db, TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    pre_host_schema::remove_empty_host19_in_transaction(&tx)?;
    predecessor::verify_empty_conversation18(&tx)?;
    tx.commit().map_err(|e| e.to_string())
}

/// Compare actual persisted images on either side of the Host migration.
/// Check structural additions independently of retained schema and row values.
pub fn assert_host19_delta(before18: &Image, after19: &Image) {
    use host_registry_migration::{schema19_reference, HOST_TABLE};
    assert_eq!(before18.2, 18);
    assert_eq!(after19.2, 19);
    let reference = image(&schema19_reference().unwrap());
    let host_objects = |schema: &super::Schema| {
        schema
            .iter()
            .filter(|object| object.2 == HOST_TABLE)
            .cloned()
            .collect::<Vec<_>>()
    };
    let expected = host_objects(&reference.0);
    assert_eq!(
        expected
            .iter()
            .map(|object| object.1.as_str())
            .collect::<Vec<_>>(),
        [
            "host_registrations_name_id",
            "sqlite_autoindex_host_registrations_1",
            "sqlite_autoindex_host_registrations_2",
            "host_registrations"
        ],
    );
    assert!(!before18.1.contains_key(HOST_TABLE));
    assert!(after19.1[HOST_TABLE].is_empty());
    assert_eq!(host_objects(&after19.0), expected);
    assert_eq!(
        after19
            .0
            .iter()
            .filter(|object| object.2 != HOST_TABLE)
            .collect::<Vec<_>>(),
        before18.0.iter().collect::<Vec<_>>(),
    );
    assert_eq!(
        after19
            .1
            .iter()
            .filter(|(name, _)| name.as_str() != HOST_TABLE)
            .collect::<BTreeMap<_, _>>(),
        before18.1.iter().collect::<BTreeMap<_, _>>(),
    );
}
