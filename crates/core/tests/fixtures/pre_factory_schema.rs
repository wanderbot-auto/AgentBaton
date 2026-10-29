//! Test-fixture-only inverse of the empty schema14 Factory increments.
//! Never use this on a live runtime or a database with consumed reservations.
use rusqlite::Connection;

pub const FACTORY_TABLES: [&str; 11] = [
    "factory_child_plan_heads",
    "factory_child_plan_revisions",
    "factory_scoped_handles",
    "factory_handoffs",
    "factory_attempts",
    "factory_budget_reservations",
    "factory_delegations",
    "factory_task_bindings",
    "factory_task_lineage",
    "factory_runs",
    "factory_scopes",
];

pub fn remove_empty_factory_increment(connection: &Connection) -> rusqlite::Result<()> {
    let transaction = connection.unchecked_transaction()?;
    let version: u32 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version != 14 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    // Check EVERY named table before any destructive statement. A missing table
    // also refuses; do not broaden this to IF EXISTS or delete unknown objects.
    for table in FACTORY_TABLES {
        let rows: i64 =
            transaction.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })?;
        if rows != 0 {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    // Child-before-parent order also works with foreign_keys=ON.
    transaction.execute_batch(
        "DROP TABLE factory_child_plan_heads;
         DROP TABLE factory_child_plan_revisions;
         DROP TABLE factory_scoped_handles;
         DROP TABLE factory_handoffs;
         DROP TABLE factory_attempts;
         DROP TABLE factory_budget_reservations;
         DROP TABLE factory_delegations;
         DROP TABLE factory_task_bindings;
         DROP TABLE factory_task_lineage;
         DROP TABLE factory_runs;
         DROP TABLE factory_scopes;
         PRAGMA user_version=12;",
    )?;
    transaction.commit()
}
