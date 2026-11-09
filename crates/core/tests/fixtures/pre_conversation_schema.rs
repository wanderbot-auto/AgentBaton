//! Test-only, exact empty schema18 -> schema17 inverse. Never a historical producer.
use rusqlite::{Connection, Transaction, TransactionBehavior};

pub const CONVERSATION18_TABLES: [&str; 4] = [
    "conversations",
    "conversation_dispatches",
    "conversation_turns",
    "conversation_budgets",
];
type Object = (String, String, String, Option<String>);
const REFUSED: &str = "controlled empty Conversation18 inverse refused";
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
    CONVERSATION18_TABLES.contains(&object.2.as_str())
}
fn reference_objects() -> Result<Vec<Object>, String> {
    // Exact statements from the compiled checkout, not an inferred SQLite label.
    let source = include_str!("../../src/conversations.rs");
    let ddl = source
        .split_once("CREATE TABLE conversations(")
        .and_then(|(_, tail)| {
            tail.split_once("PRAGMA user_version=18;")
                .map(|(ddl, _)| format!("CREATE TABLE conversations({ddl}"))
        })
        .ok_or_else(|| REFUSED.to_owned())?;
    let reference = Connection::open_in_memory().map_err(|_| REFUSED.to_owned())?;
    reference
        .execute_batch(&ddl)
        .map_err(|_| REFUSED.to_owned())?;
    objects(&reference)
}
/// Read-only proof of precisely the new domain; older nonempty history is allowed.
pub fn verify_empty_conversation18(db: &Connection) -> Result<(), String> {
    let version: u32 = db
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| REFUSED.to_owned())?;
    if version != 18 {
        return Err(REFUSED.to_owned());
    }
    let all = objects(db)?;
    let actual: Vec<_> = all
        .iter()
        .filter(|object| is_domain(object))
        .cloned()
        .collect();
    if actual != reference_objects()? {
        return Err("changed Conversation18 DDL or index set".into());
    }
    for table in CONVERSATION18_TABLES {
        let count: i64 = db
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .map_err(|_| REFUSED.to_owned())?;
        if count != 0 {
            return Err("nonempty Conversation18 history".into());
        }
    }
    // Unknown domain extensions and incoming dependencies cannot be silently
    // left in a fake schema17. Unrelated user tables/rows/DDL remain untouched.
    for object in all.iter().filter(|object| !is_domain(object)) {
        let name = object.1.to_ascii_lowercase();
        if name.starts_with("conversation_") || name.starts_with("conversations") {
            return Err("unknown Conversation18 domain object".into());
        }
        if matches!(object.0.as_str(), "trigger" | "view")
            && object.3.as_ref().is_some_and(|sql| {
                let lower = sql.to_ascii_lowercase();
                CONVERSATION18_TABLES
                    .iter()
                    .any(|name| lower.contains(*name))
            })
        {
            return Err("external Conversation18 dependency".into());
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
                let target = target.map_err(|_| REFUSED.to_owned())?;
                if CONVERSATION18_TABLES
                    .iter()
                    .any(|name| target.eq_ignore_ascii_case(name))
                {
                    return Err("incoming Conversation18 foreign key".into());
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
/// Caller-owned variant permits an atomic 18 ->17 -> older synthetic fixture.
pub fn remove_empty_conversation18_in_transaction(
    transaction: &Transaction<'_>,
) -> Result<(), String> {
    if transaction.is_autocommit() {
        return Err(REFUSED.to_owned());
    }
    verify_empty_conversation18(transaction)?;
    let before: Vec<_> = objects(transaction)?
        .into_iter()
        .filter(|object| !is_domain(object))
        .collect();
    transaction
        .execute_batch(
            "DROP TABLE conversation_budgets;
DROP TABLE conversation_turns;
DROP TABLE conversation_dispatches;
DROP TABLE conversations;
PRAGMA user_version=17;",
        )
        .map_err(|_| REFUSED.to_owned())?;
    if objects(transaction)? != before
        || transaction
            .prepare("PRAGMA foreign_key_check")
            .map_err(|_| REFUSED.to_owned())?
            .exists([])
            .map_err(|_| REFUSED.to_owned())?
    {
        return Err("Conversation18 inverse changed retained schema or foreign keys".into());
    }
    Ok(())
}
/// Standalone strict inverse owns one transaction and never changes FK policy.
#[allow(dead_code)] // Some unit fixtures use only the caller-owned transaction variant.
pub fn remove_empty_conversation18(connection: &Connection) -> Result<(), String> {
    if !connection.is_autocommit() {
        return Err(REFUSED.to_owned());
    }
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)
        .map_err(|_| REFUSED.to_owned())?;
    remove_empty_conversation18_in_transaction(&transaction)?;
    transaction.commit().map_err(|_| REFUSED.to_owned())
}
