//! Dormant Host19 schema preparation. Ordinary `Store::open` still stops at18.
//!
//! These explicit low-level helpers provide no startup wiring, private backup,
//! Host registration API, record/receipt integrity audit or execution authority.
//! Store19 activation and those contracts belong to the coordinated S2 slice.
use crate::storage::db_error;
use crate::{CoreError, Result};
use rusqlite::{Connection, Transaction, TransactionBehavior};

/// Exact B4e contract bytes, including the final newline (SHA-256 below).
pub const HOST_DDL: &str = "CREATE TABLE host_registrations(\n id TEXT PRIMARY KEY,\n name TEXT NOT NULL,\n name_key TEXT NOT NULL,\n revision INTEGER NOT NULL CHECK(revision=1),\n created_sequence INTEGER NOT NULL UNIQUE REFERENCES events(sequence),\n document TEXT NOT NULL\n);\nCREATE INDEX host_registrations_name_id ON host_registrations(name_key,id);\n";
pub const HOST_DDL_SHA256: &str =
    "a81ce46f0c66692b48ed887a389c7f7d30890840c7111c324b78503145a7815c";
pub const HOST_TABLE: &str = "host_registrations";
pub const HOST_EVENT_KIND: &str = "host.registered";
pub const HOST_IDEMPOTENCY_SCOPE: &str = "hosts.register:v1";

type Object = (String, String, String, Option<String>);

fn changed() -> CoreError {
    CoreError::new(
        "INPUT_CHANGED",
        "Host schema preparation refused inconsistent input",
    )
}

fn version(db: &Connection) -> Result<i64> {
    db.pragma_query_value(Some("main"), "user_version", |r| r.get(0))
        .map_err(db_error)
}

/// Schema-only reference, without constructing a Store, recovery or disk DB.
/// The original16/17/18 migrations and references remain untouched.
pub fn schema18_reference() -> Result<Connection> {
    let mut db = crate::storage::integration_schema16_reference()?;
    crate::integration::migrate(&mut db, 16)?;
    crate::conversations::migrate(&mut db, 17)?;
    Ok(db)
}

/// Known authority schema only; safe ordinary opaque user tables are not part
/// of this reference count. This is not a second persistent registry database.
pub fn schema19_reference() -> Result<Connection> {
    let mut db = schema18_reference()?;
    let tx = db.transaction().map_err(db_error)?;
    tx.execute_batch(HOST_DDL).map_err(db_error)?;
    tx.pragma_update(None, "user_version", 19)
        .map_err(db_error)?;
    tx.commit().map_err(db_error)?;
    Ok(db)
}

/// Explicit opt-in migration, deliberately not called by Store or IPC.
/// The caller is responsible for the future product backup/activation contract.
pub fn migrate(connection: &mut Connection) -> Result<()> {
    if !connection.is_autocommit() {
        return Err(changed());
    }
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_error)?;
    migrate_in_transaction(&tx)?;
    tx.commit().map_err(db_error)
}

/// Caller-owned transaction permits a later failure to roll back the complete
/// Host extension. Never commits or changes the caller's FK enforcement policy.
pub fn migrate_in_transaction(tx: &Transaction<'_>) -> Result<()> {
    if tx.is_autocommit()
        || !tx
            .pragma_query_value(None, "foreign_keys", |r| r.get::<_, bool>(0))
            .map_err(db_error)?
    {
        return Err(changed());
    }
    match version(tx)? {
        19 => validate_current19(tx),
        18 => {
            let reference = schema18_reference()?;
            compatible_schema(tx, &reference, 63)?;
            // A predeclared future authority object is not an opaque table.
            if objects(tx)?.iter().any(|o| {
                o.1.eq_ignore_ascii_case(HOST_TABLE)
                    || o.1.eq_ignore_ascii_case("host_registrations_name_id")
                    || o.2.eq_ignore_ascii_case(HOST_TABLE)
            }) {
                return Err(changed());
            }
            integrity(tx, &reference)?;
            tx.execute_batch(HOST_DDL).map_err(db_error)?;
            tx.pragma_update(None, "user_version", 19)
                .map_err(db_error)?;
            validate_current19(tx)
        }
        _ => Err(CoreError::new(
            "VERSION_UNSUPPORTED",
            "Host preparation requires exact schema18 or19",
        )),
    }
}

/// Read-only structural recognition. It intentionally does not implement B's
/// bounded Host record/event/receipt audit, which must precede product reads.
pub fn validate_current19(db: &Connection) -> Result<()> {
    if version(db)? != 19 {
        return Err(CoreError::new(
            "VERSION_UNSUPPORTED",
            "Expected exact Host schema19",
        ));
    }
    let reference = schema19_reference()?;
    compatible_schema(db, &reference, 64)?;
    integrity(db, &reference)
}

fn integrity(db: &Connection, reference: &Connection) -> Result<()> {
    // SQLite quick_check evaluates CHECK expressions. Limit it to the trusted
    // authority tables: a whole-main check would execute/re-resolve opaque UDFs.
    // Pinned SQLite supports the table argument for both quick/integrity_check.
    for (_, name, _, _) in objects(reference)?.iter().filter(|o| o.0 == "table") {
        let quoted = name.replace('"', "\"\"");
        let mut check = db
            .prepare(&format!("PRAGMA main.quick_check(\"{quoted}\")"))
            .map_err(db_error)?;
        let results = check
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_error)?;
        let mut count = 0;
        for result in results {
            if result.map_err(db_error)? != "ok" || count != 0 {
                return Err(changed());
            }
            count += 1;
        }
        if count != 1 {
            return Err(changed());
        }
    }
    // Compatible opaque tables have no FKs or generated columns. This checks
    // actual declared FK relationships without evaluating their CHECK payloads.
    if db
        .prepare("PRAGMA main.foreign_key_check")
        .map_err(db_error)?
        .exists([])
        .map_err(db_error)?
    {
        return Err(changed());
    }
    Ok(())
}

fn objects(db: &Connection) -> Result<Vec<Object>> {
    let (count, bytes): (i64, i64) = db.query_row(
        "SELECT count(*),coalesce(sum(octet_length(type)+octet_length(name)+octet_length(tbl_name)+coalesce(octet_length(sql),0)),0) FROM main.sqlite_schema",
        [], |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(db_error)?;
    if !(1..=256).contains(&count) || !(1..=1_048_576).contains(&bytes) {
        return Err(changed());
    }
    let mut statement = db
        .prepare("SELECT type,name,tbl_name,sql FROM main.sqlite_schema ORDER BY type,name")
        .map_err(db_error)?;
    let result = statement
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get::<_, Option<String>>(3)?.map(|s| normalize(&s)),
            ))
        })
        .map_err(db_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_error);
    result
}

// Preserve accepted Integration opaque compatibility; this is not whole-user-DB
// equality. Use direct PRAGMA metadata so an opaque pragma_* table cannot shadow
// a table-valued metadata function. No opaque rows or expressions are evaluated.
fn compatible_schema(db: &Connection, reference: &Connection, tables: usize) -> Result<()> {
    let known = objects(reference)?;
    if known.iter().filter(|o| o.0 == "table").count() != tables {
        return Err(changed());
    }
    let actual = objects(db)?;
    if !known.iter().all(|o| actual.contains(o)) {
        return Err(changed());
    }
    let extra: Vec<_> = actual.iter().filter(|o| !known.contains(o)).collect();
    let opaque: Vec<_> = extra
        .iter()
        .filter(|o| o.0 == "table")
        .map(|o| o.1.as_str())
        .collect();
    for (kind, name, table, sql) in extra {
        if name.to_ascii_lowercase().starts_with("integration")
            || known.iter().any(|o| o.1.eq_ignore_ascii_case(name))
        {
            return Err(changed());
        }
        match kind.as_str() {
            "table" => {
                if name != table
                    || name.to_ascii_lowercase().starts_with("sqlite_")
                    || sql.is_none()
                    || !ordinary_table(db, name)?
                {
                    return Err(changed());
                }
                let quoted = name.replace('"', "\"\"");
                if db
                    .prepare(&format!("PRAGMA main.foreign_key_list(\"{quoted}\")"))
                    .map_err(db_error)?
                    .exists([])
                    .map_err(db_error)?
                {
                    return Err(changed());
                }
                let mut columns = db
                    .prepare(&format!("PRAGMA main.table_xinfo(\"{quoted}\")"))
                    .map_err(db_error)?;
                for hidden in columns
                    .query_map([], |r| r.get::<_, i64>(6))
                    .map_err(db_error)?
                {
                    if hidden.map_err(db_error)? != 0 {
                        return Err(changed());
                    }
                }
            }
            "index" => {
                if !opaque.contains(&table.as_str())
                    || !ordinary_index(db, name, table, sql.is_some())?
                    || (name.to_ascii_lowercase().starts_with("sqlite_")
                        && !name.to_ascii_lowercase().starts_with("sqlite_autoindex_"))
                {
                    return Err(changed());
                }
            }
            _ => return Err(changed()),
        }
    }
    Ok(())
}

fn ordinary_table(db: &Connection, name: &str) -> Result<bool> {
    let mut statement = db.prepare("PRAGMA main.table_list").map_err(db_error)?;
    let mut rows = statement.query([]).map_err(db_error)?;
    while let Some(row) = rows.next().map_err(db_error)? {
        if row.get::<_, String>(0).map_err(db_error)? == "main"
            && row.get::<_, String>(1).map_err(db_error)? == name
        {
            return Ok(row.get::<_, String>(2).map_err(db_error)? == "table");
        }
    }
    Ok(false)
}

fn ordinary_index(db: &Connection, name: &str, table: &str, explicit: bool) -> Result<bool> {
    let quoted = table.replace('"', "\"\"");
    let mut list = db
        .prepare(&format!("PRAGMA main.index_list(\"{quoted}\")"))
        .map_err(db_error)?;
    let mut rows = list.query([]).map_err(db_error)?;
    let mut found = false;
    while let Some(row) = rows.next().map_err(db_error)? {
        if row.get::<_, String>(1).map_err(db_error)? == name {
            let origin: String = row.get(3).map_err(db_error)?;
            found = row.get::<_, i64>(4).map_err(db_error)? == 0
                && ((explicit && origin == "c")
                    || (!explicit && matches!(origin.as_str(), "u" | "pk")));
        }
    }
    if !found {
        return Ok(false);
    }
    let quoted = name.replace('"', "\"\"");
    let mut info = db
        .prepare(&format!("PRAGMA main.index_xinfo(\"{quoted}\")"))
        .map_err(db_error)?;
    for row in info
        .query_map([], |r| Ok((r.get::<_, i64>(1)?, r.get::<_, i64>(5)?)))
        .map_err(db_error)?
    {
        let (column, key) = row.map_err(db_error)?;
        if key == 1 && column < 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

fn normalize(sql: &str) -> String {
    let mut out = String::new();
    let mut quote = None;
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(q) = quote {
            out.push(c);
            if c == q {
                if chars.peek() == Some(&q) {
                    out.push(chars.next().expect("peeked character"));
                } else {
                    quote = None;
                }
            }
        } else if matches!(c, '\'' | '"' | '`') {
            quote = Some(c);
            out.push(c);
        } else if c.is_ascii_whitespace() {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }
    out.trim_end().to_owned()
}

#[cfg(test)]
#[path = "host_registry_migration_tests.rs"]
pub(crate) mod tests;
