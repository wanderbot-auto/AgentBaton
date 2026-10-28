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
    pub candidate_receipts: Vec<GraphCandidateReceipt>,
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

/// Presence is enough to reject a legacy interpretation, never to grant graph
/// authority. Positive graph access separately verifies its immutable capability.
pub(super) fn is_graph_run(db: &Connection, id: &str) -> Result<bool> {
    key(id)?;
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM goal_graph_capabilities WHERE run_id=?1)",
        [id],
        |row| row.get(0),
    )
    .map_err(db_error)
}
pub(super) fn require_legacy_run(db: &Connection, id: &str) -> Result<()> {
    if is_graph_run(db, id)? {
        return Err(err("CAPABILITY_UNSUPPORTED", "This is a versioned graph run; legacy candidate-only methods cannot read, replay or mutate its authority"));
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GraphCapability {
    version: u32,
    run_id: String,
    approval_id: String,
    snapshot_digest: String,
}
pub(super) fn checked_capability(db: &Connection, r: &GoalRun) -> Result<ApprovalProposal> {
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(run_id)+octet_length(approval_id)),0),COALESCE(max(octet_length(document)),0) FROM goal_graph_capabilities WHERE run_id=?1",[&r.id],1,MAX_STORED_BYTES)?;
    let stored: Option<Option<String>> = db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM goal_graph_capabilities WHERE run_id=?1",[&r.id],|row|row.get(0)).optional().map_err(db_error)?;
    let cap: GraphCapability = decode(&stored.ok_or_else(denied)?.ok_or_else(limited)?)?;
    let (p, result) = native_confirmation(db, &r.id, "goal_graph_execution")?;
    if cap.version != GRAPH_CONTRACT_VERSION
        || cap.run_id != r.id
        || cap.approval_id != p.id
        || cap.snapshot_digest != sha(&p.snapshot)?
        || result["graph_contract_version"] != GRAPH_CONTRACT_VERSION
        || result["run"]["id"] != r.id
        || p.snapshot["execution"] != "goal_graph_v1_existing_workspace"
        || p.snapshot["graph_contract_version"] != GRAPH_CONTRACT_VERSION
        || p.snapshot["new_workspace_authority"] != false
    {
        return Err(changed());
    }
    let initial = &result["run"];
    if initial["scope_id"] != r.scope_id
        || initial["task_id"] != r.task_id
        || initial["project_id"] != r.project_id
        || initial["root_grant_id"] != r.root_grant_id
        || initial["plan_digest"] != p.snapshot["plan"]["digest"]
        || initial["plan_revision"] != p.snapshot["plan"]["proposal"]["revision"]
    {
        return Err(changed());
    }
    let confirmation:Option<i64>=db.query_row("SELECT sequence FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='approval.confirmed' AND json_extract(document,'$.source')='local_owner_confirmation' AND json_extract(document,'$.payload.kind')='goal_graph_execution' AND json_extract(document,'$.payload.run_id')=?1 AND json_extract(document,'$.payload.execution_snapshot_digest')=?2 ORDER BY sequence LIMIT 1",params![r.id,cap.snapshot_digest],|row|row.get(0)).optional().map_err(db_error)?;
    let authorized:Option<i64>=db.query_row("SELECT sequence FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.run_authorized' AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.payload.scope_id')=?2 AND json_extract(document,'$.payload.plan_digest')=?3 AND json_extract(document,'$.payload.plan_revision')=?4 AND json_extract(document,'$.payload.execution_snapshot_digest')=?5 AND json_extract(document,'$.payload.reservation_made')=0 AND json_extract(document,'$.payload.dispatched')=0 ORDER BY sequence LIMIT 1",params![r.id,r.scope_id,initial["plan_digest"].as_str().ok_or_else(changed)?,initial["plan_revision"].as_i64().ok_or_else(changed)?,cap.snapshot_digest],|row|row.get(0)).optional().map_err(db_error)?;
    if !matches!((confirmation,authorized),(Some(a),Some(b)) if a>0&&b>a) {
        return Err(denied());
    }
    verify_plan_advances(db, r, &p, initial, authorized.ok_or_else(denied)?)?;
    Ok(p)
}

pub(super) fn persist_capability(db: &Connection, p: &ApprovalProposal) -> Result<()> {
    let cap = GraphCapability {
        version: GRAPH_CONTRACT_VERSION,
        run_id: p.id.clone(),
        approval_id: p.id.clone(),
        snapshot_digest: sha(&p.snapshot)?,
    };
    db.execute(
        "INSERT INTO goal_graph_capabilities VALUES(?1,?1,1,?2)",
        params![p.id, encode(&cap)?],
    )
    .map_err(db_error)?;
    Ok(())
}
pub(super) fn check_finalization_snapshot(
    db: &Connection,
    p: &ApprovalProposal,
    data: Option<&crate::data_workspaces::DataInputWitness>,
) -> Result<()> {
    let original: GraphRunSelection =
        serde_json::from_value(p.snapshot["intent"].clone()).map_err(|_| changed())?;
    let current = GraphRunSelection {
        expected_ledger_revision: sequence(db)?,
        ..original.clone()
    };
    let mut expected = graph_snapshot(db, &current, data)?;
    expected["intent"] = json!(original);
    if expected != p.snapshot {
        return Err(changed());
    }
    Ok(())
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
CREATE TRIGGER graph_fence_lease_insert BEFORE INSERT ON workspace_leases WHEN EXISTS(SELECT 1 FROM goal_graph_jobs WHERE workspace_id=NEW.resource AND state IN('prepared','running','cancel_requested','outcome_unknown')) BEGIN SELECT RAISE(ABORT,'graph workspace remains fenced'); END;
CREATE TRIGGER graph_fence_lease_update BEFORE UPDATE ON workspace_leases WHEN EXISTS(SELECT 1 FROM goal_graph_jobs WHERE workspace_id=NEW.resource AND state IN('prepared','running','cancel_requested','outcome_unknown')) BEGIN SELECT RAISE(ABORT,'graph workspace remains fenced'); END;
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
            &json!({"request_id":"\0".repeat(128),"result":proposal,"revision":u64::MAX,"warnings":[]}),
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
    pub(crate) fn finalize_goal_graph_run(&mut self, id: &str, revision: u64) -> Result<Value> {
        self.finalize_goal_run_kind(id, revision, true)
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
        let r = run(&self.conn, id)?;
        checked_capability(&self.conn, &r)?;
        Ok(GraphRunSummary {
            graph_contract_version: GRAPH_CONTRACT_VERSION,
            task_accepted: r.task_accepted,
            run: r,
            ledger_revision: sequence(&self.conn)?,
        })
    }
    pub fn goal_graph_details(&self, id: &str) -> Result<GraphRunDetails> {
        let summary = self.goal_graph_summary(id)?;
        let candidates = graph_candidates(&self.conn, &summary.run)?;
        let validations = graph_validations(&self.conn, &summary.run)?;
        let owners = graph_owner_decisions(&self.conn, &summary.run)?;
        let nodes = self
            .goal_node_history(&summary.run)?
            .into_iter()
            .map(|n| GraphNodeView {
                execution_kind: match n.node.executor {
                    Executor::Connection { .. } => "agent",
                    Executor::Validator { .. } => "validator",
                    Executor::Owner { .. } => "owner",
                }
                .into(),
                node: n.node.clone(),
                plan_revision: n.plan_revision,
                state: n.state,
                session_id: n.session_id,
                attempt_id: n.attempt_id,
                job_id: None,
                acceptance_state: "pending".into(),
                stop_confirmed: n.stop_confirmed,
                actual_runtime_ms: n.actual_runtime_ms,
                actual_output_bytes: n.actual_output_bytes,
                tokens: n.tokens,
                cost_microunits: n.cost_microunits,
                candidate_ids: candidates
                    .iter()
                    .filter(|c| c.node_id == n.node.id && c.plan_revision == n.plan_revision)
                    .map(|c| c.id.clone())
                    .collect(),
                validation_ids: validations
                    .iter()
                    .filter(|v| v.node_id == n.node.id && v.plan_revision == n.plan_revision)
                    .map(|v| v.id.clone())
                    .collect(),
                owner_decision_ids: owners
                    .iter()
                    .filter(|o| o.node_id == n.node.id && o.plan_revision == n.plan_revision)
                    .map(|o| o.id.clone())
                    .collect(),
                blocked_reason: None,
            })
            .collect();
        let task_acceptance = if summary.task_accepted {
            Some(verify_accepted_state(&self.conn, &summary.run)?)
        } else {
            None
        };
        let details = GraphRunDetails {
            graph_contract_version: GRAPH_CONTRACT_VERSION,
            run: summary.run,
            nodes,
            candidates,
            validations,
            owner_decisions: owners,
            ledger_revision: summary.ledger_revision,
            task_accepted: summary.task_accepted,
            task_acceptance,
        };
        if encode(&details)?.len() > 4_194_304 {
            return Err(limited());
        }
        Ok(details)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CaptureOrigin {
    id: String,
    run_id: String,
    node_id: String,
    plan_revision: u64,
    plan_digest: String,
    attempt_id: String,
    session_id: String,
    workspace_id: String,
    root: agentbaton_workspaces::WorkspaceIdentity,
    writer_epoch: u64,
    stop_receipt_sequence: u64,
    capability_digest: String,
    elapsed_at_stop_ms: u64,
    prepared_at_ms: i64,
    runtime_reservation_ms: u64,
    consumed_inputs: Vec<GraphInputBinding>,
}
struct CaptureWorker {
    origin: CaptureOrigin,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: std::thread::JoinHandle<Result<evidence::CapturedCandidate>>,
    deadline: std::time::Instant,
    unknown_recorded: bool,
    queued_at: std::time::Instant,
}
#[derive(Default)]
pub(crate) struct GraphWorkers {
    captures: std::collections::HashMap<String, CaptureWorker>,
    stopping: bool,
}
impl GraphWorkers {
    pub(crate) fn has_workers(&self) -> bool {
        !self.captures.is_empty()
    }
    pub(crate) fn tick(&mut self, store: &mut Store) -> Result<()> {
        let finished: Vec<_> = self
            .captures
            .iter()
            .filter(|(_, w)| w.thread.is_finished())
            .map(|(id, _)| id.clone())
            .collect();
        for id in finished {
            let w = self.captures.remove(&id).ok_or_else(changed)?;
            let result = w
                .thread
                .join()
                .map_err(|_| {
                    err(
                        "OUTCOME_UNKNOWN",
                        "Owned evidence worker panicked after an unknown partial observation",
                    )
                })
                .and_then(|r| r);
            store.finish_graph_capture(
                &w.origin,
                result,
                w.unknown_recorded,
                w.queued_at.elapsed(),
            )?;
        }
        for w in self.captures.values_mut() {
            let r = run(&store.conn, &w.origin.run_id)?;
            if r.cancel_requested
                || crate::policy::checked_lineage(&store.conn, &r.root_grant_id).is_err()
            {
                w.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            if std::time::Instant::now() > w.deadline && !w.unknown_recorded {
                w.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                store.mark_graph_capture_unknown(&w.origin)?;
                w.unknown_recorded = true;
            }
        }
        if self.has_workers() || self.stopping {
            return Ok(());
        }
        store.reconcile_graph_frontiers()?;
        raw_collection_budget(&store.conn,"SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)+octet_length(state)+octet_length(workspace_id)),0),COALESCE(max(octet_length(document)),0) FROM goal_graph_jobs WHERE state='prepared'",[],1,MAX_STORED_BYTES)?;
        let doc: Option<String> = store
            .conn
            .query_row(
                "SELECT document FROM goal_graph_jobs WHERE state='prepared' LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?;
        let Some(doc) = doc else { return Ok(()) };
        let origin: CaptureOrigin = decode(&doc)?;
        let Some(queued_at) = store.graph_capture_clocks.remove(&origin.id) else {
            store.mark_graph_capture_unknown(&origin)?;
            return Ok(());
        };
        let prepared = match store.prepare_graph_capture_worker(&origin) {
            Ok(p) => p,
            Err(error) => {
                store.finish_graph_capture(&origin, Err(error), false, queued_at.elapsed())?;
                return Ok(());
            }
        };
        let cancel = prepared.cancelled.clone();
        let deadline = prepared.deadline;
        let thread = std::thread::Builder::new()
            .name("baton-goal-evidence".into())
            .spawn(move || prepared.execute());
        match thread {
            Ok(thread) => {
                self.captures.insert(
                    origin.id.clone(),
                    CaptureWorker {
                        origin,
                        cancel,
                        thread,
                        deadline,
                        unknown_recorded: false,
                        queued_at,
                    },
                );
            }
            Err(_) => store.finish_graph_capture(
                &origin,
                Err(err(
                    "RESOURCE_LIMIT",
                    "Owned graph evidence worker could not start",
                )),
                false,
                queued_at.elapsed(),
            )?,
        }
        Ok(())
    }
    pub(crate) fn shutdown(&mut self, store: &mut Store) -> Result<()> {
        self.stopping = true;
        for w in self.captures.values() {
            w.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        // Only drain owned handles; never admit a queued job during shutdown.
        // If the OS read does not join, retain its handle until daemon/process
        // teardown and leave durable unknown/fence/no-replay, never a join receipt.
        let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while self.has_workers() && std::time::Instant::now() < until {
            self.tick(store)?;
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        for w in self.captures.values_mut() {
            if !w.unknown_recorded {
                store.mark_graph_capture_unknown(&w.origin)?;
                w.unknown_recorded = true;
            }
        }
        Ok(())
    }
}
impl Store {
    pub(super) fn queue_graph_capture(
        &mut self,
        session_id: &str,
        elapsed_at_stop_ms: u64,
    ) -> Result<()> {
        let s = self.get_session(session_id)?;
        let (run_id,doc):(String,String)=self.conn.query_row("SELECT run_id,document FROM goal_run_nodes WHERE session_id=?1 AND octet_length(document)<=262144",[session_id],|row|Ok((row.get(0)?,row.get(1)?))).map_err(db_error)?;
        let r = run(&self.conn, &run_id)?;
        let p = checked_capability(&self.conn, &r)?;
        let n: RuntimeNode = decode(&doc)?;
        let stop = trusted_session_stop(&self.conn, &r.id, &s)?;
        let record: ScopeRecord = raw(&self.conn, "goal_scopes", &r.scope_id)?;
        if !current_output_root(&self.conn, &record.refs.workspace) {
            return Err(changed());
        }
        let lease = self.workspace_lease(&s.workspace_id)?.ok_or_else(changed)?;
        if !lease.holder.is_empty() || lease.fencing_token != s.fencing_token {
            return Err(changed());
        }
        let origin = CaptureOrigin {
            id: format!("capture:{}", s.attempt_id),
            run_id: r.id.clone(),
            node_id: n.node.id.clone(),
            plan_revision: r.plan_revision,
            plan_digest: r.plan_digest.clone(),
            attempt_id: s.attempt_id,
            session_id: s.id,
            workspace_id: s.workspace_id,
            root: record.refs.workspace.identity.clone().ok_or_else(changed)?,
            writer_epoch: u64::try_from(s.fencing_token).map_err(|_| changed())?,
            stop_receipt_sequence: u64::try_from(stop).map_err(|_| changed())?,
            capability_digest: sha(&p.snapshot)?,
            elapsed_at_stop_ms,
            prepared_at_ms: ms()?,
            runtime_reservation_ms: n.node.reservation.runtime_ms,
            consumed_inputs: input_bindings(
                &n.node,
                &record,
                &graph_candidates(&self.conn, &r)?,
                r.plan_revision,
            )?,
        };
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        tx.execute(
            "INSERT INTO goal_graph_jobs VALUES(?1,?2,?3,?4,'prepared',?5,?6,?7)",
            params![
                origin.id,
                r.id,
                origin.node_id,
                sql_num(origin.plan_revision)?,
                origin.attempt_id,
                origin.workspace_id,
                encode(&origin)?
            ],
        )
        .map_err(db_error)?;
        event(
            &tx,
            &r.id,
            r.revision,
            "goal.capture_prepared",
            json!({"job_id":origin.id,"origin_digest":sha(&origin)?,"stop_receipt_sequence":origin.stop_receipt_sequence,"task_accepted":false}),
        )?;
        tx.commit().map_err(db_error)?;
        self.graph_capture_clocks
            .insert(origin.id, std::time::Instant::now());
        Ok(())
    }
    fn verify_capture_origin(
        &self,
        origin: &CaptureOrigin,
    ) -> Result<(GoalRun, RuntimeNode, ScopeRecord)> {
        raw_collection_budget(&self.conn,"SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)+octet_length(run_id)+octet_length(node_id)+octet_length(state)+octet_length(attempt_id)+octet_length(workspace_id)),0),COALESCE(max(octet_length(document)),0) FROM goal_graph_jobs WHERE id=?1",[&origin.id],1,MAX_STORED_BYTES)?;
        let (run_id,node_id,rev,attempt,workspace,doc):(String,String,i64,String,String,String)=self.conn.query_row("SELECT run_id,node_id,plan_revision,attempt_id,workspace_id,document FROM goal_graph_jobs WHERE id=?1",[&origin.id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?))).map_err(db_error)?;
        if decode::<CaptureOrigin>(&doc)? != *origin
            || run_id != origin.run_id
            || node_id != origin.node_id
            || rev != sql_num(origin.plan_revision)?
            || attempt != origin.attempt_id
            || workspace != origin.workspace_id
        {
            return Err(changed());
        }
        let r = run(&self.conn, &origin.run_id)?;
        let p = checked_capability(&self.conn, &r)?;
        let n = node_records(&self.conn, &r.id)?
            .into_iter()
            .find(|n| n.node.id == origin.node_id)
            .ok_or_else(changed)?;
        let s = self.get_session(&origin.session_id)?;
        let record: ScopeRecord = raw(&self.conn, "goal_scopes", &r.scope_id)?;
        let lease = self
            .workspace_lease(&origin.workspace_id)?
            .ok_or_else(changed)?;
        if r.plan_revision != origin.plan_revision
            || r.plan_digest != origin.plan_digest
            || n.attempt_id.as_deref() != Some(&origin.attempt_id)
            || n.session_id.as_deref() != Some(&origin.session_id)
            || sha(&p.snapshot)? != origin.capability_digest
            || trusted_session_stop(&self.conn, &r.id, &s)?
                != sql_num(origin.stop_receipt_sequence)?
            || !lease.holder.is_empty()
            || u64::try_from(lease.fencing_token).ok() != Some(origin.writer_epoch)
            || record.refs.workspace.identity.as_ref() != Some(&origin.root)
            || !current_output_root(&self.conn, &record.refs.workspace)
        {
            return Err(changed());
        }
        let prepared:bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.capture_prepared' AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.payload.job_id')=?2 AND json_extract(document,'$.payload.origin_digest')=?3 AND sequence>?4)",params![r.id,origin.id,sha(origin)?,sql_num(origin.stop_receipt_sequence)?],|row|row.get(0)).map_err(db_error)?;
        if !prepared {
            return Err(denied());
        }
        Ok((r, n, record))
    }
    fn prepare_graph_capture_worker(
        &mut self,
        origin: &CaptureOrigin,
    ) -> Result<evidence::PreparedCandidateCapture> {
        let (_, n, record) = self.verify_capture_origin(origin)?;
        let inputs = origin
            .consumed_inputs
            .iter()
            .map(|i| {
                let mut a = if let Some(c) = &i.candidate {
                    ArtifactRequirement {
                        artifact_id: i.artifact_id.clone(),
                        relative_path: c.relative_path.clone(),
                        max_bytes: c.bytes,
                        expected_sha256: Some(i.sha256.clone()),
                    }
                } else {
                    record
                        .refs
                        .inputs
                        .iter()
                        .find(|a| a.artifact.artifact_id == i.artifact_id)
                        .ok_or_else(changed)?
                        .artifact
                        .clone()
                };
                a.expected_sha256 = Some(i.sha256.clone());
                Ok(a)
            })
            .collect::<Result<Vec<_>>>()?;
        let outputs = n
            .node
            .outputs
            .iter()
            .map(|o| {
                record
                    .selection
                    .outputs
                    .iter()
                    .find(|a| {
                        a.artifact.artifact_id == o.artifact_id
                            && o.contract_id == format!("output:{}", a.artifact.artifact_id)
                    })
                    .map(|a| (o.contract_id.clone(), a.artifact.clone()))
                    .ok_or_else(changed)
            })
            .collect::<Result<Vec<_>>>()?;
        let remaining = origin
            .runtime_reservation_ms
            .saturating_sub(origin.elapsed_at_stop_ms)
            .min(30_000);
        if remaining == 0 {
            return Err(err(
                "RESOURCE_LIMIT",
                "Producer exhausted its evidence runtime reservation",
            ));
        }
        Ok(evidence::PreparedCandidateCapture {
            root: origin.root.clone(),
            inputs,
            outputs,
            cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            deadline: std::time::Instant::now() + std::time::Duration::from_millis(remaining),
        })
    }
    fn mark_graph_capture_unknown(&mut self, origin: &CaptureOrigin) -> Result<()> {
        let (mut r, _, _) = self.verify_capture_origin(origin)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        tx.execute(
            "UPDATE goal_graph_jobs SET state='outcome_unknown' WHERE id=?1",
            [&origin.id],
        )
        .map_err(db_error)?;
        r.state = "outcome_unknown".into();
        save_run(
            &tx,
            &mut r,
            "goal.capture_unknown",
            json!({"job_id":origin.id,"io_completion":"outcome_unknown","worker_joined":false,"reservation_refunded":false,"task_accepted":false}),
        )?;
        tx.commit().map_err(db_error)
    }
    fn finish_graph_capture(
        &mut self,
        origin: &CaptureOrigin,
        result: Result<evidence::CapturedCandidate>,
        was_unknown: bool,
        queue_elapsed: std::time::Duration,
    ) -> Result<()> {
        let (r, n, record) = self.verify_capture_origin(origin)?;
        let observed = u64::try_from(queue_elapsed.as_millis()).map_err(|_| limited())?;
        let worker_ms = result
            .as_ref()
            .ok()
            .map(|c| u64::try_from(c.elapsed.as_millis()).unwrap_or(u64::MAX))
            .unwrap_or(0);
        let elapsed = observed
            .max(worker_ms)
            .checked_add(origin.elapsed_at_stop_ms)
            .ok_or_else(limited)?;
        let output_bytes = result.as_ref().ok().and_then(|c| {
            c.outputs
                .iter()
                .try_fold(0u64, |a, (_, f)| a.checked_add(f.bytes))
        });
        // A real joined result may settle its held reservation; an earlier
        // unjoined deadline remains unknown and cannot restart the graph.
        self.settle_goal_worker_observed(
            &origin.session_id,
            elapsed,
            true,
            false,
            Some(output_bytes),
        )?;
        let mut current = run(&self.conn, &r.id)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let success = result.is_ok()
            && !was_unknown
            && !current.cancel_requested
            && current.state != "blocked";
        let state = if success {
            "completed"
        } else if was_unknown {
            "outcome_unknown"
        } else {
            "failed"
        };
        if success {
            let c = result.map_err(|e| e)?;
            if c.root != origin.root || c.outputs.len() != n.node.outputs.len() {
                return Err(changed());
            }
            let id = Uuid::new_v4().to_string();
            let artifacts = c
                .outputs
                .into_iter()
                .map(|(contract_id, f)| GraphArtifactReference {
                    candidate_id: id.clone(),
                    artifact_id: f.artifact.artifact_id,
                    contract_id,
                    producer_node_id: origin.node_id.clone(),
                    producer_attempt_id: origin.attempt_id.clone(),
                    sha256: f.sha256,
                    bytes: f.bytes,
                    relative_path: f.artifact.relative_path,
                    root: c.root.clone(),
                    file_pin: f.pin,
                    writer_epoch: origin.writer_epoch,
                })
                .collect();
            let receipt = GraphCandidateReceipt {
                id: id.clone(),
                graph_contract_version: 1,
                run_id: r.id.clone(),
                scope_id: r.scope_id.clone(),
                plan_revision: origin.plan_revision,
                plan_digest: origin.plan_digest.clone(),
                node_id: origin.node_id.clone(),
                attempt_id: origin.attempt_id.clone(),
                session_id: origin.session_id.clone(),
                task_id: r.task_id.clone(),
                spec_revision: record.refs.spec_revision,
                input_revision: record.refs.workspace.base_revision.clone(),
                workspace_id: origin.workspace_id.clone(),
                capability_digest: origin.capability_digest.clone(),
                consumed_inputs: origin.consumed_inputs.clone(),
                artifacts,
                stop_receipt_sequence: origin.stop_receipt_sequence,
                capture_job_id: origin.id.clone(),
                capture_receipt_sequence: sequence(&tx)?.checked_add(1).ok_or_else(limited)?,
                created_at: now()?,
                source: "baton_goal_evidence".into(),
                integrity: "verified".into(),
                task_acceptance: "not_established".into(),
            };
            tx.execute(
                "INSERT INTO goal_graph_candidates VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    id,
                    r.id,
                    n.node.id,
                    sql_num(r.plan_revision)?,
                    origin.attempt_id,
                    encode(&receipt)?
                ],
            )
            .map_err(db_error)?;
            event(
                &tx,
                &r.id,
                current.revision,
                "goal.candidate_captured",
                json!({"candidate_id":id,"job_id":origin.id,"receipt_digest":sha(&receipt)?,"io_completion":"worker_joined","stop_receipt_sequence":origin.stop_receipt_sequence,"task_accepted":false}),
            )?;
        }
        tx.execute(
            "UPDATE goal_graph_jobs SET state=?1 WHERE id=?2",
            params![state, origin.id],
        )
        .map_err(db_error)?;
        if !success && current.state != "cancelled" {
            current.state = if was_unknown {
                "outcome_unknown"
            } else {
                "blocked"
            }
            .into();
        }
        save_run(
            &tx,
            &mut current,
            "goal.capture_settled",
            json!({"job_id":origin.id,"state":state,"io_completion":"worker_joined","actual_runtime_ms":elapsed,"actual_output_bytes":output_bytes,"task_accepted":false}),
        )?;
        tx.commit().map_err(db_error)
    }
}

fn graph_candidates(db: &Connection, r: &GoalRun) -> Result<Vec<GraphCandidateReceipt>> {
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)+octet_length(run_id)+octet_length(node_id)+octet_length(attempt_id)),0),COALESCE(max(octet_length(document)),0) FROM goal_graph_candidates WHERE run_id=?1",[&r.id],65536,MAX_STORED_BYTES)?;
    let mut q=db.prepare("SELECT id,node_id,plan_revision,attempt_id,document FROM goal_graph_candidates WHERE run_id=?1 ORDER BY rowid").map_err(db_error)?;
    let rows = q
        .query_map([&r.id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    let capability = checked_capability(db, r)?;
    let mut result = Vec::new();
    for (id, node, revision, attempt, doc) in rows {
        let c: GraphCandidateReceipt = decode(&doc)?;
        if c.id != id
            || c.run_id != r.id
            || c.node_id != node
            || c.plan_revision != u64::try_from(revision).map_err(|_| changed())?
            || c.attempt_id != attempt
            || c.scope_id != r.scope_id
            || c.task_id != r.task_id
            || c.capability_digest != sha(&capability.snapshot)?
            || c.source != "baton_goal_evidence"
            || c.integrity != "verified"
            || c.task_acceptance != "not_established"
            || c.artifacts.is_empty()
            || c.artifacts.len() > 64
            || c.graph_contract_version != 1
            || c.capture_receipt_sequence <= c.stop_receipt_sequence
        {
            return Err(changed());
        }
        let event_raw:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM events WHERE sequence=?1",[sql_num(c.capture_receipt_sequence)?],|row|row.get(0)).optional().map_err(db_error)?;
        let e: Event = decode(&event_raw.ok_or_else(denied)?.ok_or_else(limited)?)?;
        if e.source != "goal_core"
            || e.kind != "goal.candidate_captured"
            || e.aggregate_id != r.id
            || e.payload["candidate_id"] != c.id
            || e.payload["job_id"] != c.capture_job_id
            || e.payload["receipt_digest"] != sha(&c)?
            || e.payload["io_completion"] != "worker_joined"
        {
            return Err(denied());
        }
        let s: crate::sessions::ManagedSession = raw(db, "sessions", &c.session_id)?;
        if s.attempt_id != c.attempt_id
            || s.workspace_id != c.workspace_id
            || trusted_session_stop(db, &r.id, &s)? != sql_num(c.stop_receipt_sequence)?
        {
            return Err(changed());
        }
        for a in &c.artifacts {
            if a.candidate_id != c.id
                || a.producer_attempt_id != c.attempt_id
                || a.producer_node_id != c.node_id
                || a.writer_epoch != s.fencing_token
                || a.bytes != a.file_pin.bytes
                || a.sha256.len() != 64
            {
                return Err(changed());
            }
        }
        result.push(c);
    }
    Ok(result)
}
/// A private admission read, not an executor or a validation receipt. Capturing
/// bytes here cannot mark any required rule passed or consume execution budget.
pub(crate) struct GraphReadContext {
    run_id: String,
    revision: u64,
    capability_digest: String,
    root: agentbaton_workspaces::WorkspaceIdentity,
    epoch: u64,
    lease_holder: String,
    refs: Vec<GraphArtifactReference>,
}
pub(crate) struct GraphReadWitness {
    context: GraphReadContext,
    captured: evidence::CapturedCandidate,
    deadline: std::time::Instant,
}
impl GraphReadContext {
    pub(crate) fn execute(
        self,
        cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        deadline: std::time::Instant,
    ) -> Result<GraphReadWitness> {
        let inputs = self
            .refs
            .iter()
            .map(|a| ArtifactRequirement {
                artifact_id: a.candidate_id.clone() + ":" + &a.artifact_id,
                relative_path: a.relative_path.clone(),
                max_bytes: a.bytes,
                expected_sha256: Some(a.sha256.clone()),
            })
            .collect();
        let captured = evidence::PreparedCandidateCapture {
            root: self.root.clone(),
            inputs,
            outputs: vec![],
            cancelled: cancel,
            deadline,
        }
        .execute()?;
        if captured.inputs.len() != self.refs.len()
            || captured
                .inputs
                .iter()
                .zip(&self.refs)
                .any(|(f, a)| f.pin != a.file_pin || f.sha256 != a.sha256 || f.bytes != a.bytes)
        {
            return Err(changed());
        }
        Ok(GraphReadWitness {
            context: self,
            captured,
            deadline,
        })
    }
}
impl Store {
    pub(crate) fn prepare_graph_read_context(
        &self,
        run_id: &str,
        refs: Vec<GraphArtifactReference>,
    ) -> Result<Option<GraphReadContext>> {
        if refs.is_empty() {
            return Ok(None);
        }
        if refs.len() > 64 {
            return Err(limited());
        }
        let r = run(&self.conn, run_id)?;
        let p = checked_capability(&self.conn, &r)?;
        let candidates = graph_candidates(&self.conn, &r)?;
        let mut refs = refs;
        for c in candidates
            .iter()
            .filter(|c| c.plan_revision == r.plan_revision)
        {
            for a in &c.artifacts {
                if !refs.contains(a) {
                    refs.push(a.clone());
                }
            }
        }
        if refs.len() > 64 {
            return Err(limited());
        }
        for a in &refs {
            if !candidates.iter().any(|c| c.artifacts.contains(a)) {
                return Err(changed());
            }
        }
        let record: ScopeRecord = raw(&self.conn, "goal_scopes", &r.scope_id)?;
        let lease = self
            .workspace_lease(&record.refs.workspace.id)?
            .ok_or_else(changed)?;
        if !current_output_root(&self.conn, &record.refs.workspace) {
            return Err(changed());
        }
        if !lease.holder.is_empty() {
            let own:bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM goal_run_nodes n JOIN sessions s ON s.id=n.session_id WHERE n.run_id=?1 AND n.session_id=?2 AND n.state='reserved' AND json_extract(s.document,'$.turn_state') IS NULL)",params![r.id,lease.holder],|row|row.get(0)).map_err(db_error)?;
            if !own {
                return Err(changed());
            }
        }
        // Every later writer must be a stopped, authenticated attempt of this
        // graph. An unrelated identical-byte rewrite cannot borrow provenance.
        for a in &refs {
            if lease.fencing_token < a.writer_epoch {
                return Err(changed());
            }
            let foreign:bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM sessions s WHERE s.workspace_id=?1 AND json_extract(s.document,'$.fencing_token')>?2 AND NOT EXISTS(SELECT 1 FROM goal_run_nodes n WHERE n.run_id=?3 AND n.session_id=s.id AND ((n.state='candidate' AND json_extract(n.document,'$.stop_confirmed')=1) OR (n.state='reserved' AND n.session_id=?4))))",params![record.refs.workspace.id,sql_num(a.writer_epoch)?,r.id,lease.holder],|row|row.get(0)).map_err(db_error)?;
            if foreign {
                return Err(changed());
            }
        }
        Ok(Some(GraphReadContext {
            run_id: r.id,
            revision: r.revision,
            capability_digest: sha(&p.snapshot)?,
            root: record.refs.workspace.identity.ok_or_else(changed)?,
            epoch: lease.fencing_token,
            lease_holder: lease.holder,
            refs,
        }))
    }
}

impl Store {
    pub(crate) fn graph_request_read_context(
        &self,
        method: &str,
        params: &Value,
        _revision: Option<u64>,
        _key: Option<&str>,
    ) -> Result<Option<GraphReadContext>> {
        if method == "goal_node_approvals.propose" {
            let s: GraphNodeApprovalSelection =
                serde_json::from_value(params.clone()).map_err(|_| invalid())?;
            if let (Some(revision), Some(key)) = (_revision, _key) {
                if self.node_proposal_replay(&s, revision, key)?.is_some() {
                    return Ok(None);
                }
            }
        }
        let run_id = match method {
            "goal_graph_runs.dispatch" | "goal_node_approvals.propose" => params["run_id"].as_str(),
            "approvals.finalize" | "approvals.decline" => {
                let Some(id) = params["proposal_id"].as_str() else {
                    return Ok(None);
                };
                let p: ApprovalProposal = raw(&self.conn, "approval_proposals", id)?;
                if p.kind != "goal_node_acceptance" || p.consumed {
                    return Ok(None);
                }
                return self.prepare_graph_read_context(
                    p.snapshot["run_binding"]["run_id"]
                        .as_str()
                        .ok_or_else(changed)?,
                    serde_json::from_value(p.snapshot["candidate_refs"].clone())
                        .map_err(|_| changed())?,
                );
            }
            _ => None,
        };
        let Some(run_id) = run_id else {
            return Ok(None);
        };
        let r = run(&self.conn, run_id)?;
        checked_capability(&self.conn, &r)?;
        let candidates = graph_candidates(&self.conn, &r)?;
        let nodes = node_records(&self.conn, &r.id)?;
        let target = if method == "goal_node_approvals.propose" {
            nodes
                .iter()
                .find(|n| Some(n.node.id.as_str()) == params["node_id"].as_str())
        } else if let Some(n) = nodes.iter().find(|n| n.state == "reserved") {
            Some(n)
        } else {
            graph_ready_node(&self.conn, &r, &nodes, &candidates)?
        };
        let Some(n) = target else { return Ok(None) };
        let refs = graph_input_refs(&n.node, &candidates, r.plan_revision)?;
        self.prepare_graph_read_context(&r.id, refs)
    }
}
fn graph_input_refs(
    node: &PlanNode,
    candidates: &[GraphCandidateReceipt],
    revision: u64,
) -> Result<Vec<GraphArtifactReference>> {
    let mut refs = Vec::new();
    for input in &node.required_inputs {
        if let InputSource::Node { node_id, .. } = &input.source {
            let found = candidates
                .iter()
                .filter(|c| c.plan_revision == revision && &c.node_id == node_id)
                .flat_map(|c| &c.artifacts)
                .filter(|a| {
                    a.artifact_id == input.artifact_id && a.contract_id == input.contract_id
                })
                .collect::<Vec<_>>();
            if found.len() != 1 {
                return Err(changed());
            }
            if !refs.contains(found[0]) {
                refs.push(found[0].clone());
            }
        }
    }
    Ok(refs)
}
fn graph_owner_decisions(db: &Connection, r: &GoalRun) -> Result<Vec<GraphOwnerDecision>> {
    graph_records(
        db,
        r,
        "goal_graph_owner_decisions",
        "goal.owner_decided",
        "owner_decision_id",
        "local_owner_confirmation",
    )
}
fn graph_validations(db: &Connection, r: &GoalRun) -> Result<Vec<GraphValidationReceipt>> {
    graph_records(
        db,
        r,
        "goal_graph_validations",
        "goal.validation_settled",
        "validation_id",
        "goal_core",
    )
}
fn graph_records<T: serde::de::DeserializeOwned + Serialize>(
    db: &Connection,
    r: &GoalRun,
    table: &str,
    event_kind: &str,
    field: &str,
    source: &str,
) -> Result<Vec<T>> {
    if !matches!(
        table,
        "goal_graph_owner_decisions" | "goal_graph_validations"
    ) {
        return Err(invalid());
    }
    raw_collection_budget(db,&format!("SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)+octet_length(run_id)+octet_length(node_id)),0),COALESCE(max(octet_length(document)),0) FROM {table} WHERE run_id=?1"),[&r.id],65536,MAX_STORED_BYTES)?;
    let mut q = db
        .prepare(&format!(
            "SELECT id,node_id,plan_revision,document FROM {table} WHERE run_id=?1 ORDER BY rowid"
        ))
        .map_err(db_error)?;
    let rows = q
        .query_map([&r.id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    let mut records = Vec::new();
    for (id, node, revision, raw) in rows {
        let v: Value = decode(&raw)?;
        let record: T = serde_json::from_value(v.clone()).map_err(|_| changed())?;
        let receipt_digest = sha(&record)?;
        if v["id"] != id
            || v["node_id"] != node
            || v["run_id"] != r.id
            || v["plan_revision"] != revision
            || v["graph_contract_version"] != 1
        {
            return Err(changed());
        }
        let trusted:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')=?2 AND json_extract(document,'$.source')=?3 AND json_extract(document,?4)=?5 AND json_extract(document,'$.payload.receipt_digest')=?6)",params![r.id,event_kind,source,format!("$.payload.{field}"),id,receipt_digest],|row|row.get(0)).map_err(db_error)?;
        // Rejections use their distinct Owner source and never satisfy acceptance.
        let rejected = if table == "goal_graph_owner_decisions" && v["decision"] == "rejected" {
            db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')=?2 AND json_extract(document,'$.source')='local_owner_rejection' AND json_extract(document,'$.payload.owner_decision_id')=?3 AND json_extract(document,'$.payload.receipt_digest')=?4)",params![r.id,event_kind,id,receipt_digest],|row|row.get::<_,bool>(0)).map_err(db_error)?
        } else {
            false
        };
        if !trusted && !rejected {
            return Err(denied());
        }
        records.push(record);
    }
    Ok(records)
}
fn rules_satisfied(
    record: &ScopeRecord,
    node: &PlanNode,
    refs: &[GraphArtifactReference],
    validations: &[GraphValidationReceipt],
    owners: &[GraphOwnerDecision],
    without_owner: bool,
) -> bool {
    node.required_validators.iter().all(|rule| {
        if rule == INPUT_RULE {
            return true;
        } // Capture committed exact approved input SHA values.
        if rule == OWNER_RULE {
            return without_owner
                || refs.iter().all(|a| {
                    owners.iter().any(|o| {
                        o.rule_id == *rule
                            && o.decision == "accepted"
                            && o.candidate_refs.contains(a)
                    })
                });
        }
        let Some(def) = record.refs.validators.iter().find(|v| v.rule_id == *rule) else {
            return false;
        };
        refs.iter()
            .filter(|a| {
                record.selection.outputs.iter().any(|o| {
                    o.artifact.artifact_id == a.artifact_id && o.validators.contains(&def.reference)
                })
            })
            .all(|a| {
                validations.iter().any(|v| {
                    v.rule_id == *rule
                        && v.definition_digest == def.digest
                        && v.passed
                        && v.state == "passed"
                        && v.stop_confirmed
                        && v.io_completion == "worker_joined"
                        && v.task_acceptance == "not_established"
                        && v.candidate_refs.contains(a)
                })
            })
    })
}
fn graph_ready_node<'a>(
    db: &Connection,
    r: &GoalRun,
    nodes: &'a [RuntimeNode],
    candidates: &[GraphCandidateReceipt],
) -> Result<Option<&'a RuntimeNode>> {
    let record: ScopeRecord = raw(db, "goal_scopes", &r.scope_id)?;
    let validations = graph_validations(db, r)?;
    let owners = graph_owner_decisions(db, r)?;
    for n in nodes {
        if !(n.state == "not_started"
            || n.node.kind == NodeKind::OwnerApproval && n.state == "awaiting_owner")
            || n.node.parent_id.is_some()
        {
            continue;
        }
        let refs = match graph_input_refs(&n.node, candidates, r.plan_revision) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let mut ready = true;
        for input in &n.node.required_inputs {
            if let InputSource::Node {
                node_id,
                candidate_for_validation,
            } = &input.source
            {
                let Some(producer) = nodes
                    .iter()
                    .find(|p| &p.node.id == node_id && p.state == "candidate" && p.stop_confirmed)
                else {
                    ready = false;
                    break;
                };
                let producer_refs = candidates
                    .iter()
                    .filter(|c| c.plan_revision == r.plan_revision && c.node_id == *node_id)
                    .flat_map(|c| c.artifacts.clone())
                    .collect::<Vec<_>>();
                let exemption = matches!(n.node.kind, NodeKind::Validate | NodeKind::OwnerApproval)
                    && *candidate_for_validation;
                if producer_refs.is_empty()
                    || (!exemption
                        && !rules_satisfied(
                            &record,
                            &producer.node,
                            &producer_refs,
                            &validations,
                            &owners,
                            false,
                        ))
                    || (n.node.kind == NodeKind::OwnerApproval
                        && !rules_satisfied(
                            &record,
                            &producer.node,
                            &producer_refs,
                            &validations,
                            &owners,
                            true,
                        ))
                {
                    ready = false;
                    break;
                }
            }
        }
        for dependency in &n.node.dependencies {
            let Some(d) = nodes.iter().find(|d| &d.node.id == dependency) else {
                ready = false;
                break;
            };
            let satisfied = match d.node.kind {
                NodeKind::Validate => {
                    d.state == "validated"
                        && validations.iter().any(|v| {
                            v.node_id == d.node.id
                                && v.plan_revision == r.plan_revision
                                && v.passed
                                && v.stop_confirmed
                                && v.io_completion == "worker_joined"
                        })
                }
                NodeKind::OwnerApproval => {
                    d.state == "accepted"
                        && owners.iter().any(|o| {
                            o.node_id == d.node.id
                                && o.plan_revision == r.plan_revision
                                && o.decision == "accepted"
                        })
                }
                _ => {
                    let output_refs = candidates
                        .iter()
                        .filter(|c| c.plan_revision == r.plan_revision && c.node_id == d.node.id)
                        .flat_map(|c| c.artifacts.clone())
                        .collect::<Vec<_>>();
                    d.state == "candidate"
                        && d.stop_confirmed
                        && !output_refs.is_empty()
                        && (matches!(n.node.kind, NodeKind::Validate | NodeKind::OwnerApproval)
                            && refs.iter().any(|a| a.producer_node_id == d.node.id)
                            || rules_satisfied(
                                &record,
                                &d.node,
                                &output_refs,
                                &validations,
                                &owners,
                                false,
                            ))
                }
            };
            if !satisfied {
                ready = false;
                break;
            }
        }
        if ready {
            return Ok(Some(n));
        }
    }
    Ok(None)
}

impl Store {
    pub(crate) fn recover_graph_jobs(&mut self) -> Result<()> {
        raw_collection_budget(&self.conn,"SELECT count(*),COALESCE(sum(octet_length(id)+octet_length(run_id)+octet_length(state)+octet_length(document)),0),COALESCE(max(octet_length(document)),0) FROM goal_graph_jobs WHERE state IN('prepared','running','cancel_requested')",[],256,MAX_STORED_BYTES)?;
        let rows = {
            let mut q=self.conn.prepare("SELECT DISTINCT run_id FROM goal_graph_jobs WHERE state IN('prepared','running','cancel_requested')").map_err(db_error)?;
            let rows = q
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(db_error)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(db_error)?;
            rows
        };
        for id in rows {
            let tx = self
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(db_error)?;
            let mut r = run(&tx, &id)?;
            checked_capability(&tx, &r)?;
            tx.execute("UPDATE goal_graph_jobs SET state='outcome_unknown' WHERE run_id=?1 AND state IN('prepared','running','cancel_requested')",[&id]).map_err(db_error)?;
            for key_id in [
                format!("project:{}", r.project_id),
                format!("root-grant:{}", r.root_grant_id),
            ] {
                let raw:Option<Option<String>>=tx.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM goal_ledgers WHERE scope=?1",[&key_id],|row|row.get(0)).optional().map_err(db_error)?;
                if let Some(raw) = raw {
                    let mut m: MeterLedger = decode(&raw.ok_or_else(limited)?)?;
                    m.runtime_ms = None;
                    m.output_bytes = None;
                    m.tokens = None;
                    m.cost_microunits = None;
                    tx.execute(
                        "UPDATE goal_ledgers SET document=?1 WHERE scope=?2",
                        params![encode(&m)?, key_id],
                    )
                    .map_err(db_error)?;
                }
            }
            r.state = "outcome_unknown".into();
            save_run(
                &tx,
                &mut r,
                "goal.graph_jobs_interrupted",
                json!({"automatic_replay":false,"worker_joined":false,"reservation_refunded":false,"workspace_fence_retained":true,"task_accepted":false}),
            )?;
            tx.commit().map_err(db_error)?;
        }
        Ok(())
    }
}

pub(super) fn ready_connection_node<'a>(
    db: &Connection,
    r: &GoalRun,
    nodes: &'a [RuntimeNode],
) -> Result<Option<&'a RuntimeNode>> {
    Ok(
        graph_ready_node(db, r, nodes, &graph_candidates(db, r)?)?.filter(|n| {
            matches!(n.node.kind, NodeKind::Work | NodeKind::Summary)
                && matches!(n.node.executor, Executor::Connection { .. })
        }),
    )
}
fn input_bindings(
    node: &PlanNode,
    record: &ScopeRecord,
    candidates: &[GraphCandidateReceipt],
    revision: u64,
) -> Result<Vec<GraphInputBinding>> {
    let refs = graph_input_refs(node, candidates, revision)?;
    node.required_inputs
        .iter()
        .map(|i| match &i.source {
            InputSource::ApprovedExternal { sha256 } => {
                let source = record
                    .refs
                    .inputs
                    .iter()
                    .find(|s| s.artifact.artifact_id == i.artifact_id && &s.sha256 == sha256)
                    .ok_or_else(changed)?;
                Ok(GraphInputBinding {
                    artifact_id: source.artifact.artifact_id.clone(),
                    contract_id: i.contract_id.clone(),
                    sha256: sha256.clone(),
                    candidate: None,
                })
            }
            InputSource::Node { node_id, .. } => {
                let a = refs
                    .iter()
                    .find(|a| {
                        &a.producer_node_id == node_id
                            && a.artifact_id == i.artifact_id
                            && a.contract_id == i.contract_id
                    })
                    .ok_or_else(changed)?;
                Ok(GraphInputBinding {
                    artifact_id: i.artifact_id.clone(),
                    contract_id: i.contract_id.clone(),
                    sha256: a.sha256.clone(),
                    candidate: Some(a.clone()),
                })
            }
        })
        .collect()
}
pub(super) fn require_dispatch_read(
    db: &Connection,
    r: &GoalRun,
    node: &PlanNode,
    witness: Option<&GraphReadWitness>,
    session: &crate::sessions::ManagedSession,
) -> Result<()> {
    let refs = graph_input_refs(node, &graph_candidates(db, r)?, r.plan_revision)?;
    require_read(db, r, &refs, witness, Some(session))
}
fn require_read(
    db: &Connection,
    r: &GoalRun,
    refs: &[GraphArtifactReference],
    witness: Option<&GraphReadWitness>,
    new_session: Option<&crate::sessions::ManagedSession>,
) -> Result<()> {
    if refs.is_empty() {
        return Ok(());
    }
    let w = witness.ok_or_else(changed)?;
    let p = checked_capability(db, r)?;
    let record: ScopeRecord = raw(db, "goal_scopes", &r.scope_id)?;
    let lease: (String, i64) = db
        .query_row(
            "SELECT holder,fencing_token FROM workspace_leases WHERE resource=?1",
            [&record.refs.workspace.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(db_error)?;
    let lease_matches = if let Some(s) = new_session {
        w.context.lease_holder.is_empty()
            && lease.0 == s.id
            && u64::try_from(lease.1).ok() == Some(s.fencing_token)
            && w.context.epoch.checked_add(1) == Some(s.fencing_token)
    } else {
        lease.0 == w.context.lease_holder && u64::try_from(lease.1).ok() == Some(w.context.epoch)
    };
    if !lease_matches
        || std::time::Instant::now() >= w.deadline
        || w.context.run_id != r.id
        || w.context.revision != r.revision
        || w.context.capability_digest != sha(&p.snapshot)?
        || record.refs.workspace.identity.as_ref() != Some(&w.context.root)
        || w.captured.root != w.context.root
        || refs.iter().any(|a| !w.context.refs.contains(a))
        || !current_output_root(db, &record.refs.workspace)
    {
        return Err(changed());
    }
    Ok(())
}
pub(super) fn prompt_inputs(store: &Store, r: &GoalRun, node: &PlanNode) -> Result<Value> {
    let record: ScopeRecord = raw(&store.conn, "goal_scopes", &r.scope_id)?;
    let candidates = graph_candidates(&store.conn, r)?;
    let bindings = input_bindings(node, &record, &candidates, r.plan_revision)?;
    require_read(
        &store.conn,
        r,
        &graph_input_refs(node, &candidates, r.plan_revision)?,
        store.graph_input.as_ref(),
        None,
    )?;
    // The session already owns the current lease at prompt time. Its reservation
    // consumed the fresh input witness atomically before that lease was acquired.
    let mut inputs = Vec::new();
    for binding in bindings {
        let artifact = if let Some(c) = &binding.candidate {
            ArtifactRequirement {
                artifact_id: binding.artifact_id.clone(),
                relative_path: c.relative_path.clone(),
                max_bytes: c.bytes,
                expected_sha256: Some(c.sha256.clone()),
            }
        } else {
            record
                .refs
                .inputs
                .iter()
                .find(|a| a.artifact.artifact_id == binding.artifact_id)
                .ok_or_else(changed)?
                .artifact
                .clone()
        };
        let mut view = json!(binding);
        view["artifact"] = json!(artifact);
        inputs.push(view);
    }
    Ok(json!(inputs))
}
impl Store {
    pub(crate) fn is_goal_graph_run(&self, id: &str) -> Result<bool> {
        is_graph_run(&self.conn, id)
    }
}

fn node_acceptance_snapshot(
    db: &Connection,
    r: &GoalRun,
    s: &GraphNodeApprovalSelection,
    revision: u64,
    data: Option<&crate::data_workspaces::DataInputWitness>,
    witness: Option<&GraphReadWitness>,
) -> Result<Value> {
    if r.revision != revision
        || r.plan_revision != s.expected_plan_revision
        || r.cancel_requested
        || matches!(
            r.state.as_str(),
            "blocked" | "failed" | "outcome_unknown" | "cancelled"
        )
    {
        return Err(err(
            "STALE_REVISION",
            "Owner decision requires the exact current unblocked run and plan",
        ));
    }
    let (record, _) = checked_runtime_scope(db, r, data)?;
    let nodes = node_records(db, &r.id)?;
    let candidates = graph_candidates(db, r)?;
    let ready = graph_ready_node(db, r, &nodes, &candidates)?.ok_or_else(denied)?;
    if ready.node.id != s.node_id
        || ready.node.kind != NodeKind::OwnerApproval
        || !matches!(&ready.node.executor,Executor::Owner{rule_id} if rule_id==OWNER_RULE)
    {
        return Err(denied());
    }
    let refs = graph_input_refs(&ready.node, &candidates, r.plan_revision)?;
    if refs.is_empty() {
        return Err(invalid());
    }
    require_read(db, r, &refs, witness, None)?;
    let p = checked_capability(db, r)?;
    let validations = graph_validations(db, r)?;
    let candidate_receipts = candidates
        .into_iter()
        .filter(|c| refs.iter().any(|a| a.candidate_id == c.id))
        .collect::<Vec<_>>();
    let validation_refs = validations
        .into_iter()
        .filter(|v| v.candidate_refs.iter().any(|a| refs.contains(a)))
        .collect::<Vec<_>>();
    Ok(json!(GraphNodeAcceptanceSnapshot {
        graph_contract_version: 1,
        intent: s.clone(),
        run_binding: GraphRunBinding {
            run_id: r.id.clone(),
            scope_id: r.scope_id.clone(),
            task_id: r.task_id.clone(),
            project_id: r.project_id.clone(),
            root_grant_id: r.root_grant_id.clone(),
            plan_revision: r.plan_revision,
            plan_digest: r.plan_digest.clone(),
            scope_digest: record.scope_digest.clone(),
            spec_revision: record.refs.spec_revision,
            input_revision: record.refs.workspace.base_revision.clone(),
            workspace_id: record.refs.workspace.id.clone(),
            workspace: record.refs.workspace.identity.clone().ok_or_else(changed)?,
            capability_digest: sha(&p.snapshot)?
        },
        node: ready.node.clone(),
        criterion: GraphCriterionBinding {
            rule_id: OWNER_RULE.into(),
            criterion_text: record.selection.owner_acceptance_criteria.clone(),
            criterion_digest: sha(&(OWNER_RULE, &record.selection.owner_acceptance_criteria))?,
            artifact_ids: refs.iter().map(|a| a.artifact_id.clone()).collect()
        },
        candidate_refs: refs,
        candidate_receipts,
        validation_refs,
        input_bindings: input_bindings(
            &ready.node,
            &record,
            &graph_candidates(db, r)?,
            r.plan_revision
        )?,
        capability_digest: sha(&p.snapshot)?,
        expected_run_revision: revision,
        expected_ledger_revision: s.expected_ledger_revision
    }))
}
fn bounded_graph_proposal(p: &ApprovalProposal, event: &Event) -> Result<String> {
    let doc = encode(p)?;
    // 128 original UTF8 bytes of ASCII controls are the largest legal JSON
    // escaping overhead (six serialized bytes per original byte).
    let envelope = encode(
        &json!({"request_id":"\0".repeat(128),"result":p,"revision":u64::MAX,"warnings":[]}),
    )?;
    if doc.len() > MAX_GRAPH_REVIEW_BYTES
        || encode(event)?.len() > MAX_GRAPH_REVIEW_BYTES
        || envelope.len() > MAX_GRAPH_REVIEW_BYTES
    {
        return Err(limited());
    }
    Ok(doc)
}
impl Store {
    pub(crate) fn node_proposal_replay(
        &self,
        s: &GraphNodeApprovalSelection,
        revision: u64,
        key_id: &str,
    ) -> Result<Option<ApprovalProposal>> {
        key(key_id)?;
        let scope = format!("goal-graph:{}:owner-propose", s.run_id);
        let payload = encode(&json!({"intent":s,"expected_revision":revision}))?;
        let Some((old_payload, raw_result)) = idempotency_entry(&self.conn, &scope, key_id)? else {
            return Ok(None);
        };
        if old_payload != payload {
            return Err(err(
                "CONFIG_CONFLICT",
                "Owner proposal key belongs to a different exact selection/CAS",
            ));
        }
        let original: ApprovalProposal = decode(&raw_result)?;
        let current: ApprovalProposal = raw(&self.conn, "approval_proposals", &original.id)?;
        if current.id != original.id
            || current.kind != "goal_node_acceptance"
            || current.revision != original.revision
            || current.snapshot != original.snapshot
        {
            return Err(changed());
        }
        Ok(Some(current))
    }
    pub fn propose_goal_node_acceptance(
        &mut self,
        s: &GraphNodeApprovalSelection,
        revision: u64,
        key_id: &str,
    ) -> Result<ApprovalProposal> {
        if let Some(old) = self.node_proposal_replay(s, revision, key_id)? {
            return Ok(old);
        }
        let r = run(&self.conn, &s.run_id)?;
        if sequence(&self.conn)? != s.expected_ledger_revision {
            return Err(err(
                "STALE_REVISION",
                "Owner selection requires the current shared ledger",
            ));
        }
        let snapshot = node_acceptance_snapshot(
            &self.conn,
            &r,
            s,
            revision,
            self.data_input.as_ref(),
            self.graph_input.as_ref(),
        )?;
        let p = ApprovalProposal {
            id: Uuid::new_v4().to_string(),
            kind: "goal_node_acceptance".into(),
            revision: 1,
            snapshot,
            created_at: now()?,
            expires_at_ms: ms()?.checked_add(300000).ok_or_else(limited)?,
            consumed: false,
        };
        let e = Event {
            sequence: 0,
            event_id: Uuid::new_v4().to_string(),
            aggregate_id: p.id.clone(),
            revision: 1,
            kind: "approval.proposed".into(),
            occurred_at: now()?,
            payload: json!({"kind":p.kind,"snapshot":p.snapshot}),
            source: "user_action".into(),
        };
        let doc = bounded_graph_proposal(&p, &e)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if sequence(&tx)? != s.expected_ledger_revision || run(&tx, &r.id)?.revision != revision {
            return Err(err(
                "STALE_REVISION",
                "Owner snapshot changed before persistence",
            ));
        }
        tx.execute(
            "INSERT INTO approval_proposals(id,document) VALUES(?1,?2)",
            params![p.id, doc],
        )
        .map_err(db_error)?;
        remember(
            &tx,
            &format!("goal-graph:{}:owner-propose", r.id),
            key_id,
            &encode(&json!({"intent":s,"expected_revision":revision}))?,
            &p,
        )?;
        tx.execute("INSERT INTO events(document) VALUES(?1)", [encode(&e)?])
            .map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(p)
    }
    pub(crate) fn decide_goal_node(
        &mut self,
        id: &str,
        revision: u64,
        accept: bool,
    ) -> Result<Value> {
        key(id)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let mut p: ApprovalProposal = raw(&tx, "approval_proposals", id)?;
        if p.id != id || p.kind != "goal_node_acceptance" || p.revision != revision {
            return Err(changed());
        }
        let decision = if accept { "accepted" } else { "rejected" };
        let old:Option<Option<String>>=tx.query_row("SELECT CASE WHEN octet_length(result)<=1048576 THEN result END FROM approval_proposals WHERE id=?1 AND result IS NOT NULL",[id],|row|row.get(0)).optional().map_err(db_error)?;
        if let Some(old) = old {
            let old: GraphOwnerDecisionResult = decode(&old.ok_or_else(limited)?)?;
            if !p.consumed || old.approval_id != id || old.decision != decision {
                return Err(err(
                    "CONFIG_CONFLICT",
                    "This fixed proposal already records a different Owner decision",
                ));
            }
            return Ok(json!(old));
        }
        if p.consumed || p.expires_at_ms <= ms()? {
            return Err(denied());
        }
        let s: GraphNodeApprovalSelection =
            serde_json::from_value(p.snapshot["intent"].clone()).map_err(|_| changed())?;
        let rev = p.snapshot["expected_run_revision"]
            .as_u64()
            .ok_or_else(changed)?;
        let mut r = run(&tx, &s.run_id)?;
        if sequence(&tx)?
            != s.expected_ledger_revision
                .checked_add(1)
                .ok_or_else(limited)?
        {
            return Err(err(
                "STALE_REVISION",
                "Execution changed during private Owner review",
            ));
        }
        let proposed:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE sequence=?1 AND octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?2 AND json_extract(document,'$.kind')='approval.proposed' AND json_extract(document,'$.source')='user_action')",params![sql_num(sequence(&tx)?)?,p.id],|row|row.get(0)).map_err(db_error)?;
        if !proposed
            || node_acceptance_snapshot(
                &tx,
                &r,
                &s,
                rev,
                self.data_input.as_ref(),
                self.graph_input.as_ref(),
            )? != p.snapshot
        {
            return Err(changed());
        }
        let mut n = node_records(&tx, &r.id)?
            .into_iter()
            .find(|n| n.node.id == s.node_id)
            .ok_or_else(changed)?;
        let owner = GraphOwnerDecision {
            id: Uuid::new_v4().to_string(),
            graph_contract_version: 1,
            approval_id: p.id.clone(),
            run_id: r.id.clone(),
            plan_revision: r.plan_revision,
            plan_digest: r.plan_digest.clone(),
            node_id: n.node.id.clone(),
            rule_id: OWNER_RULE.into(),
            criterion_digest: p.snapshot["criterion"]["criterion_digest"]
                .as_str()
                .ok_or_else(changed)?
                .into(),
            candidate_refs: serde_json::from_value(p.snapshot["candidate_refs"].clone())
                .map_err(|_| changed())?,
            validation_ids: serde_json::from_value::<Vec<GraphValidationReceipt>>(
                p.snapshot["validation_refs"].clone(),
            )
            .map_err(|_| changed())?
            .into_iter()
            .map(|v| v.id)
            .collect(),
            decision: decision.into(),
            created_at: now()?,
            source: if accept {
                "local_owner_confirmation"
            } else {
                "local_owner_rejection"
            }
            .into(),
        };
        tx.execute(
            "INSERT INTO goal_graph_owner_decisions VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                owner.id,
                p.id,
                r.id,
                n.node.id,
                sql_num(r.plan_revision)?,
                encode(&owner)?
            ],
        )
        .map_err(db_error)?;
        let e = Event {
            sequence: 0,
            event_id: Uuid::new_v4().to_string(),
            aggregate_id: r.id.clone(),
            revision: r.revision,
            kind: "goal.owner_decided".into(),
            occurred_at: now()?,
            payload: json!({"owner_decision_id":owner.id,"approval_id":p.id,"receipt_digest":sha(&owner)?,"decision":decision,"task_accepted":false}),
            source: owner.source.clone(),
        };
        tx.execute("INSERT INTO events(document) VALUES(?1)", [encode(&e)?])
            .map_err(db_error)?;
        let task_accepted = if accept {
            maybe_accept_task(&tx, &mut r, &owner, self.graph_input.as_ref())?
        } else {
            false
        };
        n.state = decision.into();
        save_node(&tx, &r.id, &n)?;
        if !task_accepted {
            r.state = if accept { "queued" } else { "blocked" }.into();
        }
        save_run(
            &tx,
            &mut r,
            "goal.owner_node_settled",
            json!({"node_id":n.node.id,"owner_decision_id":owner.id,"decision":decision,"executor_started":false,"task_accepted":task_accepted}),
        )?;
        let receipt = GraphOwnerDecisionResult {
            graph_contract_version: 1,
            approval_id: p.id.clone(),
            run_id: r.id,
            node_id: n.node.id,
            plan_revision: r.plan_revision,
            decision: decision.into(),
            owner_decision_id: owner.id,
            run_revision: r.revision,
            task_accepted,
        };
        p.consumed = true;
        tx.execute(
            "UPDATE approval_proposals SET document=?1,result=?2 WHERE id=?3 AND result IS NULL",
            params![encode(&p)?, encode(&receipt)?, id],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(json!(receipt))
    }
}

pub(super) fn verify_intrinsic_node(db: &Connection, r: &GoalRun, n: &RuntimeNode) -> Result<()> {
    if !is_graph_run(db, &r.id)?
        || n.session_id.is_some()
        || n.attempt_id.is_some()
        || n.stop_confirmed
    {
        return Err(changed());
    }
    checked_capability(db, r)?;
    match (&n.node.kind, n.state.as_str()) {
        (NodeKind::OwnerApproval, "awaiting_owner") => {
            let valid:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.owner_ready' AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.payload.node_id')=?2 AND json_extract(document,'$.payload.plan_revision')=?3)",params![r.id,n.node.id,sql_num(n.plan_revision)?],|row|row.get(0)).map_err(db_error)?;
            if !valid {
                return Err(denied());
            }
        }
        (NodeKind::OwnerApproval, "accepted" | "rejected") => {
            let decisions = graph_owner_decisions(db, r)?;
            let found = decisions
                .iter()
                .filter(|o| {
                    o.node_id == n.node.id
                        && o.plan_revision == n.plan_revision
                        && o.decision == n.state
                })
                .collect::<Vec<_>>();
            if found.len() != 1 {
                return Err(changed());
            }
            let owner = found[0];
            let p: ApprovalProposal = raw(db, "approval_proposals", &owner.approval_id)?;
            let result:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(result)<=1048576 THEN result END FROM approval_proposals WHERE id=?1 AND result IS NOT NULL",[&owner.approval_id],|row|row.get(0)).optional().map_err(db_error)?;
            let result: GraphOwnerDecisionResult =
                decode(&result.ok_or_else(denied)?.ok_or_else(limited)?)?;
            if !p.consumed
                || p.kind != "goal_node_acceptance"
                || result.approval_id != p.id
                || result.run_id != r.id
                || result.node_id != n.node.id
                || result.owner_decision_id != owner.id
                || result.decision != n.state
                || p.snapshot["run_binding"]["run_id"] != r.id
                || p.snapshot["node"] != json!(n.node)
                || p.snapshot["candidate_refs"] != json!(owner.candidate_refs)
                || p.snapshot["criterion"]["criterion_digest"] != owner.criterion_digest
            {
                return Err(changed());
            }
        }
        _ => return Err(changed()),
    }
    Ok(())
}
impl Store {
    fn reconcile_graph_frontiers(&mut self) -> Result<()> {
        raw_collection_budget(&self.conn,"SELECT count(*),COALESCE(sum(octet_length(r.id)+octet_length(r.document)+octet_length(g.document)),0),COALESCE(max(octet_length(r.document)),0) FROM goal_runs r JOIN goal_graph_capabilities g ON g.run_id=r.id WHERE r.state IN('queued','running','awaiting_acceptance')",[],256,MAX_STORED_BYTES)?;
        let ids = {
            let mut q=self.conn.prepare("SELECT r.id FROM goal_runs r JOIN goal_graph_capabilities g ON g.run_id=r.id WHERE r.state IN('queued','running','awaiting_acceptance') ORDER BY r.rowid").map_err(db_error)?;
            let rows = q
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(db_error)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(db_error)?;
            rows
        };
        for id in ids {
            let mut r = run(&self.conn, &id)?;
            checked_capability(&self.conn, &r)?;
            if r.cancel_requested || !adopted_plan_is_current_head(&self.conn, &r)? {
                continue;
            }
            let nodes = node_records(&self.conn, &r.id)?;
            if nodes.iter().any(|n| {
                matches!(
                    n.state.as_str(),
                    "reserved" | "running" | "stopping" | "outcome_unknown"
                )
            }) {
                continue;
            }
            let candidates = graph_candidates(&self.conn, &r)?;
            if let Some(ready) = graph_ready_node(&self.conn, &r, &nodes, &candidates)? {
                if ready.node.kind == NodeKind::OwnerApproval && ready.state == "not_started" {
                    let mut n = ready.clone();
                    let tx = self
                        .conn
                        .transaction_with_behavior(TransactionBehavior::Immediate)
                        .map_err(db_error)?;
                    n.state = "awaiting_owner".into();
                    save_node(&tx, &r.id, &n)?;
                    r.state = "awaiting_acceptance".into();
                    save_run(
                        &tx,
                        &mut r,
                        "goal.owner_ready",
                        json!({"node_id":n.node.id,"plan_revision":n.plan_revision,"executor_started":false,"task_accepted":false}),
                    )?;
                    tx.commit().map_err(db_error)?;
                } else if matches!(ready.node.kind, NodeKind::Work | NodeKind::Summary)
                    && r.state == "awaiting_acceptance"
                {
                    let tx = self
                        .conn
                        .transaction_with_behavior(TransactionBehavior::Immediate)
                        .map_err(db_error)?;
                    r.state = "queued".into();
                    save_run(
                        &tx,
                        &mut r,
                        "goal.graph_frontier_ready",
                        json!({"node_id":ready.node.id,"new_dispatch":false,"task_accepted":false}),
                    )?;
                    tx.commit().map_err(db_error)?;
                }
            }
        }
        Ok(())
    }
}

fn maybe_accept_task(
    db: &Connection,
    r: &mut GoalRun,
    pending: &GraphOwnerDecision,
    witness: Option<&GraphReadWitness>,
) -> Result<bool> {
    if pending.decision != "accepted" {
        return Ok(false);
    }
    let record: ScopeRecord = raw(db, "goal_scopes", &r.scope_id)?;
    let nodes = node_records(db, &r.id)?;
    let candidates = graph_candidates(db, r)?;
    let validations = graph_validations(db, r)?;
    let mut owners = graph_owner_decisions(db, r)?;
    if !owners.iter().any(|o| o.id == pending.id) {
        owners.push(pending.clone());
    }
    let mut refs = Vec::new();
    for n in &nodes {
        match n.node.kind {
            NodeKind::OwnerApproval => {
                if n.node.id != pending.node_id && n.state != "accepted" {
                    return Ok(false);
                }
            }
            NodeKind::Validate => {
                if n.state != "validated"
                    || !validations.iter().any(|v| {
                        v.node_id == n.node.id
                            && v.plan_revision == r.plan_revision
                            && v.passed
                            && v.stop_confirmed
                            && v.io_completion == "worker_joined"
                    })
                {
                    return Ok(false);
                }
            }
            _ => {
                if n.state != "candidate" || !n.stop_confirmed {
                    return Ok(false);
                }
                let own = candidates
                    .iter()
                    .filter(|c| c.node_id == n.node.id && c.plan_revision == r.plan_revision)
                    .collect::<Vec<_>>();
                if own.len() != 1
                    || !rules_satisfied(
                        &record,
                        &n.node,
                        &own[0].artifacts,
                        &validations,
                        &owners,
                        false,
                    )
                {
                    return Ok(false);
                }
                refs.extend(own[0].artifacts.clone());
            }
        }
    }
    for output in &record.selection.outputs {
        if refs
            .iter()
            .filter(|a| {
                a.artifact_id == output.artifact.artifact_id
                    && a.contract_id == format!("output:{}", output.artifact.artifact_id)
            })
            .count()
            != 1
        {
            return Ok(false);
        }
    }
    require_read(db, r, &refs, witness, None)?;
    let mut task: Task = raw(db, "tasks", &r.task_id)?;
    if matches!(
        task.state,
        TaskState::Accepted
            | TaskState::CancelRequested
            | TaskState::Cancelled
            | TaskState::OutcomeUnknown
    ) || task.spec_revision != record.refs.spec_revision
        || task.goal != record.refs.task_goal
    {
        return Err(denied());
    }
    let active:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM goal_run_reservations WHERE run_id=?1 AND json_extract(document,'$.settled')=0) OR EXISTS(SELECT 1 FROM goal_graph_jobs WHERE run_id=?1 AND state IN('prepared','running','cancel_requested','outcome_unknown'))",[&r.id],|row|row.get(0)).map_err(db_error)?;
    if active {
        return Ok(false);
    }
    let id = Uuid::new_v4().to_string();
    let seq = sequence(db)?.checked_add(1).ok_or_else(limited)?;
    task.revision = task.revision.checked_add(1).ok_or_else(limited)?;
    task.state = TaskState::Accepted;
    task.acceptance_ref = Some(id.clone());
    task.acceptance_sequence = Some(seq);
    task.updated_at = now()?;
    let acceptance = GraphTaskAcceptance {
        id: id.clone(),
        graph_contract_version: 1,
        task_id: task.id.clone(),
        task_revision: task.revision,
        spec_revision: task.spec_revision,
        project_id: task.project_id.clone(),
        run_id: r.id.clone(),
        scope_id: r.scope_id.clone(),
        root_grant_id: r.root_grant_id.clone(),
        plan_revision: r.plan_revision,
        plan_digest: r.plan_digest.clone(),
        input_revision: record.refs.workspace.base_revision.clone(),
        workspace_id: record.refs.workspace.id.clone(),
        capability_digest: sha(&checked_capability(db, r)?.snapshot)?,
        criterion_digest: sha(&(OWNER_RULE, &record.selection.owner_acceptance_criteria))?,
        artifact_refs: refs,
        validation_ids: validations
            .iter()
            .filter(|v| v.plan_revision == r.plan_revision && v.passed)
            .map(|v| v.id.clone())
            .collect(),
        owner_decision_ids: owners
            .iter()
            .filter(|o| o.plan_revision == r.plan_revision && o.decision == "accepted")
            .map(|o| o.id.clone())
            .collect(),
        accepted_at: task.updated_at.clone(),
        sequence: seq,
        source: "baton_goal_acceptance".into(),
    };
    db.execute(
        "INSERT INTO goal_graph_acceptances VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            id,
            task.id,
            sql_num(task.revision)?,
            r.id,
            sql_num(r.plan_revision)?,
            encode(&acceptance)?
        ],
    )
    .map_err(db_error)?;
    let e = Event {
        sequence: 0,
        event_id: Uuid::new_v4().to_string(),
        aggregate_id: task.id.clone(),
        revision: task.revision,
        kind: "task.accepted".into(),
        occurred_at: now()?,
        payload: json!({"acceptance_ref":id,"acceptance_sequence":seq,"run_id":r.id,"receipt_digest":sha(&acceptance)?,"criteria_complete":true,"task_accepted":true}),
        source: "baton_goal_acceptance".into(),
    };
    db.execute("INSERT INTO events(document) VALUES(?1)", [encode(&e)?])
        .map_err(db_error)?;
    db.execute(
        "UPDATE tasks SET document=?1 WHERE id=?2",
        params![encode(&task)?, task.id],
    )
    .map_err(db_error)?;
    r.state = "accepted".into();
    r.task_accepted = true;
    Ok(true)
}
pub(super) fn verify_accepted_state(db: &Connection, r: &GoalRun) -> Result<GraphTaskAcceptance> {
    if !r.task_accepted || r.state != "accepted" || r.cancel_requested {
        return Err(changed());
    }
    let p = checked_capability(db, r)?;
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)+octet_length(task_id)+octet_length(run_id)),0),COALESCE(max(octet_length(document)),0) FROM goal_graph_acceptances WHERE run_id=?1",[&r.id],1,MAX_STORED_BYTES)?;
    let raw: Option<String> = db
        .query_row(
            "SELECT document FROM goal_graph_acceptances WHERE run_id=?1",
            [&r.id],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?;
    let a: GraphTaskAcceptance = decode(&raw.ok_or_else(denied)?)?;
    let task: Task = raw_task(db, &r.task_id)?;
    if a.graph_contract_version != 1
        || a.run_id != r.id
        || a.task_id != r.task_id
        || a.project_id != r.project_id
        || a.scope_id != r.scope_id
        || a.root_grant_id != r.root_grant_id
        || a.plan_revision != r.plan_revision
        || a.plan_digest != r.plan_digest
        || a.capability_digest != sha(&p.snapshot)?
        || a.source != "baton_goal_acceptance"
        || a.artifact_refs.is_empty()
        || task.state != TaskState::Accepted
        || task.revision != a.task_revision
        || task.spec_revision != a.spec_revision
        || task.acceptance_ref.as_deref() != Some(&a.id)
        || task.acceptance_sequence != Some(a.sequence)
    {
        return Err(changed());
    }
    let event:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM events WHERE sequence=?1",[sql_num(a.sequence)?],|row|row.get(0)).optional().map_err(db_error)?;
    let e: Event = decode(&event.ok_or_else(denied)?.ok_or_else(limited)?)?;
    if e.kind != "task.accepted"
        || e.source != "baton_goal_acceptance"
        || e.aggregate_id != task.id
        || e.revision != task.revision
        || e.payload["acceptance_ref"] != a.id
        || e.payload["acceptance_sequence"] != a.sequence
        || e.payload["run_id"] != r.id
        || e.payload["receipt_digest"] != sha(&a)?
        || e.payload["criteria_complete"] != true
    {
        return Err(denied());
    }
    Ok(a)
}
fn raw_task(db: &Connection, id: &str) -> Result<Task> {
    raw(db, "tasks", id)
}
pub(crate) fn verify_task_acceptance_state(db: &Connection, task: &Task) -> Result<()> {
    if task.state != TaskState::Accepted {
        if task.acceptance_ref.is_some() || task.acceptance_sequence.is_some() {
            return Err(changed());
        }
        return Ok(());
    }
    let id = task.acceptance_ref.as_deref().ok_or_else(denied)?;
    let run_id:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(run_id)<=128 THEN run_id END FROM goal_graph_acceptances WHERE id=?1",[id],|row|row.get(0)).optional().map_err(db_error)?;
    let r = run(db, &run_id.ok_or_else(denied)?.ok_or_else(limited)?)?;
    let a = verify_accepted_state(db, &r)?;
    if a.task_id != task.id || a.task_revision != task.revision {
        return Err(changed());
    }
    Ok(())
}
