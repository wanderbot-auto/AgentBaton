//! TEST-ONLY exact empty19 ->18 inverse. Never import from a production module.
//! The including test module binds Core's Event and dormant migration module.
#![cfg(test)]
use super::{host_registry_migration, Event};
use rusqlite::{Connection, Transaction, TransactionBehavior};

type Object = (String, String, String, Option<String>);
const REFUSED: &str = "controlled empty Host19 inverse refused";
const MAX_ROWS: i64 = 10_000;
const MAX_BYTES: i64 = 4_194_304;
const MAX_RECORD: i64 = 262_144;

fn refused<T>(_: T) -> String {
    REFUSED.into()
}

fn objects(db: &Connection) -> Result<Vec<Object>, String> {
    let (count, bytes): (i64,i64) = db.query_row(
        "SELECT count(*),coalesce(sum(octet_length(type)+octet_length(name)+octet_length(tbl_name)+coalesce(octet_length(sql),0)),0) FROM main.sqlite_schema", [],
        |r| Ok((r.get(0)?,r.get(1)?)),
    ).map_err(refused)?;
    if !(1..=256).contains(&count) || !(1..=1_048_576).contains(&bytes) {
        return Err(REFUSED.into());
    }
    let mut s = db
        .prepare("SELECT type,name,tbl_name,sql FROM main.sqlite_schema ORDER BY type,name")
        .map_err(refused)?;
    let result = s
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .map_err(refused)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(refused);
    result
}

fn empty_journal_namespace(db: &Connection) -> Result<(), String> {
    // Validate the complete a4 outer envelope, including duplicate known fields,
    // before classification. Unrelated payloads are retained, not interpreted.
    let (count,bytes,max): (i64,i64,i64) = db.query_row(
        "SELECT count(*),coalesce(sum(octet_length(document)),0),coalesce(max(octet_length(document)),0) FROM events", [],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).map_err(refused)?;
    if count > MAX_ROWS || bytes > MAX_BYTES || max > MAX_RECORD {
        return Err(REFUSED.into());
    }
    let mut s = db
        .prepare("SELECT document FROM events ORDER BY sequence")
        .map_err(refused)?;
    for raw in s
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(refused)?
    {
        let raw = raw.map_err(refused)?;
        let event: Event = serde_json::from_str(&raw).map_err(refused)?;
        let value: serde_json::Value = serde_json::from_str(&raw).map_err(refused)?;
        let fields = value.as_object().ok_or_else(|| REFUSED.to_owned())?;
        let expected = [
            "sequence",
            "event_id",
            "aggregate_id",
            "revision",
            "kind",
            "occurred_at",
            "payload",
            "source",
        ];
        if fields.len() != expected.len() || expected.iter().any(|name| !fields.contains_key(*name))
        {
            return Err(REFUSED.into());
        }
        if event.kind.is_empty() || event.kind.to_ascii_lowercase().starts_with("host") {
            return Err(REFUSED.into());
        }
    }
    let (count,bytes,max): (i64,i64,i64) = db.query_row(
        "SELECT count(*),coalesce(sum(octet_length(scope)),0),coalesce(max(octet_length(scope)),0) FROM idempotency", [],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).map_err(refused)?;
    if count > MAX_ROWS || bytes > MAX_BYTES || max > MAX_RECORD {
        return Err(REFUSED.into());
    }
    let mut s = db
        .prepare("SELECT scope FROM idempotency")
        .map_err(refused)?;
    for scope in s
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(refused)?
    {
        if scope
            .map_err(refused)?
            .to_ascii_lowercase()
            .starts_with("host")
        {
            return Err(REFUSED.into());
        }
    }
    Ok(())
}

/// A deliberately strict fixture contract: the entire schema is the exact known
/// reference, not a real user database with arbitrary compatible opaque tables.
/// No temporary/attached extension, unknown dependency or malformed namespace
/// may be silently left behind under a historical version label.
pub fn verify_empty_host19(db: &Connection) -> Result<(), String> {
    let version: i64 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(refused)?;
    if version != 19 {
        return Err(REFUSED.into());
    }
    let mut databases = db.prepare("PRAGMA database_list").map_err(refused)?;
    for name in databases
        .query_map([], |r| r.get::<_, String>(1))
        .map_err(refused)?
    {
        if !matches!(name.map_err(refused)?.as_str(), "main" | "temp") {
            return Err(REFUSED.into());
        }
    }
    let temp: i64 = db
        .query_row("SELECT count(*) FROM temp.sqlite_schema", [], |r| r.get(0))
        .map_err(refused)?;
    if temp != 0 {
        return Err(REFUSED.into());
    }
    let reference = host_registry_migration::schema19_reference().map_err(refused)?;
    if objects(db)? != objects(&reference)? {
        return Err(REFUSED.into());
    }
    let hosts: i64 = db
        .query_row("SELECT count(*) FROM host_registrations", [], |r| r.get(0))
        .map_err(refused)?;
    if hosts != 0 {
        return Err(REFUSED.into());
    }
    let expected_fk: Vec<(String, String, String, String, String)> = {
        let mut s = reference
            .prepare("PRAGMA main.foreign_key_list(host_registrations)")
            .map_err(refused)?;
        let v = s
            .query_map([], |r| {
                Ok((r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
            })
            .map_err(refused)?
            .collect::<rusqlite::Result<_>>()
            .map_err(refused)?;
        v
    };
    let actual_fk: Vec<(String, String, String, String, String)> = {
        let mut s = db
            .prepare("PRAGMA main.foreign_key_list(host_registrations)")
            .map_err(refused)?;
        let v = s
            .query_map([], |r| {
                Ok((r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
            })
            .map_err(refused)?
            .collect::<rusqlite::Result<_>>()
            .map_err(refused)?;
        v
    };
    if actual_fk != expected_fk {
        return Err(REFUSED.into());
    }
    empty_journal_namespace(db)?;
    let quick: String = db
        .query_row("PRAGMA main.quick_check", [], |r| r.get(0))
        .map_err(refused)?;
    if quick != "ok"
        || db
            .prepare("PRAGMA main.foreign_key_check")
            .map_err(refused)?
            .exists([])
            .map_err(refused)?
    {
        return Err(REFUSED.into());
    }
    Ok(())
}

/// No BEGIN/COMMIT/FK-policy toggle: all later caller work shares this rollback.
pub fn remove_empty_host19_in_transaction(tx: &Transaction<'_>) -> Result<(), String> {
    if tx.is_autocommit() {
        return Err(REFUSED.into());
    }
    verify_empty_host19(tx)?;
    let retained: Vec<_> = objects(tx)?
        .into_iter()
        .filter(|o| o.2 != host_registry_migration::HOST_TABLE)
        .collect();
    tx.execute_batch("DROP INDEX host_registrations_name_id; DROP TABLE host_registrations; PRAGMA user_version=18;").map_err(refused)?;
    if objects(tx)? != retained || db_foreign_keys_bad(tx)? {
        return Err(REFUSED.into());
    }
    Ok(())
}

fn db_foreign_keys_bad(db: &Connection) -> Result<bool, String> {
    db.prepare("PRAGMA main.foreign_key_check")
        .map_err(refused)?
        .exists([])
        .map_err(refused)
}

pub fn remove_empty_host19(db: &Connection) -> Result<(), String> {
    if !db.is_autocommit() {
        return Err(REFUSED.into());
    }
    let tx = Transaction::new_unchecked(db, TransactionBehavior::Immediate).map_err(refused)?;
    remove_empty_host19_in_transaction(&tx)?;
    tx.commit().map_err(refused)
}
