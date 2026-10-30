//! Test-only, exact empty schema17 -> schema16 inverse. Never a historical producer.
use rusqlite::{Connection, Transaction, TransactionBehavior};

pub const INTEGRATION17_TABLES: [&str; 4] = [
    "integration_policies",
    "integrations",
    "integration_attempts",
    "integration_ref_fences",
];
type Object = (String, String, String, Option<String>);
const REFUSED: &str = "controlled empty Integration17 inverse refused";
fn objects(db: &Connection) -> Result<Vec<Object>, String> {
    let mut statement = db
        .prepare("SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name")
        .map_err(|_| REFUSED.to_owned())?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .map_err(|_| REFUSED.to_owned())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| REFUSED.to_owned());
    rows
}
fn is_domain(object: &Object) -> bool {
    INTEGRATION17_TABLES.contains(&object.2.as_str())
}
fn reference_objects() -> Result<Vec<Object>, String> {
    // Exact statements from the compiled checkout, not an inferred SQLite label.
    let source = include_str!("../../src/integration_store.rs");
    let ddl = source
        .split_once("const DDL: &str = r#\"")
        .and_then(|(_, tail)| tail.split_once("\"#;").map(|(ddl, _)| ddl))
        .ok_or_else(|| REFUSED.to_owned())?;
    let reference = Connection::open_in_memory().map_err(|_| REFUSED.to_owned())?;
    reference
        .execute_batch(ddl)
        .map_err(|_| REFUSED.to_owned())?;
    objects(&reference)
}
/// Read-only proof of precisely the new domain; older nonempty history is allowed.
pub fn verify_empty_integration17(db: &Connection) -> Result<(), String> {
    let version: u32 = db
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| REFUSED.to_owned())?;
    if version != 17 {
        return Err(REFUSED.to_owned());
    }
    let all = objects(db)?;
    let actual: Vec<_> = all
        .iter()
        .filter(|object| is_domain(object))
        .cloned()
        .collect();
    if actual != reference_objects()? {
        return Err("changed Integration17 DDL or index set".into());
    }
    for table in INTEGRATION17_TABLES {
        let count: i64 = db
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .map_err(|_| REFUSED.to_owned())?;
        if count != 0 {
            return Err("nonempty Integration17 history".into());
        }
    }
    // Unknown domain extensions and incoming dependencies cannot be silently
    // left in a fake schema16. Unrelated user tables/rows/DDL remain untouched.
    for object in all.iter().filter(|object| !is_domain(object)) {
        if object.1.starts_with("integration_") || object.1.starts_with("integrations") {
            return Err("unknown Integration17 domain object".into());
        }
        if matches!(object.0.as_str(), "trigger" | "view")
            && object.3.as_ref().is_some_and(|sql| {
                let lower = sql.to_ascii_lowercase();
                INTEGRATION17_TABLES
                    .iter()
                    .any(|name| lower.contains(*name))
            })
        {
            return Err("external Integration17 dependency".into());
        }
        if object.0 == "table" {
            let quoted = object.1.replace('"', "\"\"");
            let mut statement = db
                .prepare(&format!("PRAGMA foreign_key_list(\"{quoted}\")"))
                .map_err(|_| REFUSED.to_owned())?;
            for target in statement
                .query_map([], |row| row.get::<_, String>(2))
                .map_err(|_| REFUSED.to_owned())?
            {
                if INTEGRATION17_TABLES.contains(&target.map_err(|_| REFUSED.to_owned())?.as_str())
                {
                    return Err("incoming Integration17 foreign key".into());
                }
            }
        }
    }
    if db
        .prepare("PRAGMA foreign_key_check")
        .map_err(|_| REFUSED.to_owned())?
        .exists([])
        .map_err(|_| REFUSED.to_owned())?
    {
        return Err("inconsistent predecessor foreign keys".into());
    }
    Ok(())
}
/// Caller-owned variant permits an atomic 17 ->16 -> older synthetic fixture.
pub fn remove_empty_integration17_in_transaction(
    transaction: &Transaction<'_>,
) -> Result<(), String> {
    if transaction.is_autocommit() {
        return Err(REFUSED.to_owned());
    }
    verify_empty_integration17(transaction)?;
    let before: Vec<_> = objects(transaction)?
        .into_iter()
        .filter(|object| !is_domain(object))
        .collect();
    transaction
        .execute_batch(
            "DROP TABLE integration_ref_fences;
DROP TABLE integration_attempts;
DROP TABLE integrations;
DROP TABLE integration_policies;
PRAGMA user_version=16;",
        )
        .map_err(|_| REFUSED.to_owned())?;
    if objects(transaction)? != before
        || transaction
            .prepare("PRAGMA foreign_key_check")
            .map_err(|_| REFUSED.to_owned())?
            .exists([])
            .map_err(|_| REFUSED.to_owned())?
    {
        return Err("Integration17 inverse changed retained schema or foreign keys".into());
    }
    Ok(())
}
/// Standalone strict inverse owns one transaction and never changes FK policy.
pub fn remove_empty_integration17(connection: &Connection) -> Result<(), String> {
    if !connection.is_autocommit() {
        return Err(REFUSED.to_owned());
    }
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)
        .map_err(|_| REFUSED.to_owned())?;
    remove_empty_integration17_in_transaction(&transaction)?;
    transaction.commit().map_err(|_| REFUSED.to_owned())
}
