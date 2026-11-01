//! Integration domain persistence. No table/record alone creates branch authority.
use crate::{storage::db_error, CoreError, Result};
use rusqlite::{Connection, TransactionBehavior};

pub(crate) fn migrate(db: &mut Connection, _original_version: u32) -> Result<()> {
    let current: u32 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(db_error)?;
    if current == 17 {
        let reference = crate::storage::integration_schema16_reference()?;
        reference.execute_batch(DDL).map_err(db_error)?;
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if !compatible_schema(&tx, &reference, 59)? {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Integration current schema17 is incomplete or changed",
            ));
        }
        if tx
            .prepare("PRAGMA foreign_key_check")
            .map_err(db_error)?
            .exists([])
            .map_err(db_error)?
        {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Integration current schema17 foreign keys are inconsistent",
            ));
        }
        validate_predecessor_tasks(&tx)?;
        tx.commit().map_err(db_error)?;
        return Ok(());
    }
    if current != 16 {
        return Err(CoreError::new(
            "VERSION_UNSUPPORTED",
            "Integration migration requires the complete previous schema16",
        ));
    }
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_error)?;
    // A label and an empty FK check do not prove a complete predecessor.
    // Compare all original table/constraint/index/view/trigger definitions
    // before writing any new DDL; a failure leaves the source transaction inert.
    let reference = crate::storage::integration_schema16_reference()?;
    if !compatible_schema(&tx, &reference, 55)? {
        return Err(CoreError::new(
            "INPUT_CHANGED",
            "Integration migration refuses a changed or incomplete schema16 predecessor",
        ));
    }
    if tx
        .prepare("PRAGMA foreign_key_check")
        .map_err(db_error)?
        .exists([])
        .map_err(db_error)?
    {
        return Err(CoreError::new(
            "INPUT_CHANGED",
            "Integration migration refuses inconsistent predecessor foreign keys",
        ));
    }
    validate_predecessor_tasks(&tx)?;
    tx.execute_batch(DDL).map_err(db_error)?;
    // Extras consume the same fixed schema budget as known objects. Refuse
    // atomically if adding17 would exceed it and make the result unreopenable.
    schema_objects(&tx)?;
    tx.execute_batch("PRAGMA user_version=17;")
        .map_err(db_error)?;
    tx.commit().map_err(db_error)
}
const DDL: &str = r#"
CREATE TABLE integration_policies(id TEXT PRIMARY KEY,task_id TEXT NOT NULL REFERENCES tasks(id),spec_revision INTEGER NOT NULL CHECK(spec_revision>0),goal_scope_id TEXT NOT NULL REFERENCES goal_scopes(id),approval_id TEXT NOT NULL UNIQUE REFERENCES approval_proposals(id),revision INTEGER NOT NULL CHECK(revision>0),document TEXT NOT NULL);
CREATE INDEX integration_policies_task ON integration_policies(task_id,id);
CREATE TABLE integrations(id TEXT PRIMARY KEY,policy_id TEXT NOT NULL REFERENCES integration_policies(id),task_id TEXT NOT NULL REFERENCES tasks(id),spec_revision INTEGER NOT NULL CHECK(spec_revision>0),state TEXT NOT NULL CHECK(state IN('queued','capturing','preparing','validating','cas_prepared','confirmed_local_ref','blocked','stale_target','failed','cancel_requested','cancelled','outcome_unknown')),revision INTEGER NOT NULL CHECK(revision>0),document TEXT NOT NULL,UNIQUE(policy_id,task_id,spec_revision));
CREATE INDEX integrations_task ON integrations(task_id,id);
CREATE TABLE integration_attempts(attempt_id TEXT PRIMARY KEY REFERENCES attempts(id),integration_id TEXT NOT NULL REFERENCES integrations(id),round INTEGER NOT NULL CHECK(round BETWEEN 1 AND 8),kind TEXT NOT NULL CHECK(kind IN('preparation','validator','cas')),authority_kind TEXT NOT NULL CHECK(authority_kind='integration'),root_task_id TEXT NOT NULL REFERENCES tasks(id),root_grant_id TEXT NOT NULL REFERENCES execution_grants(id),project_id TEXT NOT NULL REFERENCES projects(id),state TEXT NOT NULL CHECK(state IN('prepared','running','cancel_requested','finished','outcome_unknown')),document TEXT NOT NULL);
CREATE INDEX integration_attempts_root ON integration_attempts(root_grant_id,project_id);
CREATE INDEX integration_attempts_run ON integration_attempts(integration_id,round,attempt_id);
CREATE TABLE integration_ref_fences(repository_digest TEXT NOT NULL,target_ref TEXT NOT NULL,integration_id TEXT NOT NULL REFERENCES integrations(id),revision INTEGER NOT NULL CHECK(revision>0),state TEXT NOT NULL CHECK(state IN('held','unknown','released')),document TEXT NOT NULL,PRIMARY KEY(repository_digest,target_ref));
"#;

impl crate::storage::Store {
    /// Strict read-only domain counts. Missing/malformed schema is an error,
    /// never a fabricated zero; FactoryRecordCounts remains unchanged.
    pub fn integration_record_counts(&self) -> Result<super::IntegrationRecordCounts> {
        let counts: [i64; 4] = self.conn.query_row("SELECT (SELECT count(*) FROM integration_policies),(SELECT count(*) FROM integrations),(SELECT count(*) FROM integration_attempts),(SELECT count(*) FROM integration_ref_fences)",[],|r|Ok([r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?])).map_err(db_error)?;
        let convert = |value| u64::try_from(value).map_err(|_| super::authority::changed());
        Ok(super::IntegrationRecordCounts {
            policies: convert(counts[0])?,
            runs: convert(counts[1])?,
            attempts: convert(counts[2])?,
            ref_fences: convert(counts[3])?,
        })
    }
}

type SchemaObject = (String, String, String, Option<String>);

fn schema_objects(db: &Connection) -> Result<Vec<SchemaObject>> {
    let (count, bytes):(i64,i64)=db.query_row("SELECT count(*),COALESCE(sum(octet_length(type)+octet_length(name)+octet_length(tbl_name)+COALESCE(octet_length(sql),0)),0) FROM sqlite_schema",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
    if !(1..=256).contains(&count) || !(1..=1048576).contains(&bytes) {
        return Err(CoreError::new(
            "INPUT_CHANGED",
            "Predecessor schema exceeds the fixed definition bound",
        ));
    }
    let mut statement = db
        .prepare("SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name")
        .map_err(db_error)?;
    let rows = statement
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    Ok(rows
        .into_iter()
        .map(|(kind, name, table, sql)| (kind, name, table, sql.map(|s| normalize_ddl(&s))))
        .collect())
}
// The reference defines the entire authority-bearing schema, not the entire
// historical user database. Unrecognized objects never acquire domain meaning.
fn compatible_schema(
    db: &Connection,
    reference: &Connection,
    expected_tables: usize,
) -> Result<bool> {
    let known = schema_objects(reference)?;
    if known.iter().filter(|row| row.0 == "table").count() != expected_tables {
        return Err(CoreError::new(
            "INPUT_CHANGED",
            "Integration trusted reference has an incomplete table set",
        ));
    }
    let actual = schema_objects(db)?;
    // Includes every original constraint, implicit/explicit index and any
    // reference trigger/view. Extra objects cannot replace a missing definition.
    if !known.iter().all(|row| actual.contains(row)) {
        return Ok(false);
    }
    let extras: Vec<_> = actual.iter().filter(|row| !known.contains(row)).collect();
    let opaque_tables: Vec<_> = extras
        .iter()
        .filter(|row| row.0 == "table")
        .map(|row| row.1.as_str())
        .collect();
    for row in &extras {
        let (kind, name, table, sql) = row;
        // SQLite identifiers compare ASCII case-insensitively. Also reserve
        // future Integration names so an opaque object cannot predeclare them.
        if name.to_ascii_lowercase().starts_with("integration")
            || known.iter().any(|known| known.1.eq_ignore_ascii_case(name))
        {
            return Ok(false);
        }
        match kind.as_str() {
            "table" => {
                if name != table
                    || name.to_ascii_lowercase().starts_with("sqlite_")
                    || sql.is_none()
                {
                    return Ok(false);
                }
                // Metadata is inspected without selecting/evaluating opaque
                // rows or interpolating their identifiers as executable SQL.
                let ordinary = ordinary_main_table(db, name)?;
                let dependencies: bool = db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_list(?1,'main')) OR EXISTS(SELECT 1 FROM pragma_table_xinfo(?1,'main') WHERE hidden<>0)",
                    [name], |r| r.get(0),
                ).map_err(db_error)?;
                // No FK, even opaque-to-opaque: deliberately conservative,
                // bounded preservation, not a second schema migration system.
                if !ordinary || dependencies {
                    return Ok(false);
                }
            }
            "index" => {
                if !opaque_tables.contains(&table.as_str()) {
                    return Ok(false);
                }
                let safe: bool = db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM pragma_index_list(?1,'main') WHERE name=?2 AND partial=0 AND ((origin='c' AND ?3) OR (origin IN ('u','pk') AND NOT ?3))) AND NOT EXISTS(SELECT 1 FROM pragma_index_xinfo(?2,'main') WHERE key=1 AND cid<0)",
                    rusqlite::params![table, name, sql.is_some()], |r| r.get(0),
                ).map_err(db_error)?;
                // Only ordinary column indexes owned by opaque tables; no
                // expression/partial indexes or indexes attached to authority.
                if !safe
                    || (name.to_ascii_lowercase().starts_with("sqlite_")
                        && !name.to_ascii_lowercase().starts_with("sqlite_autoindex_"))
                {
                    return Ok(false);
                }
            }
            // Unknown triggers/views/virtual or shadow objects could execute or
            // expose authority-domain behavior and are never opaque data.
            _ => return Ok(false),
        }
    }
    Ok(true)
}

// Direct PRAGMA syntax cannot resolve to an opaque ordinary table named
// pragma_table_list. Never let untrusted legacy rows supply schema metadata.
fn ordinary_main_table(db: &Connection, name: &str) -> Result<bool> {
    let mut statement = db.prepare("PRAGMA main.table_list").map_err(db_error)?;
    let mut rows = statement.query([]).map_err(db_error)?;
    while let Some(row) = rows.next().map_err(db_error)? {
        let schema: String = row.get(0).map_err(db_error)?;
        let table: String = row.get(1).map_err(db_error)?;
        let kind: String = row.get(2).map_err(db_error)?;
        if schema == "main" && table == name {
            return Ok(kind == "table");
        }
    }
    Ok(false)
}

fn normalize_ddl(sql: &str) -> String {
    // Ignore formatting only outside quoted SQL content. Literal values and
    // named constraints/index predicates remain byte-exact.
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
            // Keep a token separator. Deleting it could equate a NOT NULL
            // constraint with the different legal type name TEXTNOTNULL.
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }
    out.trim_end().to_owned()
}

fn validate_predecessor_tasks(db: &Connection) -> Result<()> {
    let(count,bytes,max):(i64,i64,i64)=db.query_row("SELECT count(*),COALESCE(sum(octet_length(id)+octet_length(document)),0),COALESCE(max(octet_length(document)),0) FROM tasks",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(db_error)?;
    let bad = || {
        CoreError::new(
            "INPUT_CHANGED",
            "Integration migration refuses malformed predecessor Task records",
        )
    };
    if !(0..=100_000).contains(&count)
        || !(0..=4_194_304).contains(&bytes)
        || !(0..=262_144).contains(&max)
    {
        return Err(bad());
    }
    let mut statement = db
        .prepare("SELECT id,document FROM tasks ORDER BY id")
        .map_err(db_error)?;
    let mut rows = statement.query([]).map_err(db_error)?;
    while let Some(row) = rows.next().map_err(db_error)? {
        let id: String = row.get(0).map_err(db_error)?;
        let text: String = row.get(1).map_err(db_error)?;
        let value: serde_json::Value = serde_json::from_str(&text).map_err(|_| bad())?;
        if !value.is_object() {
            return Err(bad());
        }
        // Decode the original text again: Value is only the object-shape check
        // and would otherwise discard duplicate keys before typed validation.
        let task: crate::storage::Task = serde_json::from_str(&text).map_err(|_| bad())?;
        if task.id != id || task.revision == 0 || task.spec_revision == 0 {
            return Err(bad());
        }
        if let Some(binding) = task.code_workspace_binding {
            if binding.receipt.task_id != id
                || binding.receipt.project_id != task.project_id
                || binding.receipt.spec_revision != task.spec_revision
                || binding.receipt.version != 1
                || binding.revision == 0
            {
                return Err(bad());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod schema_tokens {
    use super::normalize_ddl;
    #[test]
    fn ddl_normalization_preserves_constraint_token_boundaries() {
        assert_ne!(
            normalize_ddl("CREATE TABLE tasks(id TEXT PRIMARY KEY, document TEXT NOT NULL)"),
            normalize_ddl("CREATE TABLE tasks(id TEXT PRIMARY KEY, document TEXTNOTNULL)")
        );
        assert_eq!(
            normalize_ddl("CREATE  TABLE\ntasks(id TEXT PRIMARY KEY, document TEXT NOT NULL)"),
            normalize_ddl("CREATE TABLE tasks(id TEXT PRIMARY KEY, document TEXT NOT NULL)")
        );
    }
    #[test]
    fn quoted_ddl_bytes_are_never_reinterpreted_as_formatting() {
        assert_ne!(
            normalize_ddl("CREATE TABLE t(v TEXT CHECK(v='a b'))"),
            normalize_ddl("CREATE TABLE t(v TEXT CHECK(v='ab'))")
        );
        assert_ne!(
            normalize_ddl("CREATE TABLE t(\"a b\" TEXT)"),
            normalize_ddl("CREATE TABLE t(\"ab\" TEXT)")
        );
    }
}
