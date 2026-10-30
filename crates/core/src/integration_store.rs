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
