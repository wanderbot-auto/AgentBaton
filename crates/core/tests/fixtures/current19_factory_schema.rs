//! TEST-ONLY exact19 ->18 ->17 ->16 ->14 ->12, one caller transaction.
#![cfg(test)]
use super::{pre_factory_schema, pre_host_schema};
use rusqlite::{Connection, Transaction, TransactionBehavior};

const ORIGINAL: &str = include_str!("pre_factory_schema.rs");
const WHOLE_SHA: &str = "407a1a9c5a5fdb191aa24c8875eb4ee49416306e0c4c928948cd6f6024003a65";
const TAIL_SHA: &str = "abab588855690126e25979627d0925a2c346039bb73a572fc86f15269a52d265";
const SQL_SHA: &str = "eef180069d233de9904e940fe82561cdbfed27028d1f5d279a18af3ee3d6c3c3";

/// Fixed accepted-source pins, never runtime Git/candidate-generated acceptance.
/// Reuse the exact old SQL slice rather than copy a stale old helper body.
pub fn pinned_factory_sql(source: &str) -> rusqlite::Result<String> {
    let sha = agentbaton_workspaces::validation::sha256;
    if sha(source.as_bytes()) != WHOLE_SHA {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let tail = source
        .lines()
        .skip(256)
        .take(20)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    if sha(tail.as_bytes()) != TAIL_SHA {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let sql = tail
        .split_once("            \"")
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(sql, _)| sql)
        .ok_or(rusqlite::Error::InvalidQuery)?;
    if sha(sql.as_bytes()) != SQL_SHA {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(sql.to_owned())
}

#[derive(Debug)]
pub struct InverseFailure {
    pub operation: Option<rusqlite::Error>,
    pub restore: Option<rusqlite::Error>,
    /// An observed commit success only; false is not proof of generic rollback.
    pub commit_observed: bool,
}

fn operation(error: rusqlite::Error) -> InverseFailure {
    InverseFailure {
        operation: Some(error),
        restore: None,
        commit_observed: false,
    }
}

/// Child-plan callers may append their existing explicit13 shape in THIS txn.
/// This helper itself stops at14; full Factory terminal tail jumps14 ->12.
pub fn remove_empty_current19_child_goal_in_transaction(
    tx: &Transaction<'_>,
) -> rusqlite::Result<()> {
    pre_host_schema::remove_empty_host19_in_transaction(tx)
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    pre_factory_schema::remove_empty_current_child_goal_increment(tx)
}

pub fn with_empty_current19_factory(
    connection: &Connection,
    caller: impl FnOnce(&Transaction<'_>) -> rusqlite::Result<()>,
) -> Result<(), InverseFailure> {
    if !connection.is_autocommit() {
        return Err(operation(rusqlite::Error::InvalidQuery));
    }
    let captured: bool = connection
        .pragma_query_value(None, "foreign_keys", |r| r.get(0))
        .map_err(operation)?;
    // From this point every captured-policy exit attempts restoration.
    let result = (|| {
        connection.pragma_update(None, "foreign_keys", false)?;
        let tx = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
        remove_empty_current19_child_goal_in_transaction(&tx)?;
        tx.execute_batch(&pinned_factory_sql(ORIGINAL)?)?;
        caller(&tx)?;
        if tx.prepare("PRAGMA foreign_key_check")?.exists([])? {
            return Err(rusqlite::Error::InvalidQuery);
        }
        tx.commit()
    })();
    let commit_observed = result.is_ok();
    let restored = connection.pragma_update(None, "foreign_keys", captured);
    match (result, restored) {
        (Ok(()), Ok(())) => Ok(()),
        (operation, restore) => Err(InverseFailure {
            operation: operation.err(),
            restore: restore.err(),
            commit_observed,
        }),
    }
}
