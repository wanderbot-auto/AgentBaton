//! Integration domain persistence. No table/record alone creates branch authority.
use crate::{storage::db_error, CoreError, Result};
use rusqlite::{Connection, TransactionBehavior};

pub(crate) fn migrate(db: &mut Connection, _original_version: u32) -> Result<()> {
    let current: u32 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(db_error)?;
    if current == 17 {
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
    if predecessor_schema(&tx)? != predecessor_schema(&reference)? {
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

fn predecessor_schema(db: &Connection) -> Result<Vec<(String, String, String, Option<String>)>> {
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
    if rows.iter().filter(|r| r.0 == "table").count() != 55 {
        return Err(CoreError::new(
            "INPUT_CHANGED",
            "Integration predecessor must contain all 55 schema16 tables including sqlite_sequence",
        ));
    }
    Ok(rows
        .into_iter()
        .map(|(kind, name, table, sql)| (kind, name, table, sql.map(|s| normalize_ddl(&s))))
        .collect())
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
