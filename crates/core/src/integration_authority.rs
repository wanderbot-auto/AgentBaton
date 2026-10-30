//! Database-derived static material, never deserialized request authority.
use super::*;
use crate::{
    policy::ExecutionGrant,
    registry::{ManagedWorkspace, Project},
    storage::{db_error, decode, Task},
    CoreError, Result,
};
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub(super) fn invalid() -> CoreError {
    CoreError::new("INVALID_REQUEST","Select bounded exact integration records; authority flags and caller paths are not accepted")
}
pub(super) fn changed() -> CoreError {
    CoreError::new(
        "INPUT_CHANGED",
        "Integration Task, scope, acceptance, repository or approved definition changed",
    )
}
pub(super) fn denied() -> CoreError {
    CoreError::new(
        "AUTH_REQUIRED",
        "A separate live native integration permission and authentic current receipts are required",
    )
}
pub(super) fn limited() -> CoreError {
    CoreError::new(
        "RESOURCE_LIMIT",
        "Integration record or shared resource bound exceeded; no history was truncated",
    )
}
pub(super) fn id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
}
pub(super) fn key(s: &str) -> Result<()> {
    if id(s) {
        Ok(())
    } else {
        Err(invalid())
    }
}
pub(super) fn sql_num(n: u64) -> Result<i64> {
    i64::try_from(n).map_err(|_| limited())
}
pub(super) fn ms() -> Result<i64> {
    i64::try_from(time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
        .map_err(|_| invalid())
}
pub(super) fn sha<T: Serialize>(v: &T) -> Result<String> {
    let value = serde_json::to_value(v).map_err(|_| invalid())?;
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&value).map_err(|_| invalid())?)
    ))
}
pub(super) fn raw<T: for<'a> serde::Deserialize<'a>>(
    db: &Connection,
    table: &str,
    id: &str,
) -> Result<T> {
    if !self::id(id) {
        return Err(invalid());
    }
    let query=match table {
        "integration_policies"=>"SELECT octet_length(document),CASE WHEN octet_length(document)<=1048576 THEN document END FROM integration_policies WHERE id=?1",
        "integrations"=>"SELECT octet_length(document),CASE WHEN octet_length(document)<=1048576 THEN document END FROM integrations WHERE id=?1",
        "integration_attempts"=>"SELECT octet_length(document),CASE WHEN octet_length(document)<=1048576 THEN document END FROM integration_attempts WHERE attempt_id=?1",
        "dispatch_intents"=>"SELECT octet_length(document),CASE WHEN octet_length(document)<=1048576 THEN document END FROM dispatch_intents WHERE id=?1",
        "tasks"=>"SELECT octet_length(document),CASE WHEN octet_length(document)<=1048576 THEN document END FROM tasks WHERE id=?1",
        "goal_scopes"=>"SELECT octet_length(document),CASE WHEN octet_length(document)<=1048576 THEN document END FROM goal_scopes WHERE id=?1",
        "managed_workspaces"=>"SELECT octet_length(document),CASE WHEN octet_length(document)<=1048576 THEN document END FROM managed_workspaces WHERE id=?1",
        "projects"=>"SELECT octet_length(document),CASE WHEN octet_length(document)<=1048576 THEN document END FROM projects WHERE id=?1",
        "approval_proposals"=>"SELECT octet_length(document),CASE WHEN octet_length(document)<=1048576 THEN document END FROM approval_proposals WHERE id=?1",
        _=>return Err(invalid()),
    };
    let row: Option<(i64, Option<String>)> = db
        .query_row(query, [id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()
        .map_err(db_error)?;
    let (size, text) = row.ok_or_else(|| {
        CoreError::new("NOT_FOUND", "Integration referenced record does not exist")
    })?;
    if !(1..=MAX_INTEGRATION_BYTES as i64).contains(&size) {
        return Err(limited());
    }
    let text = text.ok_or_else(limited)?;
    let shape: serde_json::Value = serde_json::from_str(&text).map_err(|_| changed())?;
    if !shape.is_object() {
        return Err(changed());
    }
    // Always decode the original text, retaining duplicate-key rejection.
    decode(&text)
}
#[derive(Clone, Serialize)]
pub(crate) struct IntegrationScopeMaterial {
    pub(crate) task: Task,
    pub(crate) project: Project,
    pub(crate) workspace: ManagedWorkspace,
    pub(crate) grant: ExecutionGrant,
    pub(crate) grant_lineage: Vec<ExecutionGrant>,
    pub(crate) root_task_id: String,
    pub(crate) factory_scope_id: Option<String>,
    pub(crate) ancestors: Vec<String>,
    pub(crate) scope_id: String,
    pub(crate) scope_revision: u64,
    pub(crate) scope_digest: String,
    pub(crate) root_budget: BudgetLimits,
    pub(crate) project_budget: BudgetLimits,
    pub(crate) inputs: Vec<ArtifactRequirement>,
    pub(crate) outputs: Vec<GoalOutputSelection>,
    pub(crate) validators: Vec<IntegrationValidatorBinding>,
    pub(crate) usage: IntegrationSharedUsage,
    pub(crate) expires_at_ms: i64,
    pub(crate) workspace_ownership: IntegrationWorkspaceMaterial,
}

#[derive(Clone, Serialize)]
pub(crate) struct IntegrationDelegatedMaterial {
    pub(crate) factory_scope_id: String,
    pub(crate) factory_scope_digest: String,
    pub(crate) factory_run_id: String,
    pub(crate) child_spec_revision: u64,
    pub(crate) lineage: crate::factory::FactoryTaskLineage,
    pub(crate) delegation_event_sequence: u64,
    pub(crate) delegation_document_digest: String,
    pub(crate) original_request: crate::factory::FactoryDelegateRequest,
    pub(crate) original_handle: crate::factory::FactoryTaskHandle,
    pub(crate) workspace_id: String,
    pub(crate) source_base: String,
    pub(crate) managed_creation_ref: String,
}
#[derive(Clone, Serialize)]
pub(crate) enum IntegrationWorkspaceMaterial {
    TaskManaged(TaskWorkspaceReservationReceipt),
    FactoryDelegated(IntegrationDelegatedMaterial),
}
