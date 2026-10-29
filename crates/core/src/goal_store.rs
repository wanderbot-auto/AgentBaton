//! Durable Goal approval and versioned plan previews. This module never dispatches.
//! Private authority is reconstructed from native-confirmed records, not JSON flags.
use crate::{
    execution::{BudgetStatus, ExecutionBinding},
    goal_plan::*,
    policy::ExecutionGrant,
    registry::{ManagedWorkspace, Project},
    sessions::ApprovalProposal,
    storage::{db_error, decode, encode, now, Event, Store, Task, TaskState},
    CoreError, Result,
};
use agentbaton_workspaces::validation::{ArtifactRequirement, BoundArtifact};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use uuid::Uuid;

#[path = "goal_runtime.rs"]
mod runtime;
pub(crate) use runtime::migrate_graph;
use runtime::runtime_scope_for_revision;
pub(crate) use runtime::{
    migrate as migrate_runtime, ordinary_session_budget_gate, owns_session,
    permits_continuing_task, reserve_session, verify_task_acceptance_state, GoalDispatchContext,
    GraphReadContext, GraphReadWitness, GraphWorkers,
};
pub use runtime::{
    GoalPlanAdvance, GoalRun, GoalRunDispatch, GoalRunSelection, GraphArtifactReference,
    GraphCandidateReceipt, GraphCriterionBinding, GraphDefinitionReusePermission,
    GraphExecutionSnapshot, GraphInputBinding, GraphNodeAcceptanceSnapshot,
    GraphNodeApprovalSelection, GraphNodeView, GraphOwnerDecision, GraphOwnerDecisionResult,
    GraphRunBinding, GraphRunDetails, GraphRunSelection, GraphRunSummary, GraphTaskAcceptance,
    GraphValidationReceipt, GraphValidationView,
};

pub const MAX_GOAL_PLAN_BYTES: usize = 262_144;
const MAX_STORED_BYTES: i64 = 1_048_576;
const MAX_SCOPES: i64 = 256;
const MAX_REVISIONS: i64 = 256;
const OWNER_RULE: &str = "goal-owner-review";
const INPUT_RULE: &str = "goal-input-sha256";
fn err(code: &str, message: &str) -> CoreError {
    CoreError::new(code, message)
}
fn invalid() -> CoreError {
    err("INVALID_REQUEST","Select bounded Goal records, exact artifacts and explicit limits; authority flags are not accepted")
}
fn changed() -> CoreError {
    err("INPUT_CHANGED","Goal approval, task specification, connection, workspace or input snapshot changed; review a new Goal scope")
}
fn limited() -> CoreError {
    err(
        "RESOURCE_LIMIT",
        "Goal record exceeds its raw-byte/count bound; no history was truncated",
    )
}
fn denied() -> CoreError {
    err("PERMISSION_DENIED","A live, separate native Goal approval and its unchanged execution capabilities are required")
}
fn plan_error(e: PlanError) -> CoreError {
    match e {
        PlanError::StaleRevision => err(
            "STALE_REVISION",
            "Goal plan changed; compare the current revision before proposing a replacement",
        ),
        PlanError::InputChanged | PlanError::StaleEvidence => changed(),
        PlanError::Limit => limited(),
        PlanError::AuthorityExpansion => denied(),
        _ => invalid(),
    }
}
fn sha<T: Serialize>(value: &T) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(encode(value)?.as_bytes())))
}
fn id(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 128
        && v.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._:-".contains(&c))
}
fn key(v: &str) -> Result<()> {
    if id(v) {
        Ok(())
    } else {
        Err(invalid())
    }
}
fn sql_num(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|_| invalid())
}
fn ms() -> Result<i64> {
    i64::try_from(time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
        .map_err(|_| invalid())
}
fn raw<T: for<'a> Deserialize<'a>>(db: &Connection, table: &str, id: &str) -> Result<T> {
    let sql=match table {
        "tasks"=>"SELECT document FROM tasks WHERE id=?1 AND octet_length(document)<=?2",
        "sessions"=>"SELECT document FROM sessions WHERE id=?1 AND octet_length(document)<=?2",
        "projects"=>"SELECT document FROM projects WHERE id=?1 AND octet_length(document)<=?2",
        "managed_workspaces"=>"SELECT document FROM managed_workspaces WHERE id=?1 AND octet_length(document)<=?2",
        "execution_grants"=>"SELECT document FROM execution_grants WHERE id=?1 AND octet_length(document)<=?2",
        "connections"=>"SELECT document FROM connections WHERE id=?1 AND octet_length(document)<=?2",
        "goal_scopes"=>"SELECT document FROM goal_scopes WHERE id=?1 AND octet_length(document)<=?2",
        "approval_proposals"=>"SELECT document FROM approval_proposals WHERE id=?1 AND octet_length(document)<=?2",
        "validation_policies"=>"SELECT document FROM validation_policies WHERE id=?1 AND octet_length(document)<=?2",
        "data_validation_policies"=>"SELECT document FROM data_validation_policies WHERE id=?1 AND octet_length(document)<=?2",
        "command_validation_policies"=>"SELECT document FROM command_validation_policies WHERE id=?1 AND octet_length(document)<=?2",
        _=>return Err(err("INTERNAL_ERROR","Unknown Goal record table")),
    };
    let doc: Option<String> = db
        .query_row(sql, params![id, MAX_STORED_BYTES], |r| r.get(0))
        .optional()
        .map_err(db_error)?;
    decode(&doc.ok_or_else(|| {
        err(
            "NOT_FOUND",
            "Goal record is unavailable or exceeds its raw-byte bound",
        )
    })?)
}
fn event(db: &Connection, id: &str, revision: u64, kind: &str, payload: Value) -> Result<()> {
    let e = Event {
        sequence: 0,
        event_id: Uuid::new_v4().to_string(),
        aggregate_id: id.into(),
        revision,
        kind: kind.into(),
        occurred_at: now()?,
        payload,
        source: if kind == "approval.confirmed" {
            "local_owner_confirmation"
        } else {
            "goal_core"
        }
        .into(),
    };
    db.execute("INSERT INTO events(document) VALUES(?1)", [encode(&e)?])
        .map_err(db_error)?;
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoalValidatorKind {
    Integrity,
    Data,
    Command,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GoalValidatorRef {
    pub kind: GoalValidatorKind,
    pub policy_id: String,
    pub revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GoalOutputSelection {
    pub artifact: ArtifactRequirement,
    pub validators: Vec<GoalValidatorRef>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GoalScopeSelection {
    pub task_id: String,
    pub expected_task_revision: u64,
    pub grant_id: String,
    pub expected_grant_revision: u64,
    pub max_nodes: u32,
    pub new_max_children: u32,
    pub new_max_depth: u32,
    pub root_budget: BudgetLimits,
    pub project_budget: BudgetLimits,
    pub inputs: Vec<ArtifactRequirement>,
    pub outputs: Vec<GoalOutputSelection>,
    pub writable_prefixes: Vec<String>,
    pub owner_acceptance_criteria: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanSubmission {
    pub scope_id: String,
    pub proposal: GoalPlanProposal,
}
/// IPC retains the exact inner text so serde can reject duplicate fields.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanRawSubmission {
    pub scope_id: String,
    pub plan_json: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolvedInputFile {
    artifact: ArtifactRequirement,
    sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolvedValidator {
    reference: GoalValidatorRef,
    rule_id: String,
    digest: String,
    definition: Value,
    source_runs_started: u32,
    source_run_states: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GoalRefs {
    task_id: String,
    project_id: String,
    task_goal: String,
    spec_revision: u64,
    project_revision: u64,
    workspace: ManagedWorkspace,
    connection: Value,
    grant_before: ExecutionGrant,
    grant_after: ExecutionGrant,
    grant_confirmation_id: String,
    inputs: Vec<ResolvedInputFile>,
    validators: Vec<ResolvedValidator>,
    project_consumed: u32,
    project_active: u32,
    root_consumed: u32,
    root_active: u32,
    root_children_consumed: u32,
    project_children_consumed: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    factory_origin: Option<runtime::factory::FactoryGoalOrigin>,
    id: String,
    revision: u64,
    revoked: bool,
    approval_id: String,
    selection: GoalScopeSelection,
    refs: GoalRefs,
    scope_digest: String,
    created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanEditingContext {
    pub max_nodes: u32,
    pub writable_prefixes: Vec<String>,
    pub output_artifacts: BTreeMap<String, ArtifactRequirement>,
    pub rule_executors: BTreeMap<String, GoalRuleExecutorKind>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoalRuleExecutorKind {
    Owner,
    Validator,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalScopeView {
    pub id: String,
    pub revision: u64,
    pub revoked: bool,
    pub task_id: String,
    pub project_id: String,
    pub root_grant_id: String,
    pub root_grant_revision: u64,
    pub authority_revision: u64,
    pub scope_digest: String,
    pub goal: GoalBinding,
    pub root_budget: BudgetLimits,
    pub project_budget: BudgetLimits,
    pub outputs: BTreeMap<String, String>,
    pub required_acceptance_rules: Vec<String>,
    pub contracts: BTreeMap<String, Vec<String>>,
    pub inputs: BTreeMap<String, GoalInputView>,
    pub binding: ExecutionBinding,
    pub program_sha256: String,
    pub expires_at_ms: i64,
    pub rules: BTreeMap<String, String>,
    pub workspace_id: String,
    pub input_revision: String,
    pub planning_only: bool,
    /// Read-only projection of the original approval, never an execution token.
    /// Missing on older cached approval results; editors must not guess it.
    #[serde(default)]
    pub edit_context: Option<GoalPlanEditingContext>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalInputView {
    pub contract_id: String,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanView {
    pub scope_id: String,
    pub proposal: GoalPlanProposal,
    pub digest: String,
    pub created_at: String,
    pub state: String,
    pub dispatched: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MeterLedger {
    limits: BudgetLimits,
    observed_attempts: u32,
    runtime_ms: Option<u64>,
    output_bytes: Option<u64>,
    tokens: Option<u64>,
    cost_microunits: Option<u64>,
}
#[derive(Clone, Debug, Serialize)]
pub struct GoalBudgetUsageView {
    pub attempts: u32,
    pub active: u32,
    pub children: u32,
    pub runtime_ms: Option<u64>,
    pub output_bytes: Option<u64>,
    pub tokens: Option<u64>,
    pub cost_microunits: Option<u64>,
}
#[derive(Clone, Debug, Serialize)]
pub struct GoalBudgetUsage {
    pub root: GoalBudgetUsageView,
    pub project: GoalBudgetUsageView,
}
#[derive(Clone, Debug, Serialize)]
pub struct GoalReadyPreview {
    pub scope_id: String,
    pub plan_id: String,
    pub plan_revision: u64,
    pub journal_sequence: u64,
    pub read_only: bool,
    pub reservation_made: bool,
    pub dispatched: bool,
    pub unknown_dimensions: Vec<String>,
    pub blocked_reasons: Vec<String>,
    pub decision: Option<ReadyDecision>,
    pub budget_usage: GoalBudgetUsage,
}

fn native_confirmation(db: &Connection, id: &str, kind: &str) -> Result<(ApprovalProposal, Value)> {
    let p: ApprovalProposal = raw(db, "approval_proposals", id)?;
    let result:Option<String>=db.query_row("SELECT result FROM approval_proposals WHERE id=?1 AND COALESCE(octet_length(result),0)<=?2",params![id,MAX_STORED_BYTES],|r|r.get(0)).optional().map_err(db_error)?.flatten();
    let journal:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='approval.confirmed' AND json_extract(document,'$.source')='local_owner_confirmation')",[id],|r|r.get(0)).map_err(db_error)?;
    if p.id != id || p.kind != kind || !p.consumed || !journal {
        return Err(denied());
    }
    Ok((p, decode(&result.ok_or_else(denied)?)?))
}
fn grant_origin(db: &Connection, grant: &ExecutionGrant) -> Result<String> {
    let mut q=db.prepare("SELECT id FROM approval_proposals WHERE octet_length(document)<=?1 AND COALESCE(octet_length(result),0)<=?1 AND json_extract(result,'$.id')=?2 AND json_extract(document,'$.kind')='grant' LIMIT 2").map_err(db_error)?;
    let ids = q
        .query_map(params![MAX_STORED_BYTES, grant.id], |r| {
            r.get::<_, String>(0)
        })
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    if ids.len() != 1 {
        return Err(denied());
    }
    let (_, result) = native_confirmation(db, &ids[0], "grant")?;
    let original: ExecutionGrant = serde_json::from_value(result).map_err(|_| changed())?;
    if original.id != grant.id
        || original.project_id != grant.project_id
        || original.root_grant_id != grant.root_grant_id
        || original.parent_id != grant.parent_id
        || original.depth != grant.depth
        || original.approval_id != grant.approval_id
        || original.approved_at != grant.approved_at
        || original.expires_at_ms != grant.expires_at_ms
        || original.connection_bindings != grant.connection_bindings
        || original.workspace_bindings != grant.workspace_bindings
        || original.enforcement != grant.enforcement
    {
        return Err(changed());
    }
    let mut only_ceilings = grant.clone();
    only_ceilings.revision = original.revision;
    only_ceilings.scope.max_children = original.scope.max_children;
    only_ceilings.scope.max_depth = original.scope.max_depth;
    if only_ceilings != original {
        return Err(changed());
    }
    if grant.revision != original.revision {
        let (document,approval):(String,String)=db.query_row("SELECT document,goal_approval_id FROM goal_grant_revisions WHERE grant_id=?1 AND revision=?2 AND octet_length(document)<=?3",params![grant.id,sql_num(grant.revision)?,MAX_STORED_BYTES],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?.ok_or_else(changed)?;
        if decode::<ExecutionGrant>(&document)? != *grant {
            return Err(changed());
        }
        let (p, result) = native_confirmation(db, &approval, "goal")?;
        if result["root_grant_id"] != grant.id
            || result["root_grant_revision"] != grant.revision
            || projected_grant(&p.snapshot["resolved"], "grant_after")? != *grant
        {
            return Err(changed());
        }
    }
    Ok(ids[0].clone())
}
fn budget(db: &Connection, project: bool, id: &str) -> Result<Option<BudgetStatus>> {
    db.query_row(
        "SELECT total_limit,parallel_limit,consumed,active FROM budgets WHERE scope=?1",
        [format!("{}:{id}", if project { "project" } else { "root" })],
        |r| {
            Ok(BudgetStatus {
                total_limit: r.get(0)?,
                parallel_limit: r.get(1)?,
                consumed: r.get(2)?,
                active: r.get(3)?,
            })
        },
    )
    .optional()
    .map_err(db_error)
}
/// The execution admission path uses these rows directly. Recovered shared
/// history must therefore reach the durable counters, not only the preview.
/// Caller holds the Goal approval IMMEDIATE transaction, after snapshot checks.
fn reconcile_budget(
    db: &Connection,
    approval_id: &str,
    project: bool,
    id: &str,
    limits: &BudgetLimits,
    consumed: u32,
    active: u32,
) -> Result<()> {
    let scope = format!("{}:{id}", if project { "project" } else { "root" });
    let before = budget(db, project, id)?;
    let after = match &before {
        Some(existing) => BudgetStatus {
            // A Goal override narrows its own policy; it does not rewrite an
            // already established execution-budget ceiling.
            total_limit: existing.total_limit,
            parallel_limit: existing.parallel_limit,
            consumed: consumed.max(existing.consumed),
            active: active.max(existing.active),
        },
        None => BudgetStatus {
            total_limit: limits.attempts,
            parallel_limit: limits.parallel,
            consumed,
            active,
        },
    };
    if before.as_ref() == Some(&after) {
        return Ok(());
    }
    db.execute(
        "INSERT INTO budgets(scope,total_limit,parallel_limit,consumed,active) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(scope) DO UPDATE SET consumed=max(budgets.consumed,excluded.consumed),active=max(budgets.active,excluded.active)",
        params![scope, after.total_limit, after.parallel_limit, after.consumed, after.active],
    ).map_err(db_error)?;
    event(
        db,
        approval_id,
        1,
        "goal.budget_history_reconciled",
        json!({"goal_approval_id":approval_id,"budget_scope":scope,"before":before,"after":after,"source":"shared_persisted_attempt_history","reservation_made":false}),
    )
}
fn counts(db: &Connection, task: &Task, grant: &ExecutionGrant) -> Result<(u32, u32, u32, u32)> {
    let (p,pa):(u32,u32)=db.query_row("SELECT count(DISTINCT a.id),count(DISTINCT CASE WHEN d.state!='stopped' THEN a.id END) FROM attempts a JOIN tasks t ON t.id=a.task_id LEFT JOIN dispatch_intents d ON d.attempt_id=a.id WHERE json_extract(t.document,'$.project_id')=?1",[&task.project_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
    let (g,ga):(u32,u32)=db.query_row("SELECT count(DISTINCT a.id),count(DISTINCT CASE WHEN d.state!='stopped' THEN a.id END) FROM attempts a LEFT JOIN dispatch_intents d ON d.attempt_id=a.id WHERE a.root_task_id=?1 OR EXISTS(SELECT 1 FROM execution_grants g WHERE json_extract(g.document,'$.root_grant_id')=?2 AND substr(json_extract(a.binding,'$.policy_revision'),1,length(g.id)+1)=g.id||':') OR EXISTS(SELECT 1 FROM sessions s WHERE json_extract(s.document,'$.attempt_id')=a.id AND s.root_grant_id=?2)",params![task.id,grant.root_grant_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
    let pb = budget(db, true, &task.project_id)?;
    let gb = budget(db, false, &task.id)?;
    Ok((
        p.max(pb.as_ref().map_or(0, |b| b.consumed)),
        pa.max(pb.as_ref().map_or(0, |b| b.active)),
        g.max(gb.as_ref().map_or(0, |b| b.consumed)),
        ga.max(gb.as_ref().map_or(0, |b| b.active)),
    ))
}
fn no_writers(db: &Connection, task: &Task, grant: &ExecutionGrant) -> Result<()> {
    let active:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM dispatch_intents d JOIN attempts a ON a.id=d.attempt_id WHERE d.state!='stopped' AND (a.root_task_id=?1 OR json_extract(a.binding,'$.workspace_id') IN (SELECT value FROM json_each(?2)) OR EXISTS(SELECT 1 FROM sessions s WHERE json_extract(s.document,'$.attempt_id')=a.id AND s.root_grant_id=?3)))",params![task.id,encode(&grant.scope.workspace_ids)?,grant.root_grant_id],|r|r.get(0)).map_err(db_error)?;
    let command:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM command_workspace_fences WHERE workspace_id IN (SELECT value FROM json_each(?1)))",[encode(&grant.scope.workspace_ids)?],|r|r.get(0)).map_err(db_error)?;
    let graph:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM goal_graph_jobs WHERE workspace_id IN (SELECT value FROM json_each(?1)) AND state IN('prepared','running','cancel_requested','outcome_unknown'))",[encode(&grant.scope.workspace_ids)?],|r|r.get(0)).map_err(db_error)?;
    if active || command || graph {
        return Err(err("WORKSPACE_BUSY","Stop and reconcile all root/workspace intents and command read fences before reviewing Goal authority"));
    }
    Ok(())
}
fn unmapped_goal_history(db: &Connection, task_id: &str, grant_id: &str) -> Result<bool> {
    db.query_row("SELECT EXISTS(SELECT 1 FROM goal_node_evidence e JOIN goal_scopes s ON s.id=e.scope_id WHERE s.root_grant_id=?1 OR s.task_id=?2) OR EXISTS(SELECT 1 FROM goal_external_action_journal WHERE root_grant_id=?1)",params![grant_id,task_id],|r|r.get(0)).map_err(db_error)
}
fn limits_fit(s: &GoalScopeSelection, grant: &ExecutionGrant) -> Result<()> {
    if !s.root_budget.narrower(&s.project_budget)
        || s.root_budget.attempts == 0
        || s.project_budget.attempts == 0
        || s.root_budget.parallel == 0
        || s.project_budget.parallel == 0
        || s.root_budget.runtime_ms == 0
        || s.root_budget.output_bytes == 0
        || s.project_budget.runtime_ms == 0
        || s.project_budget.output_bytes == 0
        || s.root_budget.attempts > grant.scope.max_attempts
        || s.root_budget.parallel > grant.scope.max_concurrency
        || s.root_budget.runtime_ms > grant.scope.max_duration_ms
        || s.root_budget.attempts > s.project_budget.attempts
        || s.root_budget.parallel > s.project_budget.parallel
        || s.root_budget.runtime_ms > s.project_budget.runtime_ms
        || s.root_budget.output_bytes > s.project_budget.output_bytes
        || s.root_budget.children > s.new_max_children
        || s.root_budget.children > s.project_budget.children
        || s.root_budget.reworks > s.project_budget.reworks
        || s.new_max_children < grant.scope.max_children
        || s.new_max_depth < grant.scope.max_depth
        || s.new_max_children > 1000
        || s.new_max_depth > 32
        || s.max_nodes == 0
        || s.max_nodes > 256
        || s.new_max_children
            .checked_add(1)
            .is_none_or(|n| s.max_nodes > n)
    {
        return Err(denied());
    }
    if s.root_budget.tokens.is_some()
        || s.root_budget.cost_microunits.is_some()
        || s.project_budget.tokens.is_some()
        || s.project_budget.cost_microunits.is_some()
    {
        return Err(err("CAPABILITY_UNSUPPORTED","Current native connections have no reliable token/cost meter; monetary limits cannot be advertised as enforced"));
    }
    Ok(())
}
fn resolve_validator(
    db: &Connection,
    r: &GoalValidatorRef,
    project: &str,
    workspace: &ManagedWorkspace,
    artifact: &ArtifactRequirement,
) -> Result<ResolvedValidator> {
    key(&r.policy_id)?;
    let (table, kind, tag) = match r.kind {
        GoalValidatorKind::Integrity => ("validation_policies", "validation", "integrity"),
        GoalValidatorKind::Data => ("data_validation_policies", "data_validation", "data"),
        GoalValidatorKind::Command => (
            "command_validation_policies",
            "command_validation",
            "command",
        ),
    };
    let policy: Value = raw(db, table, &r.policy_id)?;
    if policy["id"] != r.policy_id
        || policy["project_id"] != project
        || policy["workspace_id"] != workspace.id
        || policy["revision"] != r.revision
        || policy["revoked"] != false
    {
        return Err(changed());
    }
    let approval = policy["approval_id"].as_str().ok_or_else(changed)?;
    let (p, result) = native_confirmation(db, approval, kind)?;
    if result != policy {
        return Err(changed());
    }
    let contract = match r.kind {
        GoalValidatorKind::Integrity => {
            if policy["scope"] != "artifact_integrity_only"
                || policy["contract"]["validator_id"] != "builtin.artifact-integrity.v1"
                || p.snapshot["contract"] != policy["contract"]
            {
                return Err(changed());
            }
            policy["contract"].clone()
        }
        GoalValidatorKind::Data => {
            if policy["scope"] != "data_contract_only"
                || policy["validator_id"] != agentbaton_workspaces::deterministic::VALIDATOR_ID
                || p.snapshot["validator_id"] != policy["validator_id"]
                || p.snapshot["file_contract"] != policy["file_contract"]
                || p.snapshot["table"] != policy["table"]
                || p.snapshot["contract_digest"] != policy["contract_digest"]
            {
                return Err(changed());
            }
            policy["file_contract"].clone()
        }
        GoalValidatorKind::Command => {
            if policy["scope"] != agentbaton_workspaces::command_validation::SCOPE
                || policy["validator_id"] != agentbaton_workspaces::command_validation::VALIDATOR_ID
                || policy["max_runs"] != 1
                || p.snapshot["max_runs"] != policy["max_runs"]
                || p.snapshot["refs"] != policy["refs"]
                || p.snapshot["contract"] != policy["contract"]
                || p.snapshot["contract_digest"] != policy["contract_digest"]
                || p.snapshot["intent"] != policy["selection"]
            {
                return Err(changed());
            }
            policy["contract"]["file_contract"].clone()
        }
    };
    let source_task: Task = raw(db, "tasks", policy["task_id"].as_str().ok_or_else(changed)?)?;
    if contract["subject"]["task_id"] != source_task.id
        || contract["subject"]["spec_revision"] != source_task.spec_revision
        || contract["subject"]["input_revision"] != workspace.base_revision
        || !contract["outputs"].as_array().is_some_and(|a| {
            a.iter().any(|o| {
                o["requirement"] == json!(artifact)
                    && o["canonical_root"] == json!(workspace.root)
                    && o["root_device"] == json!(workspace.identity.as_ref().map(|i| i.device))
                    && o["root_inode"] == json!(workspace.identity.as_ref().map(|i| i.inode))
            })
        })
    {
        return Err(err("CAPABILITY_UNSUPPORTED","Selected validator does not approve this exact output/input version; no policy template or result authority was invented"));
    }
    let (source_runs_started, source_run_states) = if r.kind == GoalValidatorKind::Command {
        let count: u32 = db
            .query_row(
                "SELECT count(*) FROM command_validation_runs WHERE policy_id=?1",
                [&r.policy_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let mut statement=db.prepare("SELECT state FROM command_validation_runs WHERE policy_id=?1 ORDER BY rowid LIMIT 2").map_err(db_error)?;
        let states = statement
            .query_map([&r.policy_id], |row| row.get::<_, String>(0))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        let max = policy["max_runs"].as_u64().ok_or_else(changed)?;
        if u64::from(count) >= max {
            return Err(err("CAPABILITY_UNSUPPORTED","Selected one-shot command authorization is already used; review a fresh exact command policy. A stored definition or historical receipt never permits another run"));
        }
        (count, states)
    } else {
        (0, vec![])
    };
    Ok(ResolvedValidator {
        reference: r.clone(),
        rule_id: format!("{tag}:{}", r.policy_id),
        digest: sha(&policy)?,
        definition: policy,
        source_runs_started,
        source_run_states,
    })
}
/// Factory planning may reference this exact native-approved definition. This
/// wrapper does not create Goal scope/approval, mutate Grant or authorize a run.
pub(crate) fn resolve_factory_child_validator(
    db: &Connection,
    reference: &GoalValidatorRef,
    project: &str,
    workspace: &ManagedWorkspace,
    artifact: &ArtifactRequirement,
) -> Result<Value> {
    serde_json::to_value(resolve_validator(
        db, reference, project, workspace, artifact,
    )?)
    .map_err(|_| changed())
}

fn resolve_selection(
    db: &Connection,
    s: &GoalScopeSelection,
    data: Option<&crate::data_workspaces::DataInputWitness>,
) -> Result<GoalRefs> {
    key(&s.task_id)?;
    key(&s.grant_id)?;
    if s.owner_acceptance_criteria.trim().is_empty()
        || s.owner_acceptance_criteria.len() > 2048
        || s.inputs.len() > 16
        || s.outputs.is_empty()
        || s.outputs.len() > 16
        || encode(s)?.len() > 16_384
    {
        return Err(invalid());
    }
    let task: Task = raw(db, "tasks", &s.task_id)?;
    if task.revision != s.expected_task_revision {
        return Err(err("STALE_REVISION", "Task changed before Goal proposal"));
    }
    if matches!(
        task.state,
        TaskState::Accepted
            | TaskState::Cancelled
            | TaskState::CancelRequested
            | TaskState::OutcomeUnknown
    ) {
        return Err(denied());
    }
    let project: Project = raw(db, "projects", &task.project_id)?;
    if project.archived {
        return Err(denied());
    }
    let g: ExecutionGrant = raw(db, "execution_grants", &s.grant_id)?;
    if g.revision != s.expected_grant_revision {
        return Err(err(
            "STALE_REVISION",
            "Root execution grant changed before Goal proposal",
        ));
    }
    if g.parent_id.is_some()
        || g.root_grant_id != g.id
        || g.scope.workspace_ids.len() != 1
        || g.scope.connection_ids.len() != 1
    {
        return Err(err("CAPABILITY_UNSUPPORTED","Goal approval currently resolves one existing local root grant; arbitrary/remote or multi-connection authority is unsupported"));
    }
    no_writers(db, &task, &g)?;
    if unmapped_goal_history(db, &task.id, &g.id)? {
        return Err(err("WORKSPACE_BUSY","Retained Goal evidence/action history requires actual scheduler reconciliation; a new scope cannot reset it"));
    }
    let grant = if data.is_none() {
        crate::policy::validate_grant_from(
            db,
            &g.id,
            &task.project_id,
            &g.scope.workspace_ids[0],
            &g.scope.connection_ids[0],
        )?
    } else {
        crate::policy::validate_grant_from_with_data(
            db,
            &g.id,
            &task.project_id,
            &g.scope.workspace_ids[0],
            &g.scope.connection_ids[0],
            data,
        )?
    };
    let grant_confirmation_id = grant_origin(db, &grant)?;
    limits_fit(s, &grant)?;
    no_writers(db, &task, &grant)?;
    let workspace: ManagedWorkspace = raw(db, "managed_workspaces", &grant.scope.workspace_ids[0])?;
    if workspace.project_id != task.project_id
        || workspace.state != "ready"
        || workspace.base_revision.len() > 128
    {
        return Err(changed());
    }
    let connection: Value = raw(db, "connections", &grant.scope.connection_ids[0])?;
    let mut ids = BTreeSet::new();
    let mut inputs = Vec::new();
    let mut total = 0u64;
    for a in &s.inputs {
        if !id(&a.artifact_id)
            || !ids.insert(&a.artifact_id)
            || a.max_bytes == 0
            || a.max_bytes > 16 * 1024 * 1024
        {
            return Err(invalid());
        }
        total = total.checked_add(a.max_bytes).ok_or_else(limited)?;
        if total > 64 * 1024 * 1024 {
            return Err(limited());
        }
        BoundArtifact::from_approved_root(&workspace.root, a.clone()).map_err(|_| changed())?;
        let digest = if matches!(
            workspace.identity.as_ref().map(|i| &i.kind),
            Some(agentbaton_workspaces::WorkspaceKind::Directory)
        ) {
            data.ok_or_else(|| {
                err(
                    "WORKSPACE_BUSY",
                    "Goal data inputs require a fresh exact-request dataset observation",
                )
            })?
            .selected_input_hash(db, &workspace, a)?
        } else {
            let bytes = agentbaton_workspaces::read_artifact(
                &workspace.root,
                &a.relative_path,
                a.max_bytes,
            )
            .map_err(|_| changed())?;
            agentbaton_workspaces::validation::sha256(&bytes)
        };
        if a.expected_sha256.as_ref().is_some_and(|h| h != &digest) {
            return Err(changed());
        }
        inputs.push(ResolvedInputFile {
            artifact: a.clone(),
            sha256: digest,
        });
    }
    let mut validators = BTreeMap::new();
    let mut output_bytes = 0u64;
    for output in &s.outputs {
        let a = &output.artifact;
        if !id(&a.artifact_id)
            || !ids.insert(&a.artifact_id)
            || a.max_bytes == 0
            || output.validators.len() > 8
        {
            return Err(invalid());
        }
        output_bytes = output_bytes.checked_add(a.max_bytes).ok_or_else(limited)?;
        if output_bytes > s.root_budget.output_bytes {
            return Err(denied());
        }
        BoundArtifact::from_approved_root(&workspace.root, a.clone()).map_err(|_| changed())?;
        for r in &output.validators {
            let v = resolve_validator(db, r, &task.project_id, &workspace, a)?;
            if validators
                .insert(v.rule_id.clone(), v.clone())
                .is_some_and(|old| old.digest != v.digest)
            {
                return Err(changed());
            }
        }
    }
    let (project_consumed, project_active, root_consumed, root_active) = counts(db, &task, &grant)?;
    for (project, id, limits) in [
        (true, task.project_id.as_str(), &s.project_budget),
        (false, task.id.as_str(), &s.root_budget),
    ] {
        if budget(db, project, id)?
            .is_some_and(|b| limits.attempts > b.total_limit || limits.parallel > b.parallel_limit)
        {
            return Err(denied());
        }
    }
    let root_children_consumed:u32=db.query_row("SELECT count(*) FROM execution_grants WHERE parent_id IS NOT NULL AND json_extract(document,'$.root_grant_id')=?1",[&grant.id],|r|r.get(0)).map_err(db_error)?;
    let project_children_consumed: u32 = db
        .query_row(
            "SELECT count(*) FROM execution_grants WHERE parent_id IS NOT NULL AND project_id=?1",
            [&task.project_id],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    let mut grant_after = grant.clone();
    grant_after.revision = grant.revision.checked_add(1).ok_or_else(limited)?;
    grant_after.scope.max_children = s.new_max_children;
    grant_after.scope.max_depth = s.new_max_depth;
    Ok(GoalRefs {
        task_id: task.id,
        project_id: task.project_id,
        task_goal: task.goal,
        spec_revision: task.spec_revision,
        project_revision: project.revision,
        workspace,
        connection,
        grant_before: grant,
        grant_after,
        grant_confirmation_id,
        inputs,
        validators: validators.into_values().collect(),
        project_consumed,
        project_active,
        root_consumed,
        root_active,
        root_children_consumed,
        project_children_consumed,
    })
}

fn projected_grant(snapshot: &Value, which: &str) -> Result<ExecutionGrant> {
    let diff = &snapshot[which];
    if diff["immutable_ref"] != "resolved.grant_immutable" {
        return Err(changed());
    }
    let mut full = snapshot["grant_immutable"].clone();
    if full["id"] != diff["id"] {
        return Err(changed());
    }
    full["revision"] = diff["revision"].clone();
    full["scope"]["max_children"] = diff["scope"]["max_children"].clone();
    full["scope"]["max_depth"] = diff["scope"]["max_depth"].clone();
    serde_json::from_value(full).map_err(|_| changed())
}
/// Complete authorization projection, without repeating the same approved
/// command selection/refs alongside its full immutable contract. The private
/// resolved-record digest binds all original metadata as well.
fn approval_refs(r: &GoalRefs) -> Result<Value> {
    let mut view = json!(r);
    let mut shared = json!(r.grant_before);
    shared
        .as_object_mut()
        .ok_or_else(changed)?
        .remove("revision");
    shared["scope"]
        .as_object_mut()
        .ok_or_else(changed)?
        .remove("max_children");
    shared["scope"]
        .as_object_mut()
        .ok_or_else(changed)?
        .remove("max_depth");
    view["grant_immutable"] = shared;
    view["grant_projection_format"] = json!("shared_immutable_with_revision_ceilings");
    for (name, g) in [
        ("grant_before", &r.grant_before),
        ("grant_after", &r.grant_after),
    ] {
        view[name] = json!({"id":g.id,"revision":g.revision,"scope":{"max_children":g.scope.max_children,"max_depth":g.scope.max_depth},"immutable_ref":"resolved.grant_immutable"});
    }
    if projected_grant(&view, "grant_before")? != r.grant_before
        || projected_grant(&view, "grant_after")? != r.grant_after
    {
        return Err(changed());
    }
    let connection_id = &r.grant_after.scope.connection_ids[0];
    let c = &r.connection;
    view["connection"] = json!({"id":connection_id,"name":c["name"],"provider":c["provider"],"protocol_route":c["protocol_route"],"executable":c["executable"],"arguments":c["arguments"],"identity_status":c["identity_status"],"config_status":c["config_status"],"probe_state":c["probe_state"],"immutable_binding_ref":format!("resolved.grant_immutable.connection_bindings.{connection_id}"),"record_sha256":sha(c)?,"configuration_changed":false});
    view["workspace"] = json!({"id":r.workspace.id,"project_id":r.workspace.project_id,"root":r.workspace.root,"base_revision":r.workspace.base_revision,"state":r.workspace.state,"revision":r.workspace.revision,"identity_ref":format!("resolved.grant_immutable.workspace_bindings.{}",r.workspace.id)});
    view["validators"]=Value::Array(r.validators.iter().map(|v| -> Result<Value> {
        let d=&v.definition;
        let contract=match v.reference.kind {GoalValidatorKind::Data=>json!({"file_contract":d["file_contract"],"table":d["table"]}),_=>d["contract"].clone()};
        let full_contract_json=encode(&contract)?;
        Ok(json!({"reference":v.reference,"rule_id":v.rule_id,"definition_digest":v.digest,"native_approval_id":d["approval_id"],"source_task_id":d["task_id"],"source_session_id":d["session_id"],"source_attempt_id":d["attempt_id"],"source_workspace_id":d["workspace_id"],"source_scope":d["scope"],"validator_id":d["validator_id"],"max_runs":d["max_runs"],"source_runs_started":v.source_runs_started,"source_run_states":v.source_run_states,"existing_command_authorization_reset":false,"approved_contract_json":full_contract_json,"task_acceptance":"not_established","historical_receipts_reused":false}))
    }).collect::<Result<Vec<_>>>()?);
    Ok(view)
}
fn host_policy(s: &GoalScopeSelection, r: &GoalRefs) -> Result<HostGoalPolicy> {
    if s.writable_prefixes.is_empty()
        || s.writable_prefixes.len() > 32
        || s.writable_prefixes.iter().any(|p| {
            p.len() > 4096
                || p.is_empty()
                || p.chars().any(char::is_control)
                || p.contains('\\')
                || p.split('/').any(|c| c.is_empty() || c == "." || c == "..")
                || PathBuf::from(p)
                    .components()
                    .any(|c| !matches!(c, std::path::Component::Normal(_)))
        })
    {
        return Err(invalid());
    }
    let owner_digest = sha(&(
        OWNER_RULE,
        &r.task_id,
        r.spec_revision,
        &r.task_goal,
        &s.owner_acceptance_criteria,
        &s.outputs,
        &r.validators,
    ))?;
    let mut rules = BTreeMap::from([(
        OWNER_RULE.into(),
        ApprovedRuleRef {
            kind: RuleKind::Owner,
            policy_digest: owner_digest,
            goal_acceptance: true,
        },
    )]);
    if !r.inputs.is_empty() {
        rules.insert(
            INPUT_RULE.into(),
            ApprovedRuleRef {
                kind: RuleKind::Deterministic,
                policy_digest: sha(&(INPUT_RULE, &r.inputs))?,
                goal_acceptance: false,
            },
        );
    }
    let mut contracts = BTreeMap::new();
    let mut external_inputs = BTreeMap::new();
    let mut outputs = BTreeMap::new();
    for i in &r.inputs {
        let contract = format!("input:{}", i.artifact.artifact_id);
        contracts.insert(
            contract.clone(),
            ArtifactContract {
                required_validators: BTreeSet::from([INPUT_RULE.into()]),
            },
        );
        external_inputs.insert(
            i.artifact.artifact_id.clone(),
            ExternalInput {
                contract_id: contract,
                sha256: i.sha256.clone(),
                verified: true,
            },
        );
    }
    for o in &s.outputs {
        let contract = format!("output:{}", o.artifact.artifact_id);
        let mut required = BTreeSet::from([OWNER_RULE.into()]);
        for v in &o.validators {
            let resolved = r
                .validators
                .iter()
                .find(|p| p.reference == *v)
                .ok_or_else(changed)?;
            required.insert(resolved.rule_id.clone());
            rules.insert(
                resolved.rule_id.clone(),
                ApprovedRuleRef {
                    kind: RuleKind::Deterministic,
                    policy_digest: resolved.digest.clone(),
                    goal_acceptance: false,
                },
            );
        }
        contracts.insert(
            contract.clone(),
            ArtifactContract {
                required_validators: required,
            },
        );
        outputs.insert(o.artifact.artifact_id.clone(), contract);
    }
    let w = &r.workspace;
    let pin = w.identity.as_ref().ok_or_else(changed)?;
    let connection_id = &r.grant_after.scope.connection_ids[0];
    let c = &r.grant_after.connection_bindings[connection_id];
    // Without native account verification all connections conservatively share
    // one local quota; different labels do not manufacture independent capacity.
    let provider_key = "local-unverified-native-account".to_string();
    Ok(HostGoalPolicy {
        root_task_id: r.task_id.clone(),
        input_revision: w.base_revision.clone(),
        max_nodes: s.max_nodes,
        max_depth: s.new_max_depth,
        root_budget: s.root_budget.clone(),
        project_budget: s.project_budget.clone(),
        resources: ResourceLimits {
            global: 1,
            hosts: BTreeMap::from([(c.host_id.clone(), 1)]),
            providers: BTreeMap::from([(provider_key.clone(), 1)]),
            workspaces: BTreeMap::from([(w.id.clone(), 1)]),
        },
        connections: BTreeMap::from([(
            connection_id.clone(),
            ConnectionPermit {
                provider_quota_key: provider_key,
                binding: c.clone(),
            },
        )]),
        workspaces: BTreeMap::from([(
            w.id.clone(),
            WorkspacePermit {
                host_id: c.host_id.clone(),
                canonical_root: pin.canonical_root.to_str().ok_or_else(changed)?.into(),
                device: pin.device,
                inode: pin.inode,
                writable_prefixes: s.writable_prefixes.clone(),
            },
        )]),
        rules,
        contracts,
        external_inputs,
        external_writes: BTreeMap::new(),
        external_actions: BTreeMap::new(),
        required_goal_outputs: outputs,
        acceptance_rules: BTreeSet::from([OWNER_RULE.into()]),
    })
}
fn policy_for_record(db: &Connection, record: &ScopeRecord) -> Result<HostGoalPolicy> {
    match &record.factory_origin {
        Some(origin) => {
            let policy = crate::factory::expected_child_goal_policy(
                db,
                &origin.factory_scope_id,
                &record.refs.task_id,
            )?;
            if json!(policy) != origin.policy {
                return Err(changed());
            }
            Ok(policy)
        }
        None => host_policy(&record.selection, &record.refs),
    }
}
fn root_task(record: &ScopeRecord) -> &str {
    record
        .factory_origin
        .as_ref()
        .map_or(record.refs.task_id.as_str(), |o| o.root_task_id.as_str())
}
fn owner_rule(record: &ScopeRecord) -> &str {
    if record.factory_origin.is_some() {
        "factory-child-owner-review"
    } else {
        OWNER_RULE
    }
}
fn input_rule(record: &ScopeRecord) -> &str {
    if record.factory_origin.is_some() {
        "factory-child-input-sha256"
    } else {
        INPUT_RULE
    }
}
fn approved_scope(db: &Connection, task: &Task, record: &ScopeRecord) -> Result<ApprovedGoalScope> {
    if let Some(origin) = &record.factory_origin {
        return ApprovedGoalScope::from_verified_factory(
            task,
            &record.refs.grant_after,
            policy_for_record(db, record)?,
            &record.approval_id,
            origin.approval_revision,
            &origin.planning_policy_digest,
            origin.expires_at_ms,
            ms()?,
        )
        .map_err(plan_error);
    }
    ApprovedGoalScope::from_verified_host(
        task,
        &record.refs.grant_after,
        host_policy(&record.selection, &record.refs)?,
        ms()?,
    )
    .map_err(plan_error)?
    .bind_native_goal_approval(&record.approval_id)
    .map_err(plan_error)
}
fn scope_view(
    db: &Connection,
    record: &ScopeRecord,
    scope: &ApprovedGoalScope,
) -> Result<GoalScopeView> {
    let view = recorded_scope_view(db, record)?;
    if view.goal != *scope.goal() || view.scope_digest != scope.digest() {
        return Err(changed());
    }
    Ok(view)
}
/// Plain recorded data, not a reconstructed execution authority. In particular,
/// historical reads never backdate host time or mint an expired scope token.
fn recorded_scope_view(db: &Connection, record: &ScopeRecord) -> Result<GoalScopeView> {
    let p = policy_for_record(db, record)?;
    let edit_context = GoalPlanEditingContext {
        max_nodes: record.selection.max_nodes,
        writable_prefixes: record.selection.writable_prefixes.clone(),
        output_artifacts: record
            .selection
            .outputs
            .iter()
            .map(|output| (output.artifact.artifact_id.clone(), output.artifact.clone()))
            .collect(),
        rule_executors: p
            .rules
            .keys()
            .map(|id| {
                let executor = if id == owner_rule(record) {
                    GoalRuleExecutorKind::Owner
                } else {
                    GoalRuleExecutorKind::Validator
                };
                (id.clone(), executor)
            })
            .collect(),
    };
    let connection_id = record.refs.connection["id"].as_str().ok_or_else(changed)?;
    let c = &record.refs.grant_after.connection_bindings[connection_id];
    let binding = ExecutionBinding {
        connection_id: connection_id.to_owned(),
        environment_id: c.environment_id.clone(),
        identity_ref: c.identity_ref.clone(),
        host_id: c.host_id.clone(),
        workspace_id: record.refs.workspace.id.clone(),
        config_revision: c.config_revision.clone(),
        input_revision: record.refs.workspace.base_revision.clone(),
        capability_snapshot_id: c.capability_snapshot_id.clone(),
        policy_revision: format!(
            "{}:{}",
            record.refs.grant_after.id, record.refs.grant_after.revision
        ),
    };
    Ok(GoalScopeView {
        id: record.id.clone(),
        revision: record.revision,
        revoked: record.revoked,
        task_id: record.refs.task_id.clone(),
        project_id: record.refs.project_id.clone(),
        root_grant_id: record.refs.grant_after.id.clone(),
        root_grant_revision: record.refs.grant_after.revision,
        authority_revision: record
            .factory_origin
            .as_ref()
            .map_or(record.refs.grant_after.revision, |o| o.approval_revision),
        scope_digest: record.scope_digest.clone(),
        goal: GoalBinding {
            project_id: record.refs.project_id.clone(),
            root_task_id: p.root_task_id.clone(),
            task_id: record.refs.task_id.clone(),
            spec_revision: record.refs.spec_revision,
            input_revision: record.refs.workspace.base_revision.clone(),
        },
        root_budget: p.root_budget,
        project_budget: p.project_budget,
        contracts: p
            .contracts
            .into_iter()
            .map(|(k, v)| (k, v.required_validators.into_iter().collect()))
            .collect(),
        inputs: p
            .external_inputs
            .into_iter()
            .map(|(k, v)| {
                (
                    k,
                    GoalInputView {
                        contract_id: v.contract_id,
                        sha256: v.sha256,
                    },
                )
            })
            .collect(),
        binding,
        program_sha256: c.executable_sha256.clone(),
        expires_at_ms: record
            .factory_origin
            .as_ref()
            .map_or(record.refs.grant_after.expires_at_ms, |o| o.expires_at_ms),
        outputs: p.required_goal_outputs,
        required_acceptance_rules: p.acceptance_rules.into_iter().collect(),
        rules: p
            .rules
            .into_iter()
            .map(|(k, v)| (k, v.policy_digest))
            .collect(),
        workspace_id: record.refs.workspace.id.clone(),
        input_revision: record.refs.workspace.base_revision.clone(),
        planning_only: true,
        edit_context: Some(edit_context),
    })
}
fn read_scope(
    db: &Connection,
    id: &str,
    data: Option<&crate::data_workspaces::DataInputWitness>,
) -> Result<(ScopeRecord, ApprovedGoalScope)> {
    key(id)?;
    let r: ScopeRecord = raw(db, "goal_scopes", id)?;
    if r.factory_origin.is_some() {
        return Err(err("AUTH_REQUIRED", "Factory-derived scopes require their fixed Factory child Goal methods; Native Goal methods cannot reinterpret them"));
    }
    let columns: (i64, bool, String, String) = db
        .query_row(
            "SELECT revision,revoked,task_id,root_grant_id FROM goal_scopes WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(db_error)?;
    if columns
        != (
            sql_num(r.revision)?,
            r.revoked,
            r.refs.task_id.clone(),
            r.refs.grant_after.id.clone(),
        )
    {
        return Err(changed());
    }
    if r.id != id || r.approval_id != id {
        return Err(changed());
    }
    let (p, result) = native_confirmation(db, &r.approval_id, "goal")?;
    if p.snapshot["intent"] != json!(r.selection)
        || p.snapshot["resolved"] != approval_refs(&r.refs)?
        || p.snapshot["resolved_digest"] != json!(sha(&r.refs)?)
        || result["id"] != id
        || result["scope_digest"] != r.scope_digest
    {
        return Err(changed());
    }
    let task: Task = raw(db, "tasks", &r.refs.task_id)?;
    if task.project_id != r.refs.project_id
        || task.spec_revision != r.refs.spec_revision
        || task.goal != r.refs.task_goal
    {
        return Err(changed());
    }
    let original: ExecutionGrant =
        read_grant_revision(db, &r.refs.grant_after.id, r.refs.grant_after.revision)?;
    if original != r.refs.grant_after {
        return Err(changed());
    }
    if r.revoked
        || matches!(
            task.state,
            TaskState::Accepted
                | TaskState::Cancelled
                | TaskState::CancelRequested
                | TaskState::OutcomeUnknown
        )
    {
        return Err(denied());
    }
    let latest: ExecutionGrant = raw(db, "execution_grants", &original.id)?;
    if latest.revoked || latest.expires_at_ms <= ms()? {
        return Err(denied());
    }
    grant_origin(db, &latest)?;
    no_writers(db, &task, &latest)?;
    if unmapped_goal_history(db, &task.id, &latest.id)? {
        return Err(err("WORKSPACE_BUSY","Retained Goal evidence requires reconciliation before reading inputs or revising plans"));
    }
    // Later Goal approvals may increase ceilings; they cannot migrate this
    // approval's inputs, identity, budget or historical binding revision.
    let mut same = latest.clone();
    same.revision = original.revision;
    same.scope.max_children = original.scope.max_children;
    same.scope.max_depth = original.scope.max_depth;
    if same != original {
        return Err(changed());
    }
    let project: Project = raw(db, "projects", &r.refs.project_id)?;
    if project.archived || project.revision != r.refs.project_revision {
        return Err(changed());
    }
    let workspace: ManagedWorkspace = raw(db, "managed_workspaces", &r.refs.workspace.id)?;
    if encode(&workspace)? != encode(&r.refs.workspace)? || workspace.state != "ready" {
        return Err(changed());
    }
    let data_workspace = matches!(
        workspace.identity.as_ref().map(|i| &i.kind),
        Some(agentbaton_workspaces::WorkspaceKind::Directory)
    );
    if data_workspace {
        crate::data_workspaces::require_data_input_from(db, &workspace, data)?;
    } else {
        if !crate::policy::live_workspace_matches(
            &workspace.root,
            workspace.identity.as_ref().ok_or_else(changed)?,
        )
        .map_err(|_| changed())?
        {
            return Err(changed());
        }
        let current_input =
            agentbaton_workspaces::inspect(&workspace.root).map_err(|_| changed())?;
        if !matches!(&current_input.kind,agentbaton_workspaces::WorkspaceKind::Git {head,dirty:false,..} if head==&workspace.base_revision)
        {
            return Err(changed());
        }
    }
    let connection: Value = raw(db, "connections", &original.scope.connection_ids[0])?;
    if connection != r.refs.connection {
        return Err(changed());
    }
    for i in &r.refs.inputs {
        let digest = if data_workspace {
            data.ok_or_else(changed)?
                .selected_input_hash(db, &workspace, &i.artifact)?
        } else {
            let bytes = agentbaton_workspaces::read_artifact(
                &workspace.root,
                &i.artifact.relative_path,
                i.artifact.max_bytes,
            )
            .map_err(|_| changed())?;
            agentbaton_workspaces::validation::sha256(&bytes)
        };
        if digest != i.sha256 {
            return Err(changed());
        }
    }
    for output in &r.selection.outputs {
        for v in &output.validators {
            let current =
                resolve_validator(db, v, &r.refs.project_id, &workspace, &output.artifact)?;
            if r.refs
                .validators
                .iter()
                .find(|p| p.reference == *v)
                .is_none_or(|old| old.digest != current.digest)
            {
                return Err(changed());
            }
        }
    }
    let scope = approved_scope(db, &task, &r)?;
    if scope.digest() != r.scope_digest {
        return Err(changed());
    }
    Ok((r, scope))
}
fn read_grant_revision(db: &Connection, id: &str, revision: u64) -> Result<ExecutionGrant> {
    let doc:Option<String>=db.query_row("SELECT document FROM goal_grant_revisions WHERE grant_id=?1 AND revision=?2 AND octet_length(document)<=?3",params![id,sql_num(revision)?,MAX_STORED_BYTES],|r|r.get(0)).optional().map_err(db_error)?;
    if let Some(doc) = doc {
        let g: ExecutionGrant = decode(&doc)?;
        if g.id != id || g.revision != revision {
            return Err(changed());
        }
        return Ok(g);
    }
    let g: ExecutionGrant = raw(db, "execution_grants", id)?;
    if g.revision != revision {
        return Err(err("NOT_FOUND","Immutable execution grant revision is unavailable; no current snapshot was substituted"));
    }
    Ok(g)
}
fn meter(
    db: &Connection,
    scope: &str,
    limits: &BudgetLimits,
    consumed: u32,
) -> Result<MeterLedger> {
    let old: Option<Option<String>> = db
        .query_row(
            "SELECT CASE WHEN octet_length(document)<=?2 THEN document END FROM goal_ledgers WHERE scope=?1",
            params![scope, MAX_STORED_BYTES],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)?;
    if let Some(old) = old {
        // An over-limit existing row is not evidence of absence/zero history.
        let old = old.ok_or_else(limited)?;
        let mut record: MeterLedger = decode(&old)?;
        if consumed < record.observed_attempts {
            return Err(changed());
        }
        if consumed > record.observed_attempts {
            record.runtime_ms = None;
            record.output_bytes = None;
            record.tokens = None;
            record.cost_microunits = None;
        }
        // Scope overrides cannot widen an established shared cumulative policy.
        if !limits.narrower(&record.limits) {
            return Err(denied());
        }
        return Ok(record);
    }
    let known = if consumed == 0 { Some(0) } else { None };
    Ok(MeterLedger {
        limits: limits.clone(),
        observed_attempts: consumed,
        runtime_ms: known,
        output_bytes: known,
        tokens: known,
        cost_microunits: known,
    })
}
fn remember_meter(
    db: &Connection,
    scope: &str,
    limits: &BudgetLimits,
    consumed: u32,
) -> Result<()> {
    let record = meter(db, scope, limits, consumed)?;
    db.execute(
        "INSERT OR IGNORE INTO goal_ledgers(scope,document) VALUES(?1,?2)",
        params![scope, encode(&record)?],
    )
    .map_err(db_error)?;
    Ok(())
}

pub(crate) fn migrate(db: &mut Connection, version: u32) -> Result<()> {
    if version < 9 {
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        tx.execute_batch("CREATE TABLE goal_grant_revisions(grant_id TEXT NOT NULL REFERENCES execution_grants(id),revision INTEGER NOT NULL CHECK(revision>0),document TEXT NOT NULL,goal_approval_id TEXT REFERENCES approval_proposals(id),PRIMARY KEY(grant_id,revision));
CREATE TABLE goal_scopes(id TEXT PRIMARY KEY REFERENCES approval_proposals(id),task_id TEXT NOT NULL REFERENCES tasks(id),root_grant_id TEXT NOT NULL REFERENCES execution_grants(id),revision INTEGER NOT NULL CHECK(revision>0),revoked INTEGER NOT NULL CHECK(revoked IN(0,1)),document TEXT NOT NULL);
CREATE UNIQUE INDEX goal_one_active_scope_per_task ON goal_scopes(task_id) WHERE revoked=0;
CREATE TABLE goal_plan_heads(scope_id TEXT PRIMARY KEY REFERENCES goal_scopes(id),plan_id TEXT NOT NULL UNIQUE,revision INTEGER NOT NULL CHECK(revision>0),digest TEXT NOT NULL);
CREATE TABLE goal_plan_revisions(scope_id TEXT NOT NULL REFERENCES goal_scopes(id),plan_id TEXT NOT NULL,revision INTEGER NOT NULL CHECK(revision>0),previous_digest TEXT,digest TEXT NOT NULL,document TEXT NOT NULL,created_at TEXT NOT NULL,PRIMARY KEY(scope_id,revision));
CREATE TABLE goal_plan_proposals(id TEXT PRIMARY KEY,scope_id TEXT NOT NULL REFERENCES goal_scopes(id),plan_id TEXT NOT NULL,revision INTEGER NOT NULL,document TEXT NOT NULL,digest TEXT NOT NULL,UNIQUE(scope_id,revision));
CREATE TABLE goal_ledgers(scope TEXT PRIMARY KEY,document TEXT NOT NULL);
CREATE TABLE goal_node_evidence(scope_id TEXT NOT NULL REFERENCES goal_scopes(id),node_id TEXT NOT NULL,document TEXT NOT NULL,PRIMARY KEY(scope_id,node_id));
CREATE TABLE goal_external_action_journal(root_grant_id TEXT NOT NULL REFERENCES execution_grants(id),action_id TEXT NOT NULL,document TEXT NOT NULL,PRIMARY KEY(root_grant_id,action_id));
PRAGMA user_version=9;").map_err(db_error)?;
        tx.commit().map_err(db_error)?;
    }
    Ok(())
}
impl Store {
    pub fn propose_goal_scope(
        &mut self,
        selection: &GoalScopeSelection,
        idempotency_key: &str,
    ) -> Result<ApprovalProposal> {
        key(idempotency_key)?;
        let cached =
            idempotency_entry(&self.conn, "local:approvals.propose:goal", idempotency_key)?;
        if let Some((_, raw)) = cached {
            let proposal: ApprovalProposal = decode(&raw)?;
            if proposal.kind != "goal" || proposal.snapshot["intent"] != json!(selection) {
                return Err(err(
                    "CONFIG_CONFLICT",
                    "Goal proposal key already identifies another exact selection",
                ));
            }
            // Returning the original proposal recovers a lost finalize response;
            // it neither refreshes authority nor bypasses finalizer rechecks.
            return Ok(proposal);
        }
        let refs = resolve_selection(&self.conn, selection, self.data_input.as_ref())?;
        let policy = host_policy(selection, &refs)?;
        // Display-only proposed data. No ApprovedGoalScope is constructed until
        // native Goal confirmation has entered the private finalizer below.
        let snapshot = json!({"intent":selection,"resolved":approval_refs(&refs)?,"resolved_digest":sha(&refs)?,"goal_policy":policy,"scope":"goal_planning_and_bounded_delegation","execution":"preview_only_in_this_version","warning":"Only displayed children/depth change; all other root-grant bindings/ceilings and shared counters remain fixed. Goal approval defines rules, never a passed result, Task acceptance, dispatch/model call, integration or release. A validator reference never renews a one-shot command permission or adopts a historical receipt. Attempts retain their grant revision; unknown meters remain unknown."});
        let snapshot_bytes = serde_json::to_string_pretty(&snapshot)
            .map_err(|_| invalid())?
            .len();
        if snapshot_bytes > 16_384 {
            let mut error=err("RESOURCE_LIMIT","Goal snapshot exceeds the complete native review bound; reduce selections, no approval was made");
            error.details_ref = Some(format!("goal-snapshot-bytes:{snapshot_bytes}"));
            return Err(error);
        }
        self.persist_proposal("goal", snapshot, idempotency_key)
    }
    /// Called only by the existing private native/owner finalizer, never a
    /// renderer consent boolean. Consumes Goal approval and grant CAS together.
    pub(crate) fn finalize_goal_scope_proposal(
        &mut self,
        id: &str,
        revision: u64,
    ) -> Result<Value> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let mut p: ApprovalProposal = raw(&tx, "approval_proposals", id)?;
        if p.id != id || p.kind != "goal" {
            return Err(denied());
        }
        let result:Option<String>=tx.query_row("SELECT result FROM approval_proposals WHERE id=?1 AND COALESCE(octet_length(result),0)<=?2",params![id,MAX_STORED_BYTES],|r|r.get(0)).map_err(db_error)?;
        if result.is_some() {
            let view = stored_scope_view(&tx, id)?;
            return Ok(json!(view));
        }
        if p.revision != revision {
            return Err(err("STALE_REVISION", "Goal owner proposal changed"));
        }
        if p.consumed || p.expires_at_ms <= ms()? {
            return Err(denied());
        }
        let selection: GoalScopeSelection =
            serde_json::from_value(p.snapshot["intent"].clone()).map_err(|_| invalid())?;
        let refs = resolve_selection(&tx, &selection, self.data_input.as_ref())?;
        if p.snapshot["resolved"] != approval_refs(&refs)?
            || p.snapshot["resolved_digest"] != json!(sha(&refs)?)
            || p.snapshot["goal_policy"] != json!(host_policy(&selection, &refs)?)
        {
            return Err(changed());
        }
        if tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM goal_scopes WHERE task_id=?1 AND revoked=0)",
                [&selection.task_id],
                |r| r.get::<_, bool>(0),
            )
            .map_err(db_error)?
        {
            return Err(err("CONFIG_CONFLICT","Revoke the existing Goal scope before reviewing a replacement; shared counters and old actions are retained"));
        }
        for (scope, limits, consumed) in [
            (
                format!("project:{}", refs.project_id),
                &selection.project_budget,
                refs.project_consumed,
            ),
            (
                format!("root-grant:{}", refs.grant_after.id),
                &selection.root_budget,
                refs.root_consumed,
            ),
        ] {
            remember_meter(&tx, &scope, limits, consumed)?;
        }
        for (project, key_id, limits, consumed, active) in [
            (
                true,
                refs.project_id.as_str(),
                &selection.project_budget,
                refs.project_consumed,
                refs.project_active,
            ),
            (
                false,
                refs.task_id.as_str(),
                &selection.root_budget,
                refs.root_consumed,
                refs.root_active,
            ),
        ] {
            reconcile_budget(&tx, id, project, key_id, limits, consumed, active)?;
        }
        tx.execute("INSERT OR IGNORE INTO goal_grant_revisions(grant_id,revision,document,goal_approval_id) VALUES(?1,?2,?3,NULL)",params![refs.grant_before.id,sql_num(refs.grant_before.revision)?,encode(&refs.grant_before)?]).map_err(db_error)?;
        if read_grant_revision(&tx, &refs.grant_before.id, refs.grant_before.revision)?
            != refs.grant_before
        {
            return Err(changed());
        }
        if tx.execute("UPDATE execution_grants SET document=?1 WHERE id=?2 AND json_extract(document,'$.revision')=?3 AND document=?4",params![encode(&refs.grant_after)?,refs.grant_before.id,sql_num(refs.grant_before.revision)?,encode(&refs.grant_before)?]).map_err(db_error)?!=1 {return Err(err("STALE_REVISION","Root execution grant changed at Goal approval CAS"));}
        tx.execute("INSERT INTO goal_grant_revisions(grant_id,revision,document,goal_approval_id) VALUES(?1,?2,?3,?4)",params![refs.grant_after.id,sql_num(refs.grant_after.revision)?,encode(&refs.grant_after)?,id]).map_err(db_error)?;
        let mut record = ScopeRecord {
            factory_origin: None,
            id: id.into(),
            revision: 1,
            revoked: false,
            approval_id: id.into(),
            selection,
            refs,
            scope_digest: String::new(),
            created_at: now()?,
        };
        let task: Task = raw(&tx, "tasks", &record.refs.task_id)?;
        let scope = approved_scope(&tx, &task, &record)?;
        record.scope_digest = scope.digest().into();
        let view = scope_view(&tx, &record, &scope)?;
        tx.execute("INSERT INTO goal_scopes(id,task_id,root_grant_id,revision,revoked,document) VALUES(?1,?2,?3,1,0,?4)",params![id,record.refs.task_id,record.refs.grant_after.id,encode(&record)?]).map_err(db_error)?;
        p.consumed = true;
        p.revision = p.revision.checked_add(1).ok_or_else(limited)?;
        tx.execute(
            "UPDATE approval_proposals SET document=?1,result=?2 WHERE id=?3 AND result IS NULL",
            params![encode(&p)?, encode(&view)?, id],
        )
        .map_err(db_error)?;
        event(
            &tx,
            &record.refs.grant_after.id,
            record.refs.grant_after.revision,
            "execution_grant.goal_ceilings_approved",
            json!({"goal_approval_id":id,"previous_revision":record.refs.grant_before.revision,"previous_children":record.refs.grant_before.scope.max_children,"new_children":record.refs.grant_after.scope.max_children,"previous_depth":record.refs.grant_before.scope.max_depth,"new_depth":record.refs.grant_after.scope.max_depth,"shared_consumed_unchanged":true}),
        )?;
        event(
            &tx,
            id,
            1,
            "goal.scope_approved",
            json!({"scope_id":id,"task_id":record.refs.task_id,"scope_digest":record.scope_digest,"grant_revision":record.refs.grant_after.revision,"task_acceptance":"not_established","dispatched":false}),
        )?;
        event(
            &tx,
            id,
            p.revision,
            "approval.confirmed",
            json!({"kind":"goal","proposal_id":id,"consumed":true}),
        )?;
        tx.commit().map_err(db_error)?;
        Ok(json!(view))
    }
    pub fn get_goal_scope(&self, id: &str) -> Result<GoalScopeView> {
        stored_scope_view(&self.conn, id)
    }
    pub fn list_goal_scopes(&self, task_id: &str) -> Result<Vec<GoalScopeView>> {
        key(task_id)?;
        let count: i64 = self
            .conn
            .query_row(
                "SELECT count(*) FROM goal_scopes WHERE task_id=?1",
                [task_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if count > MAX_SCOPES {
            return Err(limited());
        }
        let mut q = self
            .conn
            .prepare("SELECT id FROM goal_scopes WHERE task_id=?1 ORDER BY rowid DESC LIMIT 256")
            .map_err(db_error)?;
        let ids = q
            .query_map([task_id], |r| r.get::<_, String>(0))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        ids.iter()
            .map(|id| stored_scope_view(&self.conn, id))
            .collect()
    }
    pub fn revoke_goal_scope(
        &mut self,
        id: &str,
        revision: u64,
        idempotency_key: &str,
    ) -> Result<GoalScopeView> {
        key(idempotency_key)?;
        let payload = encode(&json!({"id":id,"revision":revision}))?;
        let idem = format!("goal:{id}:revoke");
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(v) = replay(&tx, &idem, idempotency_key, &payload)? {
            return Ok(v);
        }
        let mut r: ScopeRecord = raw(&tx, "goal_scopes", id)?;
        if runtime::factory::marked_scope(&tx, id)? {
            return Err(err("AUTH_REQUIRED","Use the fixed Factory child Goal cancel route; a derived scope is not a native Goal approval"));
        }
        stored_scope_view(&tx, id)?;
        if r.revision != revision {
            return Err(err(
                "STALE_REVISION",
                "Goal scope changed before revocation",
            ));
        }
        if !r.revoked {
            r.revoked = true;
            r.revision = r.revision.checked_add(1).ok_or_else(limited)?;
            if tx.execute("UPDATE goal_scopes SET revoked=1,revision=?1,document=?2 WHERE id=?3 AND revision=?4",params![sql_num(r.revision)?,encode(&r)?,id,sql_num(revision)?]).map_err(db_error)?!=1 {return Err(changed());}
            event(
                &tx,
                id,
                r.revision,
                "goal.scope_revoked",
                json!({"scope_id":id,"new_dispatch_forbidden":true,"running_workers_not_stopped_by_preview":true}),
            )?;
        }
        let result = stored_scope_view(&tx, id)?;
        remember(&tx, &idem, idempotency_key, &payload, &result)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }
    /// Historical snapshots are never substituted with the latest grant revision.
    pub fn execution_grant_revision(&self, id: &str, revision: u64) -> Result<ExecutionGrant> {
        read_grant_revision(&self.conn, id, revision)
    }
}
fn stored_scope_view(db: &Connection, id: &str) -> Result<GoalScopeView> {
    key(id)?;
    let r: ScopeRecord = raw(db, "goal_scopes", id)?;
    if r.factory_origin.is_some() {
        return runtime::factory::historical_scope(db, id);
    }
    let columns: (i64, bool, String, String) = db
        .query_row(
            "SELECT revision,revoked,task_id,root_grant_id FROM goal_scopes WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(db_error)?;
    let (p, result) = native_confirmation(db, &r.approval_id, "goal")?;
    if r.id != id
        || r.approval_id != id
        || columns
            != (
                sql_num(r.revision)?,
                r.revoked,
                r.refs.task_id.clone(),
                r.refs.grant_after.id.clone(),
            )
        || p.snapshot["intent"] != json!(r.selection)
        || p.snapshot["resolved"] != approval_refs(&r.refs)?
        || p.snapshot["resolved_digest"] != json!(sha(&r.refs)?)
        || p.snapshot["goal_policy"] != json!(host_policy(&r.selection, &r.refs)?)
    {
        return Err(changed());
    }
    let original = read_grant_revision(db, &r.refs.grant_after.id, r.refs.grant_after.revision)?;
    if original != r.refs.grant_after
        || grant_origin(db, &original)? != r.refs.grant_confirmation_id
    {
        return Err(changed());
    }
    let view = recorded_scope_view(db, &r)?;
    let mut approved = view.clone();
    approved.revision = 1;
    approved.revoked = false;
    let mut approved = json!(approved);
    // Older schema9 confirmations predate this additive display projection.
    // Compare every original field, and validate the context exactly whenever
    // it was recorded. Do not rewrite consent or accept a null/forged context.
    if result.get("edit_context").is_none() {
        if let Some(object) = approved.as_object_mut() {
            object.remove("edit_context");
        }
    }
    if approved != result {
        return Err(changed());
    }
    let committed: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=?1 AND json_extract(document,'$.kind')='goal.scope_approved' AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.aggregate_id')=?2 AND json_extract(document,'$.revision')=1 AND json_extract(document,'$.payload.scope_id')=?2 AND json_extract(document,'$.payload.task_id')=?3 AND json_extract(document,'$.payload.scope_digest')=?4 AND json_extract(document,'$.payload.grant_revision')=?5)",
        params![MAX_STORED_BYTES,id,r.refs.task_id,r.scope_digest,sql_num(r.refs.grant_after.revision)?], |row|row.get(0),
    ).map_err(db_error)?;
    if !committed {
        return Err(changed());
    }
    Ok(view)
}

/// Verify immutable persistence/journal commitments and return data only. This
/// deliberately neither reads current artifact bytes nor constructs a private
/// ApprovedGoalScope/ValidatedPlan. New submissions and ready decisions still
/// use read_scope/current_plan with all live authority and one-shot checks.
fn historical_plan_views(db: &Connection, scope_id: &str) -> Result<Vec<GoalPlanView>> {
    let scope = stored_scope_view(db, scope_id)?;
    let head: Option<(String, i64, String)> = db
        .query_row(
            "SELECT plan_id,revision,digest FROM goal_plan_heads WHERE scope_id=?1",
            [scope_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(db_error)?;
    let (revisions,proposals):(i64,i64) = db.query_row(
        "SELECT (SELECT count(*) FROM goal_plan_revisions WHERE scope_id=?1),(SELECT count(*) FROM goal_plan_proposals WHERE scope_id=?1)",
        [scope_id], |r|Ok((r.get(0)?,r.get(1)?)),
    ).map_err(db_error)?;
    let Some((plan_id, head, head_digest)) = head else {
        let journal:bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM events WHERE json_extract(document,'$.kind')='goal.plan_validated' AND json_extract(document,'$.payload.scope_id')=?1)",
            [scope_id], |r|r.get(0),
        ).map_err(db_error)?;
        if revisions != 0 || proposals != 0 || journal {
            return Err(changed());
        }
        return Ok(vec![]);
    };
    if head > MAX_REVISIONS || revisions > MAX_REVISIONS || proposals > MAX_REVISIONS {
        return Err(limited());
    }
    if head < 1 || revisions != head || proposals != head {
        return Err(changed());
    }
    let mut q = db.prepare(
        "SELECT r.revision,r.previous_digest,r.digest,CASE WHEN octet_length(r.document)<=?2 THEN r.document END,r.created_at,p.plan_id,p.digest,CASE WHEN octet_length(p.document)<=?3 THEN p.document END,r.plan_id FROM goal_plan_revisions r LEFT JOIN goal_plan_proposals p ON p.scope_id=r.scope_id AND p.revision=r.revision WHERE r.scope_id=?1 ORDER BY r.revision LIMIT 256",
    ).map_err(db_error)?;
    let mut rows = q
        .query(params![
            scope_id,
            MAX_GOAL_PLAN_BYTES as i64,
            MAX_STORED_BYTES
        ])
        .map_err(db_error)?;
    let mut views: Vec<GoalPlanView> = vec![];
    while let Some(row) = rows.next().map_err(db_error)? {
        let revision =
            u64::try_from(row.get::<_, i64>(0).map_err(db_error)?).map_err(|_| changed())?;
        let expected = u64::try_from(views.len()).map_err(|_| limited())?;
        let previous: Option<String> = row.get(1).map_err(db_error)?;
        let digest: String = row.get(2).map_err(db_error)?;
        let document: String = row
            .get::<_, Option<String>>(3)
            .map_err(db_error)?
            .ok_or_else(limited)?;
        let created_at: String = row.get(4).map_err(db_error)?;
        let proposal_plan_id: Option<String> = row.get(5).map_err(db_error)?;
        let proposal_digest: Option<String> = row.get(6).map_err(db_error)?;
        let raw_proposal: String = row
            .get::<_, Option<String>>(7)
            .map_err(db_error)?
            .ok_or_else(limited)?;
        let revision_plan_id: String = row.get(8).map_err(db_error)?;
        let proposal: GoalPlanProposal = decode(&document)?;
        if revision != expected.checked_add(1).ok_or_else(limited)?
            || proposal.revision != revision
            || proposal.expected_revision != expected
            || proposal.plan_id != plan_id
            || revision_plan_id != plan_id
            || proposal.goal != scope.goal
            || proposal.scope_revision != scope.authority_revision
            || previous.as_deref() != views.last().map(|v| v.digest.as_str())
            || sha(&(&scope.scope_digest, &proposal))? != digest
            || proposal_digest.as_ref() != Some(&digest)
            || proposal_plan_id.as_ref() != Some(&plan_id)
        {
            return Err(changed());
        }
        let stored = match serde_json::from_str::<GoalPlanRawSubmission>(&raw_proposal) {
            Ok(raw) => {
                if raw.plan_json.len() > MAX_GOAL_PLAN_BYTES {
                    return Err(limited());
                }
                GoalPlanSubmission {
                    scope_id: raw.scope_id,
                    proposal: serde_json::from_str(&raw.plan_json).map_err(|_| changed())?,
                }
            }
            Err(_) => {
                serde_json::from_str::<GoalPlanSubmission>(&raw_proposal).map_err(|_| changed())?
            }
        };
        if stored.scope_id != scope_id || stored.proposal != proposal {
            return Err(changed());
        }
        let committed:bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=?1 AND json_extract(document,'$.kind')='goal.plan_validated' AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.aggregate_id')=?2 AND json_extract(document,'$.revision')=?3 AND json_extract(document,'$.payload.scope_id')=?4 AND json_extract(document,'$.payload.plan_id')=?2 AND json_extract(document,'$.payload.plan_revision')=?3 AND json_extract(document,'$.payload.digest')=?5 AND json_extract(document,'$.payload.previous_digest') IS ?6 AND json_extract(document,'$.payload.dispatched')=0 AND json_extract(document,'$.payload.reservation_made')=0)",
            params![MAX_STORED_BYTES,plan_id,sql_num(revision)?,scope_id,digest,previous], |r|r.get(0),
        ).map_err(db_error)?;
        if !committed {
            return Err(changed());
        }
        views.push(GoalPlanView {
            scope_id: scope_id.into(),
            proposal,
            digest,
            created_at,
            state: "validated_proposal".into(),
            dispatched: false,
        });
    }
    if views.last().is_none_or(|v| {
        v.digest != head_digest || i64::try_from(v.proposal.revision).ok() != Some(head)
    }) {
        return Err(changed());
    }
    Ok(views)
}
fn idempotency_entry(db: &Connection, scope: &str, key: &str) -> Result<Option<(String, String)>> {
    // CASE bounds the bytes materialized by this statement and distinguishes a
    // missing row from an existing over-limit payload/result without a second
    // read that could observe a different record.
    let old: Option<(Option<String>, Option<String>)> = db.query_row(
        "SELECT CASE WHEN octet_length(payload)<=?3 THEN payload END, CASE WHEN octet_length(result)<=?3 THEN result END FROM idempotency WHERE scope=?1 AND key=?2",
        params![scope, key, MAX_STORED_BYTES],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).optional().map_err(db_error)?;
    old.map(|(payload, result)| Ok((payload.ok_or_else(limited)?, result.ok_or_else(limited)?)))
        .transpose()
}
fn replay<T: for<'a> Deserialize<'a>>(
    db: &Connection,
    scope: &str,
    key: &str,
    payload: &str,
) -> Result<Option<T>> {
    let old = idempotency_entry(db, scope, key)?;
    if let Some((p, v)) = old {
        if p != payload {
            return Err(err(
                "CONFIG_CONFLICT",
                "Goal idempotency key already belongs to another exact request",
            ));
        }
        Ok(Some(decode(&v)?))
    } else {
        Ok(None)
    }
}
fn remember<T: Serialize>(
    db: &Connection,
    scope: &str,
    key: &str,
    payload: &str,
    result: &T,
) -> Result<()> {
    db.execute(
        "INSERT INTO idempotency(scope,key,payload,result) VALUES(?1,?2,?3,?4)",
        params![scope, key, payload, encode(result)?],
    )
    .map_err(db_error)?;
    Ok(())
}

fn current_plan(
    db: &Connection,
    scope_id: &str,
    scope: &ApprovedGoalScope,
) -> Result<(ValidatedPlan, GoalPlanView)> {
    if runtime::factory::marked_scope(db, scope_id)? {
        let history = historical_plan_views(db, scope_id)?;
        let view = history.last().cloned().ok_or_else(changed)?;
        let validated = ValidatedPlan::from_verified_factory_saved(scope, view.proposal.clone())
            .map_err(plan_error)?;
        if validated.digest() != view.digest {
            return Err(changed());
        }
        return Ok((validated, view));
    }
    let (plan_id, head, head_digest): (String, i64, String) = db
        .query_row(
            "SELECT plan_id,revision,digest FROM goal_plan_heads WHERE scope_id=?1",
            [scope_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(db_error)?
        .ok_or_else(|| err("NOT_FOUND", "Goal has no persisted plan"))?;
    let head = u64::try_from(head).map_err(|_| changed())?;
    let count: i64 = db
        .query_row(
            "SELECT count(*) FROM goal_plan_revisions WHERE scope_id=?1",
            [scope_id],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    if count > MAX_REVISIONS || count == 0 || u64::try_from(count).ok() != Some(head) {
        return Err(limited());
    }
    if db.query_row("SELECT EXISTS(SELECT 1 FROM goal_plan_revisions WHERE scope_id=?1 AND octet_length(document)>?2)",params![scope_id,MAX_GOAL_PLAN_BYTES as i64],|r|r.get::<_,bool>(0)).map_err(db_error)? {return Err(limited());}
    let mut q=db.prepare("SELECT revision,previous_digest,digest,document,created_at,octet_length(document) FROM goal_plan_revisions WHERE scope_id=?1 AND octet_length(document)<=262144 ORDER BY revision LIMIT 256").map_err(db_error)?;
    let mut rows = q.query([scope_id]).map_err(db_error)?;
    let mut validated: Option<ValidatedPlan> = None;
    let mut latest = None;
    while let Some(row) = rows.next().map_err(db_error)? {
        // Select raw length first; never decode a giant persisted JSON contract.
        let bytes: i64 = row.get(5).map_err(db_error)?;
        if !(0..=MAX_GOAL_PLAN_BYTES as i64).contains(&bytes) {
            return Err(limited());
        }
        let revision =
            u64::try_from(row.get::<_, i64>(0).map_err(db_error)?).map_err(|_| changed())?;
        let previous: Option<String> = row.get(1).map_err(db_error)?;
        let digest: String = row.get(2).map_err(db_error)?;
        let document: String = row.get(3).map_err(db_error)?;
        let created: String = row.get(4).map_err(db_error)?;
        let proposal: GoalPlanProposal = decode(&document)?;
        if proposal.plan_id != plan_id
            || proposal.revision != revision
            || previous.as_deref() != validated.as_ref().map(|p| p.digest())
        {
            return Err(changed());
        }
        let plan = match &validated {
            None => ValidatedPlan::initial(scope, proposal.clone()),
            Some(old) => old.revise(scope, proposal.clone()),
        }
        .map_err(plan_error)?;
        if plan.digest() != digest {
            return Err(changed());
        }
        latest = Some(GoalPlanView {
            scope_id: scope_id.into(),
            proposal,
            digest,
            created_at: created,
            state: "validated_proposal".into(),
            dispatched: false,
        });
        validated = Some(plan);
    }
    let view = latest.ok_or_else(changed)?;
    if view.digest != head_digest || view.proposal.revision != head {
        return Err(changed());
    }
    Ok((validated.ok_or_else(changed)?, view))
}
impl Store {
    pub fn submit_goal_plan_json(
        &mut self,
        submission: &GoalPlanRawSubmission,
        key_id: &str,
        revision: u64,
    ) -> Result<GoalPlanView> {
        if submission.plan_json.len() > MAX_GOAL_PLAN_BYTES {
            return Err(limited());
        }
        let proposal: GoalPlanProposal =
            serde_json::from_str(&submission.plan_json).map_err(|_| invalid())?;
        let typed = GoalPlanSubmission {
            scope_id: submission.scope_id.clone(),
            proposal,
        };
        self.submit_goal_plan_payload(&typed, key_id, revision, encode(submission)?)
    }
    /// Trusted Core typed convenience path. Renderer/Agent IPC always uses the
    /// raw-string path above; its idempotency binds the original plan text.
    pub fn submit_goal_plan(
        &mut self,
        submission: &GoalPlanSubmission,
        key_id: &str,
        revision: u64,
    ) -> Result<GoalPlanView> {
        self.submit_goal_plan_payload(submission, key_id, revision, encode(submission)?)
    }
    fn submit_goal_plan_payload(
        &mut self,
        submission: &GoalPlanSubmission,
        key_id: &str,
        revision: u64,
        payload: String,
    ) -> Result<GoalPlanView> {
        key(key_id)?;
        key(&submission.scope_id)?;
        if encode(&submission.proposal)?.len() > MAX_GOAL_PLAN_BYTES
            || payload.len() > MAX_STORED_BYTES as usize
            || submission.proposal.nodes.len() > 256
        {
            return Err(limited());
        }
        if revision != submission.proposal.expected_revision {
            return Err(err(
                "STALE_REVISION",
                "Plan CAS envelope and proposed expected revision differ",
            ));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let idem = format!("goal:{}:plan", submission.scope_id);
        if let Some(view) = replay(&tx, &idem, key_id, &payload)? {
            // Recover an already committed immutable proposal even if later
            // authority is revoked. This data is never a new ready/dispatch grant.
            return Ok(view);
        }
        let (_, scope) = match runtime_scope_for_revision(
            &tx,
            &submission.scope_id,
            self.data_input.as_ref(),
        )? {
            Some(resolved) => resolved,
            None => read_scope(&tx, &submission.scope_id, self.data_input.as_ref())?,
        };
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM goal_plan_heads WHERE scope_id=?1)",
                [&submission.scope_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let previous = if exists {
            Some(current_plan(&tx, &submission.scope_id, &scope)?.0)
        } else {
            None
        };
        let plan = match &previous {
            None => ValidatedPlan::initial(&scope, submission.proposal.clone()),
            Some(old) => old.revise(&scope, submission.proposal.clone()),
        }
        .map_err(plan_error)?;
        if plan.proposal().revision > MAX_REVISIONS as u64 {
            return Err(limited());
        }
        let view = GoalPlanView {
            scope_id: submission.scope_id.clone(),
            proposal: submission.proposal.clone(),
            digest: plan.digest().into(),
            created_at: now()?,
            state: "validated_proposal".into(),
            dispatched: false,
        };
        if let Some(old) = &previous {
            if tx.execute("UPDATE goal_plan_heads SET revision=?1,digest=?2 WHERE scope_id=?3 AND plan_id=?4 AND revision=?5 AND digest=?6",params![sql_num(view.proposal.revision)?,view.digest,view.scope_id,view.proposal.plan_id,sql_num(revision)?,old.digest()]).map_err(db_error)?!=1 {return Err(err("STALE_REVISION","Plan head changed at atomic compare-and-swap"));}
        } else {
            if revision != 0 {
                return Err(err(
                    "STALE_REVISION",
                    "First Goal plan must compare revision zero",
                ));
            }
            tx.execute(
                "INSERT INTO goal_plan_heads(scope_id,plan_id,revision,digest) VALUES(?1,?2,?3,?4)",
                params![
                    view.scope_id,
                    view.proposal.plan_id,
                    sql_num(view.proposal.revision)?,
                    view.digest
                ],
            )
            .map_err(db_error)?;
        }
        tx.execute("INSERT INTO goal_plan_proposals(id,scope_id,plan_id,revision,document,digest) VALUES(?1,?2,?3,?4,?5,?6)",params![Uuid::new_v4().to_string(),view.scope_id,view.proposal.plan_id,sql_num(view.proposal.revision)?,payload,view.digest]).map_err(db_error)?;
        tx.execute("INSERT INTO goal_plan_revisions(scope_id,plan_id,revision,previous_digest,digest,document,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![view.scope_id,view.proposal.plan_id,sql_num(view.proposal.revision)?,previous.as_ref().map(|p|p.digest()),view.digest,encode(&view.proposal)?,view.created_at]).map_err(db_error)?;
        event(
            &tx,
            &view.proposal.plan_id,
            view.proposal.revision,
            "goal.plan_validated",
            json!({"scope_id":view.scope_id,"plan_id":view.proposal.plan_id,"plan_revision":view.proposal.revision,"digest":view.digest,"previous_digest":previous.as_ref().map(|p|p.digest()),"dispatched":false,"reservation_made":false,"task_acceptance":"not_established"}),
        )?;
        remember(&tx, &idem, key_id, &payload, &view)?;
        tx.commit().map_err(db_error)?;
        Ok(view)
    }
    pub fn get_goal_plan(&self, scope_id: &str) -> Result<GoalPlanView> {
        historical_plan_views(&self.conn, scope_id)?
            .pop()
            .ok_or_else(|| err("NOT_FOUND", "Goal has no persisted plan"))
    }
    pub fn list_goal_plan_revisions(&self, scope_id: &str) -> Result<Vec<GoalPlanView>> {
        historical_plan_views(&self.conn, scope_id)
    }
    pub fn preview_goal_ready(
        &mut self,
        scope_id: &str,
        expected_revision: u64,
    ) -> Result<GoalReadyPreview> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let retained: ScopeRecord = raw(&tx, "goal_scopes", scope_id)?;
        stored_scope_view(&tx, scope_id)?;
        if unmapped_goal_history(&tx, &retained.refs.task_id, &retained.refs.grant_after.id)? {
            return blocked_history_preview(&tx, &retained, expected_revision);
        }
        let (record, scope) = read_scope(&tx, scope_id, self.data_input.as_ref())?;
        let (plan, view) = current_plan(&tx, scope_id, &scope)?;
        if view.proposal.revision != expected_revision {
            return Err(err(
                "STALE_REVISION",
                "Ready preview requested for an old plan revision",
            ));
        }
        let sequence: i64 = tx
            .query_row("SELECT COALESCE(max(sequence),0) FROM events", [], |r| {
                r.get(0)
            })
            .map_err(db_error)?;
        let sequence = u64::try_from(sequence).map_err(|_| changed())?;
        let task: Task = raw(&tx, "tasks", &record.refs.task_id)?;
        let (pc, pa, rc, ra) = counts(&tx, &task, &record.refs.grant_after)?;
        let pm = meter(
            &tx,
            &format!("project:{}", task.project_id),
            &record.selection.project_budget,
            pc,
        )?;
        let rm = meter(
            &tx,
            &format!("root-grant:{}", record.refs.grant_after.id),
            &record.selection.root_budget,
            rc,
        )?;
        let root_children:u32=tx.query_row("SELECT count(*) FROM execution_grants WHERE parent_id IS NOT NULL AND json_extract(document,'$.root_grant_id')=?1",[&record.refs.grant_after.id],|r|r.get(0)).map_err(db_error)?;
        let project_children:u32=tx.query_row("SELECT count(*) FROM execution_grants WHERE parent_id IS NOT NULL AND project_id=?1",[&record.refs.project_id],|r|r.get(0)).map_err(db_error)?;
        let usage_view = |m: &MeterLedger, attempts, active, children| GoalBudgetUsageView {
            attempts,
            active,
            children,
            runtime_ms: m.runtime_ms,
            output_bytes: m.output_bytes,
            tokens: m.tokens,
            cost_microunits: m.cost_microunits,
        };
        let mut result = GoalReadyPreview {
            scope_id: scope_id.into(),
            plan_id: view.proposal.plan_id,
            plan_revision: expected_revision,
            journal_sequence: sequence,
            read_only: true,
            reservation_made: false,
            dispatched: false,
            unknown_dimensions: vec![],
            blocked_reasons: vec![],
            decision: None,
            budget_usage: GoalBudgetUsage {
                root: usage_view(&rm, rc, ra, root_children),
                project: usage_view(&pm, pc, pa, project_children),
            },
        };
        for (name, m) in [("project", &pm), ("root", &rm)] {
            if m.runtime_ms.is_none() {
                result.unknown_dimensions.push(format!("{name}.runtime_ms"));
            }
            if m.output_bytes.is_none() {
                result
                    .unknown_dimensions
                    .push(format!("{name}.output_bytes"));
            }
            if m.tokens.is_none() {
                result.unknown_dimensions.push(format!("{name}.tokens"));
            }
            if m.cost_microunits.is_none() {
                result
                    .unknown_dimensions
                    .push(format!("{name}.cost_microunits"));
            }
        }
        if pm.runtime_ms.is_none()
            || pm.output_bytes.is_none()
            || rm.runtime_ms.is_none()
            || rm.output_bytes.is_none()
        {
            result
                .blocked_reasons
                .push("historical_runtime_or_output_ledger_unknown".into());
            return Ok(result);
        }
        let evidence:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM goal_node_evidence WHERE scope_id=?1) OR EXISTS(SELECT 1 FROM goal_external_action_journal WHERE root_grant_id=?2)",params![scope_id,record.refs.grant_after.id],|r|r.get(0)).map_err(db_error)?;
        if evidence {
            result.blocked_reasons.push(
                "goal_dispatch_evidence_requires_scheduler_reconciliation_not_json_adoption".into(),
            );
            return Ok(result);
        }
        let global:u32=tx.query_row("SELECT (SELECT count(*) FROM dispatch_intents WHERE state!='stopped')+(SELECT count(*) FROM command_validation_runs WHERE state IN('prepared','running','cancel_requested','outcome_unknown'))",[],|r|r.get(0)).map_err(db_error)?;
        let usage = |m: &MeterLedger, attempts, active| BudgetUsage {
            attempts_started: attempts,
            active_slots: active,
            runtime_ms_reserved: m.runtime_ms.unwrap_or(0),
            output_bytes_reserved: m.output_bytes.unwrap_or(0),
            tokens: m.tokens,
            cost_microunits: m.cost_microunits,
            ..Default::default()
        };
        let binding = plan.evidence_binding();
        let mut host = HostPlanSnapshot {
            goal: scope.goal().clone(),
            plan_digest: plan.digest().into(),
            scope_revision: record.refs.grant_after.revision,
            ledger_revision: sequence.max(1),
            now_ms: ms()?,
            revoked: false,
            cancelled: false,
            nodes: plan
                .proposal()
                .nodes
                .iter()
                .map(|n| {
                    (
                        n.id.clone(),
                        NodeObservation {
                            evidence: binding.clone(),
                            state: NodeState::NotStarted,
                            attempt_id: None,
                            attempts_started: 0,
                            reworks_used: 0,
                            consumed_inputs: vec![],
                        },
                    )
                })
                .collect(),
            artifacts: vec![],
            validations: vec![],
            root_usage: usage(&rm, rc, ra),
            project_usage: usage(&pm, pc, pa),
            subtree_usage: plan
                .proposal()
                .nodes
                .iter()
                .map(|n| {
                    (
                        n.id.clone(),
                        BudgetUsage {
                            tokens: Some(0),
                            cost_microunits: Some(0),
                            ..Default::default()
                        },
                    )
                })
                .collect(),
            resources: ResourceUsage {
                global,
                hosts: BTreeMap::from([("local".into(), global)]),
                providers: BTreeMap::from([("local-unverified-native-account".into(), global)]),
                workspaces: BTreeMap::new(),
            },
            unresolved_actions: vec![],
            external_actions: BTreeMap::new(),
            verified_isolation: vec![],
            verified_write_sets: vec![],
        };
        host.root_usage.children_started=tx.query_row("SELECT count(*) FROM execution_grants WHERE parent_id IS NOT NULL AND json_extract(document,'$.root_grant_id')=?1",[&record.refs.grant_after.id],|r|r.get(0)).map_err(db_error)?;
        host.project_usage.children_started=tx.query_row("SELECT count(*) FROM execution_grants WHERE parent_id IS NOT NULL AND project_id=?1",[&record.refs.project_id],|r|r.get(0)).map_err(db_error)?;
        // Actual bounded local bytes were rechecked by read_scope, not inferred
        // from a serialized passed flag or from provenance approval alone.
        let policy = host_policy(&record.selection, &record.refs)?;
        for i in &record.refs.inputs {
            host.validations.push(ValidationReceipt {
                evidence: binding.clone(),
                input: ResolvedInput {
                    artifact_id: i.artifact.artifact_id.clone(),
                    contract_id: format!("input:{}", i.artifact.artifact_id),
                    source: ResolvedInputSource::ApprovedExternal {
                        scope_revision: record.refs.grant_after.revision,
                        sha256: i.sha256.clone(),
                    },
                },
                rule_id: INPUT_RULE.into(),
                policy_digest: policy.rules[INPUT_RULE].policy_digest.clone(),
                passed: true,
            });
        }
        // New Goal nodes have no dispatched attempts: existing historical
        // sessions are not mapped to nodes or adopted as successful results.
        let trusted = TrustedPlanSnapshot::from_verified_host(&plan, host).map_err(plan_error)?;
        result.decision = Some(decide_ready(&plan, &trusted).map_err(plan_error)?);
        Ok(result)
    }
}

fn blocked_history_preview(
    db: &Connection,
    record: &ScopeRecord,
    expected: u64,
) -> Result<GoalReadyPreview> {
    let task: Task = raw(db, "tasks", &record.refs.task_id)?;
    let grant: ExecutionGrant = raw(db, "execution_grants", &record.refs.grant_after.id)?;
    if record.revoked
        || grant.revoked
        || grant.expires_at_ms <= ms()?
        || matches!(
            task.state,
            TaskState::Accepted
                | TaskState::Cancelled
                | TaskState::CancelRequested
                | TaskState::OutcomeUnknown
        )
    {
        return Err(denied());
    }
    let (plan_id, revision): (String, i64) = db
        .query_row(
            "SELECT plan_id,revision FROM goal_plan_heads WHERE scope_id=?1",
            [&record.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(db_error)?
        .ok_or_else(|| err("NOT_FOUND", "Goal has no persisted plan"))?;
    if u64::try_from(revision).ok() != Some(expected) {
        return Err(err(
            "STALE_REVISION",
            "Ready preview requested for an old plan revision",
        ));
    }
    let sequence: i64 = db
        .query_row("SELECT COALESCE(max(sequence),0) FROM events", [], |r| {
            r.get(0)
        })
        .map_err(db_error)?;
    let (pc, pa, rc, ra) = counts(db, &task, &grant)?;
    let pm = meter(
        db,
        &format!("project:{}", task.project_id),
        &record.selection.project_budget,
        pc,
    )?;
    let rm = meter(
        db,
        &format!("root-grant:{}", grant.id),
        &record.selection.root_budget,
        rc,
    )?;
    let root_children=db.query_row("SELECT count(*) FROM execution_grants WHERE parent_id IS NOT NULL AND json_extract(document,'$.root_grant_id')=?1",[&grant.id],|r|r.get(0)).map_err(db_error)?;
    let project_children = db
        .query_row(
            "SELECT count(*) FROM execution_grants WHERE parent_id IS NOT NULL AND project_id=?1",
            [&task.project_id],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    let view = |m: &MeterLedger, attempts, active, children| GoalBudgetUsageView {
        attempts,
        active,
        children,
        runtime_ms: m.runtime_ms,
        output_bytes: m.output_bytes,
        tokens: m.tokens,
        cost_microunits: m.cost_microunits,
    };
    let mut unknown_dimensions = vec![];
    for (n, m) in [("root", &rm), ("project", &pm)] {
        for (d, value) in [
            ("runtime_ms", m.runtime_ms),
            ("output_bytes", m.output_bytes),
            ("tokens", m.tokens),
            ("cost_microunits", m.cost_microunits),
        ] {
            if value.is_none() {
                unknown_dimensions.push(format!("{n}.{d}"));
            }
        }
    }
    Ok(GoalReadyPreview {
        scope_id: record.id.clone(),
        plan_id,
        plan_revision: expected,
        journal_sequence: u64::try_from(sequence).map_err(|_| changed())?,
        read_only: true,
        reservation_made: false,
        dispatched: false,
        unknown_dimensions,
        blocked_reasons: vec![
            "goal_dispatch_evidence_requires_scheduler_reconciliation_not_json_adoption".into(),
        ],
        decision: None,
        budget_usage: GoalBudgetUsage {
            root: view(&rm, rc, ra, root_children),
            project: view(&pm, pc, pa, project_children),
        },
    })
}

pub(crate) use runtime::factory::verify_native_history_before_migration as verify_native_goal_history_before_factory_migration;

pub(crate) use runtime::factory::budget_records as factory_goal_budget_records;

pub(crate) use runtime::factory::session_root as factory_goal_session_root;
