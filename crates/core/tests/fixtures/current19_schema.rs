//! TEST-ONLY exact19 ->18 ->17 ->16 composition for activated Core19 fixtures.
//! Factory composition has its separate transaction wrapper; old helpers untouched.
#![cfg(test)]
use super::pre_host_schema;
use rusqlite::{Connection, Transaction, TransactionBehavior};

#[path = "pre_conversation_schema.rs"]
pub mod pre_conversation_schema;
#[path = "pre_integration_schema.rs"]
pub mod pre_integration_schema;

/// Pure reads, never a temporary PRAGMA downgrade to reuse an old validator.
pub fn verify_empty_current19(db: &Connection) -> Result<(), String> {
    pre_host_schema::verify_empty_host19(db)?;
    for table in pre_conversation_schema::CONVERSATION18_TABLES
        .into_iter()
        .chain(pre_integration_schema::INTEGRATION17_TABLES)
    {
        let count: i64 = db
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .map_err(|_| "current19 domain read failed".to_owned())?;
        if count != 0 {
            return Err("nonempty current19 domain".into());
        }
    }
    Ok(())
}

pub fn remove_empty_current19_in_transaction(tx: &Transaction<'_>) -> Result<(), String> {
    verify_empty_current19(tx)?;
    pre_host_schema::remove_empty_host19_in_transaction(tx)?;
    pre_conversation_schema::remove_empty_conversation18_in_transaction(tx)?;
    pre_integration_schema::remove_empty_integration17_in_transaction(tx)
}

pub fn remove_empty_current19(db: &Connection) -> Result<(), String> {
    if !db.is_autocommit() {
        return Err("current19 inverse requires autocommit".into());
    }
    let tx = Transaction::new_unchecked(db, TransactionBehavior::Immediate)
        .map_err(|_| "current19 transaction failed".to_owned())?;
    remove_empty_current19_in_transaction(&tx)?;
    tx.commit()
        .map_err(|_| "current19 commit failed".to_owned())
}
