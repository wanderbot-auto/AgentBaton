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
    db.query_row("SELECT EXISTS(SELECT 1 FROM goal_graph_capabilities WHERE run_id=?1)",[id],|row| row.get(0)).map_err(db_error)
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
    let (p, result) = native_confirmation(db,&r.id,"goal_graph_execution")?;
    if cap.version != GRAPH_CONTRACT_VERSION || cap.run_id != r.id || cap.approval_id != p.id || cap.snapshot_digest != sha(&p.snapshot)? || result["graph_contract_version"] != GRAPH_CONTRACT_VERSION || result["run"]["id"] != r.id || p.snapshot["execution"] != "goal_graph_v1_existing_workspace" || p.snapshot["graph_contract_version"] != GRAPH_CONTRACT_VERSION || p.snapshot["new_workspace_authority"] != false {return Err(changed());}
    let initial=&result["run"];
    if initial["scope_id"] != r.scope_id || initial["task_id"] != r.task_id || initial["project_id"] != r.project_id || initial["root_grant_id"] != r.root_grant_id || initial["plan_digest"] != p.snapshot["plan"]["digest"] || initial["plan_revision"] != p.snapshot["plan"]["proposal"]["revision"] { return Err(changed()); }
    let confirmation:Option<i64>=db.query_row("SELECT sequence FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='approval.confirmed' AND json_extract(document,'$.source')='local_owner_confirmation' AND json_extract(document,'$.payload.kind')='goal_graph_execution' AND json_extract(document,'$.payload.run_id')=?1 AND json_extract(document,'$.payload.execution_snapshot_digest')=?2 ORDER BY sequence LIMIT 1",params![r.id,cap.snapshot_digest],|row|row.get(0)).optional().map_err(db_error)?;
    let authorized:Option<i64>=db.query_row("SELECT sequence FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.run_authorized' AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.payload.scope_id')=?2 AND json_extract(document,'$.payload.plan_digest')=?3 AND json_extract(document,'$.payload.plan_revision')=?4 AND json_extract(document,'$.payload.execution_snapshot_digest')=?5 AND json_extract(document,'$.payload.reservation_made')=0 AND json_extract(document,'$.payload.dispatched')=0 ORDER BY sequence LIMIT 1",params![r.id,r.scope_id,initial["plan_digest"].as_str().ok_or_else(changed)?,initial["plan_revision"].as_i64().ok_or_else(changed)?,cap.snapshot_digest],|row|row.get(0)).optional().map_err(db_error)?;
    if !matches!((confirmation,authorized),(Some(a),Some(b)) if a>0&&b>a) {return Err(denied());}
    verify_plan_advances(db,r,&p,initial,authorized.ok_or_else(denied)?)?;
    Ok(p)
}

pub(super) fn persist_capability(db: &Connection, p: &ApprovalProposal) -> Result<()> {
    let cap = GraphCapability { version: GRAPH_CONTRACT_VERSION,run_id:p.id.clone(),approval_id:p.id.clone(),snapshot_digest:sha(&p.snapshot)? };
    db.execute("INSERT INTO goal_graph_capabilities VALUES(?1,?1,1,?2)",params![p.id,encode(&cap)?]).map_err(db_error)?;
    Ok(())
}
pub(super) fn check_finalization_snapshot(db: &Connection, p: &ApprovalProposal, data: Option<&crate::data_workspaces::DataInputWitness>) -> Result<()> {
    let original:GraphRunSelection=serde_json::from_value(p.snapshot["intent"].clone()).map_err(|_|changed())?;
    let current=GraphRunSelection { expected_ledger_revision:sequence(db)?,..original.clone() };
    let mut expected=graph_snapshot(db,&current,data)?;
    expected["intent"]=json!(original);
    if expected != p.snapshot { return Err(changed()); }
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
        Ok(GraphRunSummary { graph_contract_version: GRAPH_CONTRACT_VERSION, task_accepted: false, run:r, ledger_revision:sequence(&self.conn)? })
    }
    pub fn goal_graph_details(&self, id: &str) -> Result<GraphRunDetails> {
        let summary = self.goal_graph_summary(id)?;
        let nodes=self.goal_node_history(&summary.run)?.into_iter().map(|n| GraphNodeView { execution_kind:match n.node.executor { Executor::Connection{..}=>"agent",Executor::Validator{..}=>"validator",Executor::Owner{..}=>"owner" }.into(),node:n.node,plan_revision:n.plan_revision,state:n.state,session_id:n.session_id,attempt_id:n.attempt_id,job_id:None,acceptance_state:"pending".into(),stop_confirmed:n.stop_confirmed,actual_runtime_ms:n.actual_runtime_ms,actual_output_bytes:n.actual_output_bytes,tokens:n.tokens,cost_microunits:n.cost_microunits,candidate_ids:vec![],validation_ids:vec![],owner_decision_ids:vec![],blocked_reason:None }).collect();
        let details=GraphRunDetails {graph_contract_version:GRAPH_CONTRACT_VERSION,run:summary.run,nodes,candidates:vec![],validations:vec![],owner_decisions:vec![],ledger_revision:summary.ledger_revision,task_accepted:false,task_acceptance:None};
        if encode(&details)?.len()>4_194_304 { return Err(limited()); }
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
}
struct CaptureWorker {
    origin: CaptureOrigin,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: std::thread::JoinHandle<Result<evidence::CapturedCandidate>>,
    deadline: std::time::Instant,
    unknown_recorded: bool,
}
#[derive(Default)]
pub(crate) struct GraphWorkers {
    captures: std::collections::HashMap<String, CaptureWorker>,
    stopping: bool,
}
impl GraphWorkers {
    pub(crate) fn has_workers(&self) -> bool { !self.captures.is_empty() }
    pub(crate) fn tick(&mut self, store: &mut Store) -> Result<()> {
        let finished:Vec<_>=self.captures.iter().filter(|(_,w)|w.thread.is_finished()).map(|(id,_)|id.clone()).collect();
        for id in finished {
            let w=self.captures.remove(&id).ok_or_else(changed)?;
            let result=w.thread.join().map_err(|_|err("OUTCOME_UNKNOWN","Owned evidence worker panicked after an unknown partial observation")).and_then(|r|r);
            store.finish_graph_capture(&w.origin,result,w.unknown_recorded)?;
        }
        for w in self.captures.values_mut() {
            let r=run(&store.conn,&w.origin.run_id)?;
            if r.cancel_requested || crate::policy::checked_lineage(&store.conn,&r.root_grant_id).is_err() {w.cancel.store(true,std::sync::atomic::Ordering::Relaxed);}
            if std::time::Instant::now()>w.deadline && !w.unknown_recorded {
                w.cancel.store(true,std::sync::atomic::Ordering::Relaxed);
                store.mark_graph_capture_unknown(&w.origin)?;
                w.unknown_recorded=true;
            }
        }
        if self.has_workers() || self.stopping { return Ok(()); }
        raw_collection_budget(&store.conn,"SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)+octet_length(state)+octet_length(workspace_id)),0),COALESCE(max(octet_length(document)),0) FROM goal_graph_jobs WHERE state='prepared'",[],1,MAX_STORED_BYTES)?;
        let doc:Option<String>=store.conn.query_row("SELECT document FROM goal_graph_jobs WHERE state='prepared' LIMIT 1",[],|row|row.get(0)).optional().map_err(db_error)?;
        let Some(doc)=doc else{return Ok(())};
        let origin:CaptureOrigin=decode(&doc)?;
        let prepared=match store.prepare_graph_capture_worker(&origin) {Ok(p)=>p,Err(error)=>{store.finish_graph_capture(&origin,Err(error),false)?;return Ok(());}};
        let cancel=prepared.cancelled.clone();
        let deadline=prepared.deadline;
        let thread=std::thread::Builder::new().name("baton-goal-evidence".into()).spawn(move||prepared.execute());
        match thread {
            Ok(thread)=>{self.captures.insert(origin.id.clone(),CaptureWorker { origin,cancel,thread,deadline,unknown_recorded:false });}
            Err(_)=>store.finish_graph_capture(&origin,Err(err("RESOURCE_LIMIT","Owned graph evidence worker could not start")),false)?,
        }
        Ok(())
    }
    pub(crate) fn shutdown(&mut self, store: &mut Store) -> Result<()> {
        self.stopping=true;
        for w in self.captures.values(){w.cancel.store(true,std::sync::atomic::Ordering::Relaxed);}
        // Only drain owned handles; never admit a queued job during shutdown.
        // If the OS read does not join, retain its handle until daemon/process
        // teardown and leave durable unknown/fence/no-replay, never a join receipt.
        let until=std::time::Instant::now()+std::time::Duration::from_secs(2);
        while self.has_workers() && std::time::Instant::now()<until { self.tick(store)?;std::thread::sleep(std::time::Duration::from_millis(5)); }
        for w in self.captures.values_mut(){if !w.unknown_recorded {store.mark_graph_capture_unknown(&w.origin)?;w.unknown_recorded=true;}}
        Ok(())
    }
}
impl Store {
    pub(super) fn queue_graph_capture(&mut self, session_id:&str, elapsed_at_stop_ms:u64)->Result<()> {
        let s=self.get_session(session_id)?;
        let (run_id,doc):(String,String)=self.conn.query_row("SELECT run_id,document FROM goal_run_nodes WHERE session_id=?1 AND octet_length(document)<=262144",[session_id],|row|Ok((row.get(0)?,row.get(1)?))).map_err(db_error)?;
        let r=run(&self.conn,&run_id)?;
        let p=checked_capability(&self.conn,&r)?;
        let n:RuntimeNode=decode(&doc)?;
        let stop=trusted_session_stop(&self.conn,&r.id,&s)?;
        let record:ScopeRecord=raw(&self.conn,"goal_scopes",&r.scope_id)?;
        if !current_output_root(&self.conn,&record.refs.workspace) {return Err(changed());}
        let lease=self.workspace_lease(&s.workspace_id)?.ok_or_else(changed)?;
        if !lease.holder.is_empty() || lease.fencing_token != s.fencing_token {return Err(changed());}
        let origin=CaptureOrigin {id:format!("capture:{}",s.attempt_id),run_id:r.id.clone(),node_id:n.node.id,plan_revision:r.plan_revision,plan_digest:r.plan_digest,attempt_id:s.attempt_id,session_id:s.id,workspace_id:s.workspace_id,root:record.refs.workspace.identity.ok_or_else(changed)?,writer_epoch:u64::try_from(s.fencing_token).map_err(|_|changed())?,stop_receipt_sequence:u64::try_from(stop).map_err(|_|changed())?,capability_digest:sha(&p.snapshot)?,elapsed_at_stop_ms,prepared_at_ms:ms()?,runtime_reservation_ms:n.node.reservation.runtime_ms};
        let tx=self.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;
        tx.execute("INSERT INTO goal_graph_jobs VALUES(?1,?2,?3,?4,'prepared',?5,?6,?7)",params![origin.id,r.id,origin.node_id,sql_num(origin.plan_revision)?,origin.attempt_id,origin.workspace_id,encode(&origin)?]).map_err(db_error)?;
        event(&tx,&r.id,r.revision,"goal.capture_prepared",json!({"job_id":origin.id,"origin_digest":sha(&origin)?,"stop_receipt_sequence":origin.stop_receipt_sequence,"task_accepted":false}))?;
        tx.commit().map_err(db_error)
    }
    fn verify_capture_origin(&self, origin:&CaptureOrigin)->Result<(GoalRun,RuntimeNode,ScopeRecord)> {
        raw_collection_budget(&self.conn,"SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)+octet_length(run_id)+octet_length(node_id)+octet_length(state)+octet_length(attempt_id)+octet_length(workspace_id)),0),COALESCE(max(octet_length(document)),0) FROM goal_graph_jobs WHERE id=?1",[&origin.id],1,MAX_STORED_BYTES)?;
        let (run_id,node_id,rev,attempt,workspace,doc):(String,String,i64,String,String,String)=self.conn.query_row("SELECT run_id,node_id,plan_revision,attempt_id,workspace_id,document FROM goal_graph_jobs WHERE id=?1",[&origin.id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?))).map_err(db_error)?;
        if decode::<CaptureOrigin>(&doc)? != *origin || run_id!=origin.run_id || node_id!=origin.node_id || rev!=sql_num(origin.plan_revision)? || attempt!=origin.attempt_id || workspace!=origin.workspace_id {return Err(changed());}
        let r=run(&self.conn,&origin.run_id)?;
        let p=checked_capability(&self.conn,&r)?;
        let n=node_records(&self.conn,&r.id)?.into_iter().find(|n|n.node.id==origin.node_id).ok_or_else(changed)?;
        let s=self.get_session(&origin.session_id)?;
        let record:ScopeRecord=raw(&self.conn,"goal_scopes",&r.scope_id)?;
        let lease=self.workspace_lease(&origin.workspace_id)?.ok_or_else(changed)?;
        if r.plan_revision!=origin.plan_revision || r.plan_digest!=origin.plan_digest || n.attempt_id.as_deref()!=Some(&origin.attempt_id) || n.session_id.as_deref()!=Some(&origin.session_id) || sha(&p.snapshot)?!=origin.capability_digest || trusted_session_stop(&self.conn,&r.id,&s)?!=sql_num(origin.stop_receipt_sequence)? || !lease.holder.is_empty() || u64::try_from(lease.fencing_token).ok()!=Some(origin.writer_epoch) || record.refs.workspace.identity.as_ref()!=Some(&origin.root) || !current_output_root(&self.conn,&record.refs.workspace) {return Err(changed());}
        let prepared:bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.capture_prepared' AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.payload.job_id')=?2 AND json_extract(document,'$.payload.origin_digest')=?3 AND sequence>?4)",params![r.id,origin.id,sha(origin)?,sql_num(origin.stop_receipt_sequence)?],|row|row.get(0)).map_err(db_error)?;
        if !prepared {return Err(denied());}
        Ok((r,n,record))
    }
    fn prepare_graph_capture_worker(&mut self,origin:&CaptureOrigin)->Result<evidence::PreparedCandidateCapture> {
        let (_,n,record)=self.verify_capture_origin(origin)?;
        let inputs=record.refs.inputs.iter().map(|i|{let mut a=i.artifact.clone();a.expected_sha256=Some(i.sha256.clone());a}).collect();
        let outputs=n.node.outputs.iter().map(|o|record.selection.outputs.iter().find(|a|a.artifact.artifact_id==o.artifact_id && o.contract_id==format!("output:{}",a.artifact.artifact_id)).map(|a|(o.contract_id.clone(),a.artifact.clone())).ok_or_else(changed)).collect::<Result<Vec<_>>>()?;
        let remaining=origin.runtime_reservation_ms.saturating_sub(origin.elapsed_at_stop_ms).min(30_000);
        if remaining==0 {return Err(err("RESOURCE_LIMIT","Producer exhausted its evidence runtime reservation"));}
        Ok(evidence::PreparedCandidateCapture {root:origin.root.clone(),inputs,outputs,cancelled:std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),deadline:std::time::Instant::now()+std::time::Duration::from_millis(remaining)})
    }
    fn mark_graph_capture_unknown(&mut self,origin:&CaptureOrigin)->Result<()> {
        let (mut r,_,_)=self.verify_capture_origin(origin)?;
        let tx=self.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;
        tx.execute("UPDATE goal_graph_jobs SET state='outcome_unknown' WHERE id=?1",[&origin.id]).map_err(db_error)?;
        r.state="outcome_unknown".into();
        save_run(&tx,&mut r,"goal.capture_unknown",json!({"job_id":origin.id,"io_completion":"outcome_unknown","worker_joined":false,"reservation_refunded":false,"task_accepted":false}))?;
        tx.commit().map_err(db_error)
    }
    fn finish_graph_capture(&mut self,origin:&CaptureOrigin,result:Result<evidence::CapturedCandidate>,was_unknown:bool)->Result<()> {
        let (r,n,record)=self.verify_capture_origin(origin)?;
        let elapsed=u64::try_from(ms()?.saturating_sub(origin.prepared_at_ms)).map_err(|_|limited())?.checked_add(origin.elapsed_at_stop_ms).ok_or_else(limited)?;
        let output_bytes=result.as_ref().ok().and_then(|c|c.outputs.iter().try_fold(0u64,|a,(_,f)|a.checked_add(f.bytes)));
        // A real joined result may settle its held reservation; an earlier
        // unjoined deadline remains unknown and cannot restart the graph.
        self.settle_goal_worker_observed(&origin.session_id,elapsed,true,false,Some(output_bytes))?;
        let mut current=run(&self.conn,&r.id)?;
        let tx=self.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;
        let success=result.is_ok() && !was_unknown && !current.cancel_requested && current.state!="blocked";
        let state=if success {"completed"} else if was_unknown {"outcome_unknown"} else {"failed"};
        if success {
            let c=result.map_err(|e|e)?;
            if c.root!=origin.root || c.outputs.len()!=n.node.outputs.len() {return Err(changed());}
            let id=Uuid::new_v4().to_string();
            let artifacts=c.outputs.into_iter().map(|(contract_id,f)|GraphArtifactReference {candidate_id:id.clone(),artifact_id:f.artifact.artifact_id,contract_id,producer_node_id:origin.node_id.clone(),producer_attempt_id:origin.attempt_id.clone(),sha256:f.sha256,bytes:f.bytes,relative_path:f.artifact.relative_path,root:c.root.clone(),file_pin:f.pin,writer_epoch:origin.writer_epoch}).collect();
            let receipt=GraphCandidateReceipt {id:id.clone(),graph_contract_version:1,run_id:r.id.clone(),scope_id:r.scope_id.clone(),plan_revision:origin.plan_revision,plan_digest:origin.plan_digest.clone(),node_id:origin.node_id.clone(),attempt_id:origin.attempt_id.clone(),session_id:origin.session_id.clone(),task_id:r.task_id.clone(),spec_revision:record.refs.spec_revision,input_revision:record.refs.workspace.base_revision.clone(),workspace_id:origin.workspace_id.clone(),capability_digest:origin.capability_digest.clone(),consumed_inputs:c.inputs.into_iter().map(|f|GraphInputBinding {artifact_id:f.artifact.artifact_id,contract_id:"external-input".into(),sha256:f.sha256,candidate:None}).collect(),artifacts,stop_receipt_sequence:origin.stop_receipt_sequence,capture_job_id:origin.id.clone(),capture_receipt_sequence:sequence(&tx)?.checked_add(1).ok_or_else(limited)?,created_at:now()?,source:"baton_goal_evidence".into(),integrity:"verified".into(),task_acceptance:"not_established".into()};
            tx.execute("INSERT INTO goal_graph_candidates VALUES(?1,?2,?3,?4,?5,?6)",params![id,r.id,n.node.id,sql_num(r.plan_revision)?,origin.attempt_id,encode(&receipt)?]).map_err(db_error)?;
            event(&tx,&r.id,current.revision,"goal.candidate_captured",json!({"candidate_id":id,"job_id":origin.id,"receipt_digest":sha(&receipt)?,"io_completion":"worker_joined","stop_receipt_sequence":origin.stop_receipt_sequence,"task_accepted":false}))?;
        }
        tx.execute("UPDATE goal_graph_jobs SET state=?1 WHERE id=?2",params![state,origin.id]).map_err(db_error)?;
        if !success && current.state!="cancelled" {current.state=if was_unknown {"outcome_unknown"}else{"blocked"}.into();}
        save_run(&tx,&mut current,"goal.capture_settled",json!({"job_id":origin.id,"state":state,"io_completion":"worker_joined","actual_runtime_ms":elapsed,"actual_output_bytes":output_bytes,"task_accepted":false}))?;
        tx.commit().map_err(db_error)
    }
}

fn graph_candidates(db:&Connection,r:&GoalRun)->Result<Vec<GraphCandidateReceipt>> {
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)+octet_length(run_id)+octet_length(node_id)+octet_length(attempt_id)),0),COALESCE(max(octet_length(document)),0) FROM goal_graph_candidates WHERE run_id=?1",[&r.id],65536,MAX_STORED_BYTES)?;
    let mut q=db.prepare("SELECT id,node_id,plan_revision,attempt_id,document FROM goal_graph_candidates WHERE run_id=?1 ORDER BY rowid").map_err(db_error)?;
    let rows=q.query_map([&r.id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,i64>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?))).map_err(db_error)?.collect::<std::result::Result<Vec<_>,_>>().map_err(db_error)?;
    let capability=checked_capability(db,r)?;
    let mut result=Vec::new();
    for (id,node,revision,attempt,doc) in rows {
        let c:GraphCandidateReceipt=decode(&doc)?;
        if c.id!=id || c.run_id!=r.id || c.node_id!=node || c.plan_revision!=u64::try_from(revision).map_err(|_|changed())? || c.attempt_id!=attempt || c.scope_id!=r.scope_id || c.task_id!=r.task_id || c.capability_digest!=sha(&capability.snapshot)? || c.source!="baton_goal_evidence" || c.integrity!="verified" || c.task_acceptance!="not_established" || c.artifacts.is_empty() || c.artifacts.len()>64 || c.graph_contract_version!=1 || c.capture_receipt_sequence<=c.stop_receipt_sequence {return Err(changed());}
        let event_raw:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM events WHERE sequence=?1",[sql_num(c.capture_receipt_sequence)?],|row|row.get(0)).optional().map_err(db_error)?;
        let e:Event=decode(&event_raw.ok_or_else(denied)?.ok_or_else(limited)?)?;
        if e.source!="goal_core" || e.kind!="goal.candidate_captured" || e.aggregate_id!=r.id || e.payload["candidate_id"]!=c.id || e.payload["job_id"]!=c.capture_job_id || e.payload["receipt_digest"]!=sha(&c)? || e.payload["io_completion"]!="worker_joined" {return Err(denied());}
        let s:crate::sessions::ManagedSession=raw(db,"sessions",&c.session_id)?;
        if s.attempt_id!=c.attempt_id || s.workspace_id!=c.workspace_id || trusted_session_stop(db,&r.id,&s)?!=sql_num(c.stop_receipt_sequence)? {return Err(changed());}
        for a in &c.artifacts {
            if a.candidate_id!=c.id || a.producer_attempt_id!=c.attempt_id || a.producer_node_id!=c.node_id || a.writer_epoch!=s.fencing_token || a.bytes!=a.file_pin.bytes || a.sha256.len()!=64 {return Err(changed());}
        }
        result.push(c);
    }
    Ok(result)
}
/// A private admission read, not an executor or a validation receipt. Capturing
/// bytes here cannot mark any required rule passed or consume execution budget.
pub(crate) struct GraphReadContext {
    run_id:String,
    revision:u64,
    capability_digest:String,
    root:agentbaton_workspaces::WorkspaceIdentity,
    epoch:u64,
    refs:Vec<GraphArtifactReference>,
}
pub(crate) struct GraphReadWitness {
    context:GraphReadContext,
    captured:evidence::CapturedCandidate,
    deadline:std::time::Instant,
}
impl GraphReadContext {
    pub(crate) fn execute(self,cancel:std::sync::Arc<std::sync::atomic::AtomicBool>,deadline:std::time::Instant)->Result<GraphReadWitness> {
        let inputs=self.refs.iter().map(|a|ArtifactRequirement {artifact_id:a.candidate_id.clone()+":"+&a.artifact_id,relative_path:a.relative_path.clone(),max_bytes:a.bytes,expected_sha256:Some(a.sha256.clone())}).collect();
        let captured=evidence::PreparedCandidateCapture {root:self.root.clone(),inputs,outputs:vec![],cancelled:cancel,deadline}.execute()?;
        if captured.inputs.len()!=self.refs.len() || captured.inputs.iter().zip(&self.refs).any(|(f,a)|f.pin!=a.file_pin || f.sha256!=a.sha256 || f.bytes!=a.bytes) {return Err(changed());}
        Ok(GraphReadWitness {context:self,captured,deadline})
    }
}
impl Store {
    pub(crate) fn prepare_graph_read_context(&self,run_id:&str,refs:Vec<GraphArtifactReference>)->Result<Option<GraphReadContext>> {
        if refs.is_empty(){return Ok(None);}
        if refs.len()>64 {return Err(limited());}
        let r=run(&self.conn,run_id)?;
        let p=checked_capability(&self.conn,&r)?;
        let candidates=graph_candidates(&self.conn,&r)?;
        for a in &refs {if !candidates.iter().any(|c|c.artifacts.contains(a)) {return Err(changed());}}
        let record:ScopeRecord=raw(&self.conn,"goal_scopes",&r.scope_id)?;
        let lease=self.workspace_lease(&record.refs.workspace.id)?.ok_or_else(changed)?;
        if !lease.holder.is_empty() || !current_output_root(&self.conn,&record.refs.workspace){return Err(changed());}
        // Every later writer must be a stopped, authenticated attempt of this
        // graph. An unrelated identical-byte rewrite cannot borrow provenance.
        for a in &refs {
            if lease.fencing_token<a.writer_epoch{return Err(changed());}
            let foreign:bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM sessions s WHERE s.workspace_id=?1 AND json_extract(s.document,'$.fencing_token')>?2 AND NOT EXISTS(SELECT 1 FROM goal_run_nodes n WHERE n.run_id=?3 AND n.session_id=s.id AND n.state='candidate' AND json_extract(n.document,'$.stop_confirmed')=1))",params![record.refs.workspace.id,sql_num(a.writer_epoch)?,r.id],|row|row.get(0)).map_err(db_error)?;
            if foreign{return Err(changed());}
        }
        Ok(Some(GraphReadContext {run_id:r.id,revision:r.revision,capability_digest:sha(&p.snapshot)?,root:record.refs.workspace.identity.ok_or_else(changed)?,epoch:lease.fencing_token,refs}))
    }
    fn require_graph_read(&self,db:&Connection,r:&GoalRun,refs:&[GraphArtifactReference])->Result<()> {
        if refs.is_empty(){return Ok(());}
        let w=self.graph_input.as_ref().ok_or_else(changed)?;
        let p=checked_capability(db,r)?;
        let record:ScopeRecord=raw(db,"goal_scopes",&r.scope_id)?;
        let lease:(String,i64)=db.query_row("SELECT holder,fencing_token FROM workspace_leases WHERE resource=?1",[&record.refs.workspace.id],|row|Ok((row.get(0)?,row.get(1)?))).map_err(db_error)?;
        if std::time::Instant::now()>=w.deadline || w.context.run_id!=r.id || w.context.revision!=r.revision || w.context.capability_digest!=sha(&p.snapshot)? || !lease.0.is_empty() || u64::try_from(lease.1).ok()!=Some(w.context.epoch) || record.refs.workspace.identity.as_ref()!=Some(&w.context.root) || w.captured.root!=w.context.root || refs.iter().any(|a|!w.context.refs.contains(a)) || !current_output_root(db,&record.refs.workspace) {return Err(changed());}
        Ok(())
    }
}

impl Store {
    pub(crate) fn graph_request_read_context(&self,method:&str,params:&Value,_revision:Option<u64>,_key:Option<&str>)->Result<Option<GraphReadContext>> {
        let run_id=match method {
            "goal_graph_runs.dispatch"|"goal_node_approvals.propose"=>params["run_id"].as_str(),
            "approvals.finalize"|"approvals.decline"=>{
                let Some(id)=params["proposal_id"].as_str() else{return Ok(None)};
                let p:ApprovalProposal=raw(&self.conn,"approval_proposals",id)?;
                if p.kind!="goal_node_acceptance" || p.consumed{return Ok(None);}
                return self.prepare_graph_read_context(p.snapshot["run_binding"]["run_id"].as_str().ok_or_else(changed)?,serde_json::from_value(p.snapshot["candidate_refs"].clone()).map_err(|_|changed())?);
            }
            _=>None,
        };
        let Some(run_id)=run_id else{return Ok(None)};
        let r=run(&self.conn,run_id)?;
        checked_capability(&self.conn,&r)?;
        let candidates=graph_candidates(&self.conn,&r)?;
        let nodes=node_records(&self.conn,&r.id)?;
        let target=if method=="goal_node_approvals.propose" {nodes.iter().find(|n|Some(n.node.id.as_str())==params["node_id"].as_str())}else{graph_ready_node(&self.conn,&r,&nodes,&candidates)?};
        let Some(n)=target else{return Ok(None)};
        let refs=graph_input_refs(&n.node,&candidates,r.plan_revision)?;
        self.prepare_graph_read_context(&r.id,refs)
    }
}
fn graph_input_refs(node:&PlanNode,candidates:&[GraphCandidateReceipt],revision:u64)->Result<Vec<GraphArtifactReference>> {
    let mut refs=Vec::new();
    for input in &node.required_inputs {
        if let InputSource::Node{node_id,..}=&input.source {
            let found=candidates.iter().filter(|c|c.plan_revision==revision && &c.node_id==node_id).flat_map(|c|&c.artifacts).filter(|a|a.artifact_id==input.artifact_id&&a.contract_id==input.contract_id).collect::<Vec<_>>();
            if found.len()!=1{return Err(changed());}
            if !refs.contains(found[0]){refs.push(found[0].clone());}
        }
    }
    Ok(refs)
}
fn graph_owner_decisions(db:&Connection,r:&GoalRun)->Result<Vec<GraphOwnerDecision>> {
    graph_records(db,r,"goal_graph_owner_decisions","goal.owner_decided","owner_decision_id","local_owner_confirmation")
}
fn graph_validations(db:&Connection,r:&GoalRun)->Result<Vec<GraphValidationReceipt>> {
    graph_records(db,r,"goal_graph_validations","goal.validation_settled","validation_id","goal_core")
}
fn graph_records<T:serde::de::DeserializeOwned+Serialize>(db:&Connection,r:&GoalRun,table:&str,event_kind:&str,field:&str,source:&str)->Result<Vec<T>> {
    if !matches!(table,"goal_graph_owner_decisions"|"goal_graph_validations"){return Err(invalid());}
    raw_collection_budget(db,&format!("SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)+octet_length(run_id)+octet_length(node_id)),0),COALESCE(max(octet_length(document)),0) FROM {table} WHERE run_id=?1"),[&r.id],65536,MAX_STORED_BYTES)?;
    let mut q=db.prepare(&format!("SELECT id,node_id,plan_revision,document FROM {table} WHERE run_id=?1 ORDER BY rowid")).map_err(db_error)?;
    let rows=q.query_map([&r.id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,i64>(2)?,row.get::<_,String>(3)?))).map_err(db_error)?.collect::<std::result::Result<Vec<_>,_>>().map_err(db_error)?;
    let mut records=Vec::new();
    for (id,node,revision,raw) in rows {
        let v:Value=decode(&raw)?;
        if v["id"]!=id||v["node_id"]!=node||v["run_id"]!=r.id||v["plan_revision"]!=revision||v["graph_contract_version"]!=1{return Err(changed());}
        let trusted:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')=?2 AND json_extract(document,'$.source')=?3 AND json_extract(document,?4)=?5 AND json_extract(document,'$.payload.receipt_digest')=?6)",params![r.id,event_kind,source,format!("$.payload.{field}"),id,sha(&v)?],|row|row.get(0)).map_err(db_error)?;
        // Rejections use their distinct Owner source and never satisfy acceptance.
        let rejected=if table=="goal_graph_owner_decisions"&&v["decision"]=="rejected" {db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')=?2 AND json_extract(document,'$.source')='local_owner_rejection' AND json_extract(document,'$.payload.owner_decision_id')=?3 AND json_extract(document,'$.payload.receipt_digest')=?4)",params![r.id,event_kind,id,sha(&v)?],|row|row.get::<_,bool>(0)).map_err(db_error)?}else{false};
        if !trusted&&!rejected{return Err(denied());}
        records.push(serde_json::from_value(v).map_err(|_|changed())?);
    }
    Ok(records)
}
fn rules_satisfied(record:&ScopeRecord,node:&PlanNode,refs:&[GraphArtifactReference],validations:&[GraphValidationReceipt],owners:&[GraphOwnerDecision],without_owner:bool)->bool {
    node.required_validators.iter().all(|rule| {
        if rule==INPUT_RULE {return true;} // Capture committed exact approved input SHA values.
        if rule==OWNER_RULE {return without_owner || refs.iter().all(|a|owners.iter().any(|o|o.rule_id==*rule&&o.decision=="accepted"&&o.candidate_refs.contains(a)));}
        let Some(def)=record.refs.validators.iter().find(|v|v.rule_id==*rule)else{return false};
        refs.iter().filter(|a|record.selection.outputs.iter().any(|o|o.artifact.artifact_id==a.artifact_id&&o.validators.contains(&def.reference))).all(|a|validations.iter().any(|v|v.rule_id==*rule&&v.definition_digest==def.digest&&v.passed&&v.state=="passed"&&v.stop_confirmed&&v.io_completion=="worker_joined"&&v.task_acceptance=="not_established"&&v.candidate_refs.contains(a)))
    })
}
fn graph_ready_node<'a>(db:&Connection,r:&GoalRun,nodes:&'a[RuntimeNode],candidates:&[GraphCandidateReceipt])->Result<Option<&'a RuntimeNode>> {
    let record:ScopeRecord=raw(db,"goal_scopes",&r.scope_id)?;
    let validations=graph_validations(db,r)?;
    let owners=graph_owner_decisions(db,r)?;
    for n in nodes {
        if n.state!="not_started" || n.node.parent_id.is_some(){continue;}
        let refs=match graph_input_refs(&n.node,candidates,r.plan_revision){Ok(r)=>r,Err(_)=>continue};
        let mut ready=true;
        for input in &n.node.required_inputs {
            if let InputSource::Node{node_id,candidate_for_validation}=&input.source {
                let Some(producer)=nodes.iter().find(|p|&p.node.id==node_id&&p.state=="candidate"&&p.stop_confirmed)else{ready=false;break};
                let producer_refs=candidates.iter().filter(|c|c.plan_revision==r.plan_revision&&c.node_id==*node_id).flat_map(|c|c.artifacts.clone()).collect::<Vec<_>>();
                let exemption=matches!(n.node.kind,NodeKind::Validate|NodeKind::OwnerApproval)&&*candidate_for_validation;
                if producer_refs.is_empty() || (!exemption&&!rules_satisfied(&record,&producer.node,&producer_refs,&validations,&owners,false)) || (n.node.kind==NodeKind::OwnerApproval&&!rules_satisfied(&record,&producer.node,&producer_refs,&validations,&owners,true)){ready=false;break;}
            }
        }
        for dependency in &n.node.dependencies {
            let Some(d)=nodes.iter().find(|d|&d.node.id==dependency)else{ready=false;break};
            let satisfied=match d.node.kind {
                NodeKind::Validate=>d.state=="validated"&&validations.iter().any(|v|v.node_id==d.node.id&&v.plan_revision==r.plan_revision&&v.passed&&v.stop_confirmed&&v.io_completion=="worker_joined"),
                NodeKind::OwnerApproval=>d.state=="accepted"&&owners.iter().any(|o|o.node_id==d.node.id&&o.plan_revision==r.plan_revision&&o.decision=="accepted"),
                _=>{let output_refs=candidates.iter().filter(|c|c.plan_revision==r.plan_revision&&c.node_id==d.node.id).flat_map(|c|c.artifacts.clone()).collect::<Vec<_>>();d.state=="candidate"&&d.stop_confirmed&&!output_refs.is_empty()&&(matches!(n.node.kind,NodeKind::Validate|NodeKind::OwnerApproval)&&refs.iter().any(|a|a.producer_node_id==d.node.id)||rules_satisfied(&record,&d.node,&output_refs,&validations,&owners,false))}
            };
            if !satisfied{ready=false;break;}
        }
        if ready{return Ok(Some(n));}
    }
    Ok(None)
}

impl Store {
    pub(crate) fn recover_graph_jobs(&mut self)->Result<()> {
        raw_collection_budget(&self.conn,"SELECT count(*),COALESCE(sum(octet_length(id)+octet_length(run_id)+octet_length(state)+octet_length(document)),0),COALESCE(max(octet_length(document)),0) FROM goal_graph_jobs WHERE state IN('prepared','running','cancel_requested')",[],256,MAX_STORED_BYTES)?;
        let rows={let mut q=self.conn.prepare("SELECT DISTINCT run_id FROM goal_graph_jobs WHERE state IN('prepared','running','cancel_requested')").map_err(db_error)?;let rows=q.query_map([],|row|row.get::<_,String>(0)).map_err(db_error)?.collect::<std::result::Result<Vec<_>,_>>().map_err(db_error)?;rows};
        for id in rows {
            let tx=self.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;
            let mut r=run(&tx,&id)?;
            checked_capability(&tx,&r)?;
            tx.execute("UPDATE goal_graph_jobs SET state='outcome_unknown' WHERE run_id=?1 AND state IN('prepared','running','cancel_requested')",[&id]).map_err(db_error)?;
            r.state="outcome_unknown".into();
            save_run(&tx,&mut r,"goal.graph_jobs_interrupted",json!({"automatic_replay":false,"worker_joined":false,"reservation_refunded":false,"workspace_fence_retained":true,"task_accepted":false}))?;
            tx.commit().map_err(db_error)?;
        }
        Ok(())
    }
}
