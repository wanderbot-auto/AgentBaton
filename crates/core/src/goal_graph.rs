//! Versioned execution of the existing GoalPlan/Scope model. Graph approval is
//! separate from legacy candidate-only execution and per-attempt validation.
use super::*;
#[path = "goal_evidence.rs"]
mod evidence;
pub use evidence::{
    GraphArtifactReference, GraphCandidateReceipt, GraphInputBinding, GraphOwnerDecision,
    GraphValidationReceipt, GRAPH_CONTRACT_VERSION, MAX_GRAPH_REVIEW_BYTES,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphRunSelection {
    pub scope_id: String,
    pub expected_plan_revision: u64,
    pub expected_ledger_revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphNodeApprovalSelection {
    pub run_id: String,
    pub node_id: String,
    pub expected_plan_revision: u64,
    pub expected_ledger_revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphNodeView {
    pub node: PlanNode,
    pub plan_revision: u64,
    pub state: String,
    pub session_id: Option<String>,
    pub attempt_id: Option<String>,
    pub job_id: Option<String>,
    pub execution_kind: String,
    pub acceptance_state: String,
    pub stop_confirmed: bool,
    pub actual_runtime_ms: Option<u64>,
    pub actual_output_bytes: Option<u64>,
    pub tokens: Option<u64>,
    pub cost_microunits: Option<u64>,
    pub candidate_ids: Vec<String>,
    pub validation_ids: Vec<String>,
    pub owner_decision_ids: Vec<String>,
    pub blocked_reason: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphRunSummary {
    pub graph_contract_version: u32,
    pub run: GoalRun,
    pub ledger_revision: u64,
    pub task_accepted: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphRunDetails {
    pub graph_contract_version: u32,
    pub run: GoalRun,
    pub nodes: Vec<GraphNodeView>,
    pub candidates: Vec<GraphCandidateReceipt>,
    pub validations: Vec<GraphValidationReceipt>,
    pub owner_decisions: Vec<GraphOwnerDecision>,
    pub ledger_revision: u64,
    pub task_accepted: bool,
    pub task_acceptance: Option<GraphTaskAcceptance>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphOwnerDecisionResult {
    pub graph_contract_version: u32,
    pub approval_id: String,
    pub run_id: String,
    pub node_id: String,
    pub plan_revision: u64,
    pub decision: String,
    pub owner_decision_id: String,
    pub run_revision: u64,
    pub task_accepted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphTaskAcceptance {
    pub id: String,
    pub graph_contract_version: u32,
    pub task_id: String,
    pub task_revision: u64,
    pub spec_revision: u64,
    pub project_id: String,
    pub run_id: String,
    pub scope_id: String,
    pub root_grant_id: String,
    pub plan_revision: u64,
    pub plan_digest: String,
    pub input_revision: String,
    pub workspace_id: String,
    pub capability_digest: String,
    pub criterion_digest: String,
    pub artifact_refs: Vec<GraphArtifactReference>,
    pub validation_ids: Vec<String>,
    pub owner_decision_ids: Vec<String>,
    pub accepted_at: String,
    pub sequence: u64,
    pub source: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct GraphRunBinding {
    pub run_id: String,
    pub scope_id: String,
    pub task_id: String,
    pub project_id: String,
    pub root_grant_id: String,
    pub plan_revision: u64,
    pub plan_digest: String,
    pub scope_digest: String,
    pub spec_revision: u64,
    pub input_revision: String,
    pub workspace_id: String,
    pub workspace: agentbaton_workspaces::WorkspaceIdentity,
    pub capability_digest: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct GraphCriterionBinding {
    pub rule_id: String,
    pub criterion_text: String,
    pub criterion_digest: String,
    pub artifact_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct GraphNodeAcceptanceSnapshot {
    pub graph_contract_version: u32,
    pub intent: GraphNodeApprovalSelection,
    pub run_binding: GraphRunBinding,
    pub node: PlanNode,
    pub criterion: GraphCriterionBinding,
    pub candidate_refs: Vec<GraphArtifactReference>,
    pub validation_refs: Vec<GraphValidationReceipt>,
    pub input_bindings: Vec<GraphInputBinding>,
    pub capability_digest: String,
    pub expected_run_revision: u64,
    pub expected_ledger_revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphDefinitionReusePermission {
    pub rule_id: String,
    pub source: GoalValidatorRef,
    pub definition_digest: String,
    pub definition: Value,
    pub allowed_actions: Vec<String>,
    pub artifact_ids: Vec<String>,
    pub goal_acceptance: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct GraphExecutionSnapshot {
    pub graph_contract_version: u32,
    pub intent: GraphRunSelection,
    pub scope: GoalScopeView,
    pub root_grant: ExecutionGrant,
    pub existing_connection: Value,
    pub plan: GoalPlanView,
    pub definition_reuse: Vec<GraphDefinitionReusePermission>,
    pub subject_rebinding: Value,
    pub budgets: Value,
    pub enforcement: Value,
    pub io_limits: Value,
    pub new_workspace_authority: bool,
    pub allow_in_scope_plan_revisions: bool,
    pub execution: String,
    pub warning: String,
}

pub(crate) fn migrate(db: &mut Connection, version: u32) -> Result<()> {
    if version < 12 {
        let tx = db.transaction().map_err(db_error)?;
        tx.execute_batch("CREATE TABLE goal_graph_capabilities(run_id TEXT PRIMARY KEY REFERENCES goal_runs(id),approval_id TEXT NOT NULL UNIQUE REFERENCES approval_proposals(id),version INTEGER NOT NULL CHECK(version=1),document TEXT NOT NULL);
CREATE TABLE goal_graph_jobs(id TEXT PRIMARY KEY,run_id TEXT NOT NULL REFERENCES goal_runs(id),node_id TEXT NOT NULL,plan_revision INTEGER NOT NULL CHECK(plan_revision>0),state TEXT NOT NULL,attempt_id TEXT REFERENCES attempts(id),workspace_id TEXT NOT NULL REFERENCES managed_workspaces(id),document TEXT NOT NULL);
CREATE TABLE goal_graph_candidates(id TEXT PRIMARY KEY,run_id TEXT NOT NULL REFERENCES goal_runs(id),node_id TEXT NOT NULL,plan_revision INTEGER NOT NULL CHECK(plan_revision>0),attempt_id TEXT NOT NULL REFERENCES attempts(id),document TEXT NOT NULL);
CREATE TABLE goal_graph_validations(id TEXT PRIMARY KEY,run_id TEXT NOT NULL REFERENCES goal_runs(id),node_id TEXT NOT NULL,plan_revision INTEGER NOT NULL CHECK(plan_revision>0),attempt_id TEXT NOT NULL REFERENCES attempts(id),document TEXT NOT NULL);
CREATE TABLE goal_graph_owner_decisions(id TEXT PRIMARY KEY,approval_id TEXT NOT NULL UNIQUE REFERENCES approval_proposals(id),run_id TEXT NOT NULL REFERENCES goal_runs(id),node_id TEXT NOT NULL,plan_revision INTEGER NOT NULL CHECK(plan_revision>0),document TEXT NOT NULL);
CREATE TABLE goal_graph_acceptances(id TEXT PRIMARY KEY,task_id TEXT NOT NULL REFERENCES tasks(id),task_revision INTEGER NOT NULL,run_id TEXT NOT NULL REFERENCES goal_runs(id),plan_revision INTEGER NOT NULL CHECK(plan_revision>0),document TEXT NOT NULL,UNIQUE(task_id,task_revision));
PRAGMA user_version=12;").map_err(db_error)?;
        tx.commit().map_err(db_error)?;
    }
    Ok(())
}
fn definition_permissions(record: &ScopeRecord) -> Result<Vec<GraphDefinitionReusePermission>> {
    let mut definitions = Vec::new();
    for validator in &record.refs.validators {
        let action = match validator.reference.kind {
            GoalValidatorKind::Integrity => "artifact_integrity_only",
            GoalValidatorKind::Data => "data_contract_only",
            GoalValidatorKind::Command => "approved_deterministic_command_only",
        };
        definitions.push(GraphDefinitionReusePermission {
            rule_id: validator.rule_id.clone(),
            source: validator.reference.clone(),
            definition_digest: validator.digest.clone(),
            definition: validator.definition.clone(),
            allowed_actions: vec![action.into()],
            artifact_ids: record
                .selection
                .outputs
                .iter()
                .filter(|o| o.validators.contains(&validator.reference))
                .map(|o| o.artifact.artifact_id.clone())
                .collect(),
            goal_acceptance: false,
        });
    }
    Ok(definitions)
}
fn graph_snapshot(
    db: &Connection,
    s: &GraphRunSelection,
    data: Option<&crate::data_workspaces::DataInputWitness>,
) -> Result<Value> {
    let legacy = GoalRunSelection {
        scope_id: s.scope_id.clone(),
        expected_plan_revision: s.expected_plan_revision,
        expected_ledger_revision: s.expected_ledger_revision,
    };
    let mut value = snapshot_base(db, &legacy, data)?;
    let record: ScopeRecord = raw(db, "goal_scopes", &s.scope_id)?;
    value["graph_contract_version"] = json!(GRAPH_CONTRACT_VERSION);
    value["definition_reuse"] = json!(definition_permissions(&record)?);
    value["subject_rebinding"] = json!({"mode":"this_run_current_plan_owned_producer_only","project_id":record.refs.project_id,"workspace_id":record.refs.workspace.id,"root_grant_id":record.refs.grant_after.id,"artifact_ids":record.selection.outputs.iter().map(|o|o.artifact.artifact_id.clone()).collect::<Vec<_>>(),"requires_current_scope_and_inputs":true,"requires_owned_stop_and_capture":true,"requires_current_definition_digest":true,"allows_legacy_candidate_backfill":false,"allows_old_policy_execution_reuse":false,"allows_new_workspace":false});
    value["budgets"] = json!({"root":record.selection.root_budget,"project":record.selection.project_budget,"usage":blocked_history_preview(db,&record,s.expected_plan_revision)?.budget_usage});
    value["io_limits"] = json!({"max_total_artifact_bytes":evidence::MAX_GRAPH_IO_BYTES,"max_artifact_refs":64,"worker_within_attempt_runtime_reservation":true,"cancel_requires_actual_join":true,"timeout_without_join_is_unknown":true});
    value["execution"] = json!("goal_graph_v1_existing_workspace");
    value["warning"]=json!("Approve this existing GoalPlan/Scope graph and bounded later validated same-scope versions, including explicitly listed validator-definition reuse for new owned producer subjects. Old per-attempt policies are not reused as execution permissions. Each actual dispatch keeps current input/definition/authority checks and shared cumulative budgets. Artifact integrity, JSON/data contracts and fixed command contracts do not establish GoalAcceptance. Exact private Owner decisions are separate. No new workspace/host/identity/recipient/billing or production capability; no OS filesystem/network sandbox or output quota is claimed. Unknown effects, unjoined workers and overrun block execution. Complete review is bounded, never truncated.");
    Ok(value)
}
fn proposal_scope(db: &Connection, scope_id: &str) -> Result<String> {
    key(scope_id)?;
    let record: ScopeRecord = raw(db, "goal_scopes", scope_id)?;
    if record.id != scope_id {
        return Err(changed());
    }
    Ok(format!(
        "local:{}:goal_graph_runs.propose",
        record.refs.project_id
    ))
}
impl Store {
    pub(crate) fn graph_execution_proposal_replay(
        &self,
        s: &GraphRunSelection,
        key_id: &str,
    ) -> Result<Option<ApprovalProposal>> {
        key(key_id)?;
        let scope = proposal_scope(&self.conn, &s.scope_id)?;
        let Some((payload, result)) = idempotency_entry(&self.conn, &scope, key_id)? else {
            return Ok(None);
        };
        if payload != encode(s)? {
            return Err(err(
                "CONFIG_CONFLICT",
                "Graph proposal key belongs to another exact intent",
            ));
        }
        let original: ApprovalProposal = decode(&result)?;
        let current: ApprovalProposal = raw(&self.conn, "approval_proposals", &original.id)?;
        if current.id != original.id
            || current.kind != "goal_graph_execution"
            || current.revision != original.revision
            || current.snapshot != original.snapshot
        {
            return Err(changed());
        }
        Ok(Some(current))
    }
    pub fn propose_goal_graph_run(
        &mut self,
        s: &GraphRunSelection,
        key_id: &str,
    ) -> Result<ApprovalProposal> {
        if let Some(old) = self.graph_execution_proposal_replay(s, key_id)? {
            return Ok(old);
        }
        let snapshot = graph_snapshot(&self.conn, s, self.data_input.as_ref())?;
        let scope = proposal_scope(&self.conn, &s.scope_id)?;
        let proposal = ApprovalProposal {
            id: Uuid::new_v4().to_string(),
            kind: "goal_graph_execution".into(),
            revision: 1,
            snapshot,
            created_at: now()?,
            expires_at_ms: ms()?.checked_add(300000).ok_or_else(limited)?,
            consumed: false,
        };
        let proposed = Event {
            sequence: 0,
            event_id: Uuid::new_v4().to_string(),
            aggregate_id: proposal.id.clone(),
            revision: 1,
            kind: "approval.proposed".into(),
            occurred_at: now()?,
            payload: json!({"kind":proposal.kind,"snapshot":proposal.snapshot}),
            source: "user_action".into(),
        };
        let document = encode(&proposal)?;
        let event_document = encode(&proposed)?;
        let worst_response = encode(
            &json!({"request_id":"x".repeat(128),"result":proposal,"revision":u64::MAX,"warnings":[]}),
        )?;
        if [document.len(), event_document.len(), worst_response.len()]
            .iter()
            .any(|n| *n > MAX_GRAPH_REVIEW_BYTES)
        {
            return Err(err("RESOURCE_LIMIT","Complete graph approval row and review envelope exceed 1MiB; nothing was truncated"));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if sequence(&tx)? != s.expected_ledger_revision {
            return Err(err(
                "STALE_REVISION",
                "Graph ledger changed before proposal commit",
            ));
        }
        tx.execute(
            "INSERT INTO approval_proposals(id,document) VALUES(?1,?2)",
            params![proposal.id, document],
        )
        .map_err(db_error)?;
        tx.execute(
            "INSERT INTO idempotency VALUES(?1,?2,?3,?4)",
            params![scope, key_id, encode(s)?, document],
        )
        .map_err(db_error)?;
        tx.execute("INSERT INTO events(document) VALUES(?1)", [event_document])
            .map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(proposal)
    }
    pub fn list_goal_graph_runs(&self, task_id: &str) -> Result<Vec<GraphRunSummary>> {
        key(task_id)?;
        raw_collection_budget(&self.conn,"SELECT count(*),COALESCE(sum(octet_length(r.document)+octet_length(r.id)+octet_length(g.document)),0),COALESCE(max(octet_length(r.document)),0) FROM goal_runs r JOIN goal_graph_capabilities g ON g.run_id=r.id WHERE r.task_id=?1",[task_id],256,MAX_STORED_BYTES)?;
        let mut q=self.conn.prepare("SELECT r.id FROM goal_runs r JOIN goal_graph_capabilities g ON g.run_id=r.id WHERE r.task_id=?1 ORDER BY r.rowid DESC LIMIT 257").map_err(db_error)?;
        let ids = q
            .query_map([task_id], |r| r.get::<_, String>(0))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        ids.iter().map(|id| self.goal_graph_summary(id)).collect()
    }
    pub fn goal_graph_summary(&self, id: &str) -> Result<GraphRunSummary> {
        let _r = run(&self.conn, id)?;
        let capability:Option<String>=self.conn.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM goal_graph_capabilities WHERE run_id=?1",[id],|row|row.get(0)).optional().map_err(db_error)?.flatten();
        if capability.is_none() {
            return Err(err("CAPABILITY_UNSUPPORTED","This run has no authenticated graph execution capability; legacy permissions were not upgraded"));
        }
        let _verified: Value = decode(&capability.ok_or_else(changed)?)?;
        // Graph finalization is connected in the next dependent increment. There
        // are intentionally no constructed active graph rows in this stage.
        Err(err("CAPABILITY_UNSUPPORTED","Graph execution capability verification is not yet connected; no dispatch or acceptance was performed"))
    }
    pub fn goal_graph_details(&self, id: &str) -> Result<GraphRunDetails> {
        let _summary = self.goal_graph_summary(id)?;
        Err(err(
            "CAPABILITY_UNSUPPORTED",
            "Graph evidence execution is not yet connected",
        ))
    }
}
