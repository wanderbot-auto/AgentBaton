//! Native-approved durable execution. Nested under goal_store so authoritative
//! scope/plan constructors stay private and cannot be populated from IPC labels.
use super::*;

#[path = "goal_factory.rs"]
pub(super) mod factory;

#[path = "goal_graph.rs"]
mod graph;
pub(crate) use graph::{
    migrate as migrate_graph, verify_task_acceptance_state, GraphReadContext, GraphReadWitness,
    GraphWorkers,
};
pub use graph::{
    GraphArtifactReference, GraphCandidateReceipt, GraphCriterionBinding,
    GraphDefinitionReusePermission, GraphExecutionSnapshot, GraphInputBinding,
    GraphNodeAcceptanceSnapshot, GraphNodeApprovalSelection, GraphNodeView, GraphOwnerDecision,
    GraphOwnerDecisionResult, GraphRunBinding, GraphRunDetails, GraphRunSelection, GraphRunSummary,
    GraphTaskAcceptance, GraphValidationReceipt, GraphValidationView,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalRunSelection {
    pub scope_id: String,
    pub expected_plan_revision: u64,
    pub expected_ledger_revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalRunDispatch {
    pub run_id: String,
    pub expected_ledger_revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalRun {
    pub id: String,
    pub scope_id: String,
    pub project_id: String,
    pub task_id: String,
    pub root_grant_id: String,
    pub plan_revision: u64,
    pub plan_digest: String,
    pub revision: u64,
    pub state: String,
    pub cancel_requested: bool,
    pub task_accepted: bool,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeNode {
    node: PlanNode,
    plan_revision: u64,
    state: String,
    session_id: Option<String>,
    attempt_id: Option<String>,
    started_at_ms: Option<i64>,
    stop_confirmed: bool,
    actual_runtime_ms: Option<u64>,
    actual_output_bytes: Option<u64>,
    tokens: Option<u64>,
    cost_microunits: Option<u64>,
}
/// Constructed only after current Store authority/input checks, never from wire.
pub(crate) struct GoalDispatchContext {
    pub run: GoalRun,
    node: PlanNode,
    ledger_revision: u64,
}
fn sequence(db: &Connection) -> Result<u64> {
    let value: i64 = db
        .query_row("SELECT COALESCE(max(sequence),0) FROM events", [], |r| {
            r.get(0)
        })
        .map_err(db_error)?;
    u64::try_from(value).map_err(|_| changed())
}
fn run(db: &Connection, id: &str) -> Result<GoalRun> {
    key(id)?;
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)+octet_length(scope_id)+octet_length(project_id)+octet_length(task_id)+octet_length(state)),0),COALESCE(max(octet_length(document)),0) FROM goal_runs WHERE id=?1",[id],1,MAX_STORED_BYTES)?;
    let raw: Option<Option<String>> = db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM goal_runs WHERE id=?1",[id],|r|r.get(0)).optional().map_err(db_error)?;
    let stored = raw
        .ok_or_else(|| err("NOT_FOUND", "Goal run does not exist"))?
        .ok_or_else(limited)?;
    let r: GoalRun = decode(&stored)?;
    let cols: (String, i64, String) = db
        .query_row(
            "SELECT scope_id,revision,state FROM goal_runs WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(db_error)?;
    if r.id != id
        || cols != (r.scope_id.clone(), sql_num(r.revision)?, r.state.clone())
        || !matches!(
            r.state.as_str(),
            "queued"
                | "running"
                | "cancel_requested"
                | "cancelled"
                | "awaiting_acceptance"
                | "blocked"
                | "failed"
                | "outcome_unknown"
                | "accepted"
        )
    {
        return Err(changed());
    }
    if r.task_accepted || r.state == "accepted" {
        graph::verify_accepted_state(db, &r)?;
    }
    Ok(r)
}
fn node_records(db: &Connection, id: &str) -> Result<Vec<RuntimeNode>> {
    let r = run(db, id)?;
    let plans = bounded_plan_history(db, &r.scope_id)?;
    let plan = plans
        .iter()
        .find(|p| p.proposal.revision == r.plan_revision && p.digest == r.plan_digest)
        .ok_or_else(changed)?;
    node_records_at(db, &r, r.plan_revision, &r.plan_digest, plan)
}
fn node_records_at(
    db: &Connection,
    r: &GoalRun,
    revision: u64,
    digest: &str,
    plan: &GoalPlanView,
) -> Result<Vec<RuntimeNode>> {
    let id = r.id.as_str();
    if plan.proposal.revision != revision || plan.digest != digest {
        return Err(changed());
    }
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(node_id)+octet_length(state)+COALESCE(octet_length(session_id),0)+COALESCE(octet_length(attempt_id),0)),0),COALESCE(max(octet_length(document)),0) FROM goal_run_nodes WHERE run_id=?1 AND plan_revision=?2",params![id,sql_num(revision)?],256,MAX_GOAL_PLAN_BYTES as i64)?;
    let mut q=db.prepare("SELECT node_id,ordinal,state,session_id,attempt_id,CASE WHEN octet_length(document)<=262144 THEN document END FROM goal_run_nodes WHERE run_id=?1 AND plan_revision=?2 ORDER BY ordinal LIMIT 257").map_err(db_error)?;
    type Row = (
        String,
        i64,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    );
    let rows = q
        .query_map(params![id, sql_num(revision)?], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<Row>, _>>()
        .map_err(db_error)?;
    if rows.len() != plan.proposal.nodes.len() || rows.len() > 256 {
        return Err(changed());
    }
    let mut result = Vec::new();
    for (node_id, ordinal, state, session_id, attempt_id, doc) in rows {
        let n: RuntimeNode = decode(&doc.ok_or_else(limited)?)?;
        let ordinal = usize::try_from(ordinal).map_err(|_| changed())?;
        if n.node.id != node_id
            || n.plan_revision != revision
            || n.state != state
            || !matches!(
                n.state.as_str(),
                "not_started"
                    | "reserved"
                    | "running"
                    | "stopping"
                    | "candidate"
                    | "cancelled"
                    | "failed"
                    | "outcome_unknown"
                    | "awaiting_owner"
                    | "accepted"
                    | "rejected"
                    | "validated"
            )
            || n.session_id != session_id
            || n.attempt_id != attempt_id
            || plan.proposal.nodes.get(ordinal) != Some(&n.node)
        {
            return Err(changed());
        }
        if let Some(session) = session_id {
            let stored: String = db
                .query_row(
                    "SELECT document FROM sessions WHERE id=?1 AND octet_length(document)<=1048576",
                    [&session],
                    |row| row.get(0),
                )
                .map_err(db_error)?;
            let sess: crate::sessions::ManagedSession = decode(&stored)?;
            if sess.id != session
                || Some(&sess.attempt_id) != attempt_id.as_ref()
                || sess.task_id != r.task_id
                || sess.grant_id != r.root_grant_id
                || sess.workspace_id != n.node.workspace_id
            {
                return Err(changed());
            }
            let raw:String=db.query_row("SELECT document FROM goal_run_reservations WHERE attempt_id=?1 AND run_id=?2 AND node_id=?3 AND octet_length(document)<=262144",params![sess.attempt_id,id,node_id],|row|row.get(0)).map_err(db_error)?;
            let receipt: ReservationRecord = decode(&raw)?;
            if receipt.run_id != id
                || receipt.node_id != node_id
                || receipt.plan_revision != revision
                || receipt.reservation != n.node.reservation
            {
                return Err(changed());
            }
            let committed:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.kind')='goal.node_reserved' AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.payload.node_id')=?2 AND json_extract(document,'$.payload.attempt_id')=?3 AND json_extract(document,'$.payload.session_id')=?4 AND json_extract(document,'$.payload.plan_revision')=?5 AND json_extract(document,'$.payload.plan_digest')=?6)",params![id,node_id,sess.attempt_id,session,sql_num(revision)?,digest],|row|row.get(0)).map_err(db_error)?;
            if !committed {
                return Err(denied());
            }
            if n.state == "candidate" && sess.turn_state.as_deref() != Some("completed") {
                return Err(changed());
            }
            if n.stop_confirmed {
                let supervisor_sequence = trusted_session_stop(db, id, &sess)?;
                let committed:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.kind')='goal.node_settled' AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.payload.node_id')=?2 AND json_extract(document,'$.payload.attempt_id')=?3 AND json_extract(document,'$.payload.session_id')=?4 AND json_extract(document,'$.payload.stop_confirmed')=1 AND json_extract(document,'$.payload.state')=?5 AND sequence>?6 AND json_extract(document,'$.payload.actual_runtime_ms') IS ?7 AND json_extract(document,'$.payload.actual_output_bytes') IS ?8 AND json_extract(document,'$.payload.tokens') IS ?9 AND json_extract(document,'$.payload.cost_microunits') IS ?10)",params![id,node_id,sess.attempt_id,session,n.state,supervisor_sequence,n.actual_runtime_ms.map(sql_num).transpose()?,n.actual_output_bytes.map(sql_num).transpose()?,n.tokens.map(sql_num).transpose()?,n.cost_microunits.map(sql_num).transpose()?],|row|row.get(0)).map_err(db_error)?;
                if !committed {
                    return Err(denied());
                }
            }
        } else if attempt_id.is_some() || n.state != "not_started" {
            graph::verify_intrinsic_node(db, r, &n)?;
        } else {
            let prior:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM goal_run_reservations WHERE run_id=?1 AND node_id=?2 AND json_extract(document,'$.plan_revision')=?3)",params![id,node_id,sql_num(revision)?],|row|row.get(0)).map_err(db_error)?;
            if prior {
                return Err(changed());
            }
        }
        if !n.stop_confirmed
            && (n.actual_runtime_ms.is_some()
                || n.actual_output_bytes.is_some()
                || n.tokens.is_some()
                || n.cost_microunits.is_some())
        {
            return Err(changed());
        }
        if matches!(n.state.as_str(), "candidate" | "cancelled" | "failed") && !n.stop_confirmed {
            return Err(changed());
        }
        result.push(n);
    }
    Ok(result)
}

fn save_run(db: &Connection, r: &mut GoalRun, kind: &str, payload: Value) -> Result<()> {
    let before = r.revision;
    r.revision = r.revision.checked_add(1).ok_or_else(limited)?;
    r.updated_at = now()?;
    if db
        .execute(
            "UPDATE goal_runs SET revision=?1,state=?2,document=?3 WHERE id=?4 AND revision=?5",
            params![
                sql_num(r.revision)?,
                r.state,
                encode(r)?,
                r.id,
                sql_num(before)?
            ],
        )
        .map_err(db_error)?
        != 1
    {
        return Err(err(
            "STALE_REVISION",
            "Goal run changed at atomic compare-and-swap",
        ));
    }
    event(db, &r.id, r.revision, kind, payload)
}
fn save_node(db: &Connection, id: &str, n: &RuntimeNode) -> Result<()> {
    if db.execute("UPDATE goal_run_nodes SET state=?1,session_id=?2,attempt_id=?3,document=?4 WHERE run_id=?5 AND node_id=?6 AND plan_revision=?7",params![n.state,n.session_id,n.attempt_id,encode(n)?,id,n.node.id,sql_num(n.plan_revision)?]).map_err(db_error)?!=1{return Err(changed());}
    Ok(())
}
pub(crate) fn migrate(db: &mut Connection, version: u32) -> Result<()> {
    if version < 11 {
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        tx.execute_batch("CREATE TABLE goal_runs(id TEXT PRIMARY KEY REFERENCES approval_proposals(id),scope_id TEXT NOT NULL REFERENCES goal_scopes(id),project_id TEXT NOT NULL REFERENCES projects(id),task_id TEXT NOT NULL REFERENCES tasks(id),revision INTEGER NOT NULL CHECK(revision>0),state TEXT NOT NULL,document TEXT NOT NULL);
CREATE UNIQUE INDEX goal_one_running_scope ON goal_runs(scope_id) WHERE state IN('queued','running','cancel_requested','outcome_unknown');
CREATE TABLE goal_run_nodes(run_id TEXT NOT NULL REFERENCES goal_runs(id),plan_revision INTEGER NOT NULL CHECK(plan_revision>0),node_id TEXT NOT NULL,ordinal INTEGER NOT NULL,state TEXT NOT NULL,session_id TEXT REFERENCES sessions(id),attempt_id TEXT REFERENCES attempts(id),document TEXT NOT NULL,PRIMARY KEY(run_id,plan_revision,node_id));
CREATE TABLE goal_run_reservations(attempt_id TEXT PRIMARY KEY REFERENCES attempts(id),run_id TEXT NOT NULL REFERENCES goal_runs(id),node_id TEXT NOT NULL,parent_node_id TEXT,document TEXT NOT NULL);
PRAGMA user_version=11;").map_err(db_error)?;
        tx.commit().map_err(db_error)?;
    }
    Ok(())
}
fn connection_summary(refs: &GoalRefs) -> Result<Value> {
    let id = refs.connection["id"].as_str().ok_or_else(changed)?;
    let binding = refs
        .grant_after
        .connection_bindings
        .get(id)
        .ok_or_else(changed)?;
    let mut summary = json!(binding);
    summary["id"] = json!(id);
    Ok(summary)
}
fn require_output_mappings(selection: &GoalScopeSelection, plan: &GoalPlanProposal) -> Result<()> {
    for node in &plan.nodes {
        for output in &node.outputs {
            let approved = selection
                .outputs
                .iter()
                .find(|o| o.artifact.artifact_id == output.artifact_id)
                .ok_or_else(|| {
                    err(
                        "CAPABILITY_UNSUPPORTED",
                        "Goal output has no approved physical artifact mapping",
                    )
                })?;
            if output.contract_id != format!("output:{}", approved.artifact.artifact_id) {
                return Err(changed());
            }
        }
    }
    Ok(())
}
fn require_metered_capabilities(plan: &GoalPlanProposal) -> Result<()> {
    if plan
        .nodes
        .iter()
        .any(|n| n.budget.tokens.is_some() || n.budget.cost_microunits.is_some())
    {
        return Err(err("CAPABILITY_UNSUPPORTED","Reliable Provider token/cost measurement is unavailable for a finite Goal node or ancestor cap"));
    }
    Ok(())
}
fn snapshot_base(
    db: &Connection,
    s: &GoalRunSelection,
    data: Option<&crate::data_workspaces::DataInputWitness>,
) -> Result<Value> {
    if sequence(db)? != s.expected_ledger_revision {
        return Err(err(
            "STALE_REVISION",
            "Goal execution ledger changed; review current shared usage",
        ));
    }
    let (r, scope) = read_scope(db, &s.scope_id, data)?;
    let (_, plan) = current_plan(db, &s.scope_id, &scope)?;
    if plan.proposal.revision != s.expected_plan_revision {
        return Err(err("STALE_REVISION", "Goal execution plan changed"));
    }
    if plan.proposal.nodes.is_empty() {
        return Err(invalid());
    }
    require_output_mappings(&r.selection, &plan.proposal)?;
    require_metered_capabilities(&plan.proposal)?;
    // A complete plan is reviewed; neither IDs nor a digest-only surrogate grant
    // unseen nodes or permissions. Large owner reviews must use a separate UI design.
    let v = json!({"intent":s,"scope":scope_view(db,&r,&scope)?,"root_grant":r.refs.grant_after,"existing_connection":connection_summary(&r.refs)?,"plan":plan,"execution":"bounded_existing_workspace_only","allow_in_scope_plan_revisions":true,"new_workspace_authority":false,"enforcement":{"filesystem_sandbox":false,"network_sandbox":false,"output_quota":false},"warning":"Approve autonomous execution of this plan and later validated versions within this exact Goal scope, fixed inputs/capabilities and shared cumulative budgets. Each version requires exact Store compare-and-swap and retained lineage. No new workspace, identity, host or recipient is granted. Node completion is only candidate evidence. Unknown effects and budget overrun block further execution. Output accounting observes declared selected artifacts at stop; it is not an OS filesystem quota."});
    Ok(v)
}
fn snapshot(
    db: &Connection,
    s: &GoalRunSelection,
    data: Option<&crate::data_workspaces::DataInputWitness>,
) -> Result<Value> {
    let v = snapshot_base(db, s, data)?;
    if encode(&v)?.len() > 16_384 {
        return Err(err(
            "RESOURCE_LIMIT",
            "Complete Goal execution review exceeds the unchanged private Owner review bound",
        ));
    }
    Ok(v)
}
impl Store {
    pub fn propose_goal_run(
        &mut self,
        s: &GoalRunSelection,
        key_id: &str,
    ) -> Result<ApprovalProposal> {
        key(key_id)?;
        if let Some(p) = self.goal_execution_proposal_replay(s, key_id)? {
            return Ok(p);
        }
        let v = snapshot(&self.conn, s, self.data_input.as_ref())?;
        self.persist_proposal("goal_execution", v, key_id)
    }
    pub(crate) fn finalize_goal_run(&mut self, id: &str, revision: u64) -> Result<Value> {
        self.finalize_goal_run_kind(id, revision, false)
    }
    fn finalize_goal_run_kind(&mut self, id: &str, revision: u64, graph: bool) -> Result<Value> {
        let kind = if graph {
            "goal_graph_execution"
        } else {
            "goal_execution"
        };
        key(id)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let mut p: ApprovalProposal = raw(&tx, "approval_proposals", id)?;
        if p.kind != kind || p.id != id || p.revision != revision {
            return Err(changed());
        }
        let old:Option<String>=tx.query_row("SELECT result FROM approval_proposals WHERE id=?1 AND COALESCE(octet_length(result),0)<=1048576",[id],|r|r.get(0)).map_err(db_error)?;
        if let Some(old) = old {
            let v: Value = decode(&old)?;
            let result_id = if graph { &v["run"]["id"] } else { &v["id"] };
            if !p.consumed || result_id != id {
                return Err(changed());
            }
            return Ok(v);
        }
        if p.snapshot["execution"]
            != if graph {
                "goal_graph_v1_existing_workspace"
            } else {
                "bounded_existing_workspace_only"
            }
            || p.snapshot["allow_in_scope_plan_revisions"] != true
            || p.snapshot["new_workspace_authority"] != false
            || p.snapshot["enforcement"]
                != json!({"filesystem_sandbox":false,"network_sandbox":false,"output_quota":false})
        {
            return Err(changed());
        }
        if p.consumed || p.expires_at_ms <= ms()? {
            return Err(denied());
        }
        let s: GoalRunSelection =
            serde_json::from_value(p.snapshot["intent"].clone()).map_err(|_| changed())?;
        let seq = sequence(&tx)?;
        if seq
            != s.expected_ledger_revision
                .checked_add(1)
                .ok_or_else(limited)?
        {
            return Err(err(
                "STALE_REVISION",
                "Shared execution state changed after preparing the native review",
            ));
        }
        let own:bool=tx.query_row("SELECT json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='approval.proposed' AND json_extract(document,'$.source')='user_action' FROM events WHERE sequence=?2",params![id,sql_num(seq)?],|r|r.get(0)).map_err(db_error)?;
        if !own {
            return Err(changed());
        }
        if graph {
            graph::check_finalization_snapshot(&tx, &p, self.data_input.as_ref())?;
        }
        let (scope_record, scope) = read_scope(&tx, &s.scope_id, self.data_input.as_ref())?;
        let (_, plan) = current_plan(&tx, &s.scope_id, &scope)?;
        if plan.proposal.nodes.is_empty() {
            return Err(invalid());
        }
        if plan.proposal.revision != s.expected_plan_revision
            || p.snapshot["plan"] != json!(plan)
            || p.snapshot["scope"] != json!(scope_view(&tx, &scope_record, &scope)?)
            || p.snapshot["root_grant"] != json!(scope_record.refs.grant_after)
            || p.snapshot["existing_connection"] != connection_summary(&scope_record.refs)?
        {
            return Err(changed());
        }
        let t = now()?;
        let r = GoalRun {
            id: id.into(),
            scope_id: s.scope_id,
            project_id: scope_record.refs.project_id.clone(),
            task_id: scope_record.refs.task_id.clone(),
            root_grant_id: scope_record.refs.grant_after.id.clone(),
            plan_revision: plan.proposal.revision,
            plan_digest: plan.digest,
            revision: 1,
            state: "queued".into(),
            cancel_requested: false,
            task_accepted: false,
            created_at: t.clone(),
            updated_at: t,
        };
        tx.execute(
            "INSERT INTO goal_runs VALUES(?1,?2,?3,?4,1,'queued',?5)",
            params![r.id, r.scope_id, r.project_id, r.task_id, encode(&r)?],
        )
        .map_err(db_error)?;
        for (i, node) in plan.proposal.nodes.into_iter().enumerate() {
            let n = RuntimeNode {
                node,
                plan_revision: plan.proposal.revision,
                state: "not_started".into(),
                session_id: None,
                attempt_id: None,
                started_at_ms: None,
                stop_confirmed: false,
                actual_runtime_ms: None,
                actual_output_bytes: None,
                tokens: None,
                cost_microunits: None,
            };
            tx.execute(
                "INSERT INTO goal_run_nodes VALUES(?1,?2,?3,?4,'not_started',NULL,NULL,?5)",
                params![
                    r.id,
                    sql_num(r.plan_revision)?,
                    n.node.id,
                    i as i64,
                    encode(&n)?
                ],
            )
            .map_err(db_error)?;
        }
        p.consumed = true;
        let result = if graph {
            json!(GraphRunSummary {
                graph_contract_version: 1,
                run: r.clone(),
                ledger_revision: seq.checked_add(2).ok_or_else(limited)?,
                task_accepted: false
            })
        } else {
            json!(r)
        };
        tx.execute(
            "UPDATE approval_proposals SET document=?1,result=?2 WHERE id=?3 AND result IS NULL",
            params![encode(&p)?, encode(&result)?, id],
        )
        .map_err(db_error)?;
        event(
            &tx,
            id,
            1,
            "approval.confirmed",
            json!({"kind":kind,"run_id":id,"plan_digest":r.plan_digest,"execution_snapshot_digest":sha(&p.snapshot)?,"new_workspace_authority":false}),
        )?;
        event(
            &tx,
            id,
            1,
            "goal.run_authorized",
            json!({"scope_id":r.scope_id,"plan_digest":r.plan_digest,"plan_revision":r.plan_revision,"execution_snapshot_digest":sha(&p.snapshot)?,"reservation_made":false,"dispatched":false,"task_accepted":false}),
        )?;
        if graph {
            graph::persist_capability(&tx, &p)?;
        }
        tx.commit().map_err(db_error)?;
        Ok(result)
    }
    pub fn get_goal_run(&self, id: &str) -> Result<GoalRun> {
        graph::require_legacy_run(&self.conn, id)?;
        run(&self.conn, id)
    }
    pub fn goal_run_details(&self, id: &str) -> Result<Value> {
        graph::require_legacy_run(&self.conn, id)?;
        Ok(
            json!({"run":run(&self.conn,id)?,"nodes":self.goal_node_history(&run(&self.conn,id)?)?,"ledger_revision":sequence(&self.conn)?,"usage_kind":"conservative_charged_reservations","task_accepted":false}),
        )
    }
    pub fn list_goal_runs(&self, task_id: &str) -> Result<Vec<GoalRun>> {
        key(task_id)?;
        raw_collection_budget(
            &self.conn,
            "SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)+octet_length(scope_id)+octet_length(project_id)+octet_length(task_id)+octet_length(state)),0),COALESCE(max(octet_length(document)),0) FROM goal_runs WHERE task_id=?1 AND NOT EXISTS(SELECT 1 FROM goal_graph_capabilities g WHERE g.run_id=goal_runs.id) AND NOT EXISTS(SELECT 1 FROM factory_child_goal_runs f WHERE f.goal_run_id=goal_runs.id)",
            [task_id],
            256,
            MAX_STORED_BYTES,
        )?;
        let mut q = self
            .conn
            .prepare("SELECT id FROM goal_runs WHERE task_id=?1 AND NOT EXISTS(SELECT 1 FROM goal_graph_capabilities g WHERE g.run_id=goal_runs.id) AND NOT EXISTS(SELECT 1 FROM factory_child_goal_runs f WHERE f.goal_run_id=goal_runs.id) ORDER BY rowid DESC LIMIT 257")
            .map_err(db_error)?;
        let ids = q
            .query_map([task_id], |r| r.get::<_, String>(0))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        if ids.len() > 256 {
            return Err(limited());
        }
        ids.iter().map(|id| run(&self.conn, id)).collect()
    }
    pub fn cancel_goal_run(&mut self, id: &str, revision: u64, key_id: &str) -> Result<GoalRun> {
        self.record_goal_cancel(id, revision, key_id)
            .map(|(r, _)| r)
    }
    fn record_goal_cancel(
        &mut self,
        id: &str,
        revision: u64,
        key_id: &str,
    ) -> Result<(GoalRun, bool)> {
        self.record_goal_cancel_kind(id, revision, key_id, false)
    }
    fn record_goal_cancel_kind(
        &mut self,
        id: &str,
        revision: u64,
        key_id: &str,
        graph: bool,
    ) -> Result<(GoalRun, bool)> {
        key(key_id)?;
        if graph && factory::marked_run(&self.conn, id)? {
            return Err(denied());
        }
        if graph {
            graph::checked_capability(&self.conn, &run(&self.conn, id)?)?;
        } else {
            graph::require_legacy_run(&self.conn, id)?;
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let scope = format!("goal-run:{id}:cancel");
        let payload = encode(&json!({"revision":revision}))?;
        if let Some(v) = replay(&tx, &scope, key_id, &payload)? {
            return Ok((v, false));
        }
        let mut r = run(&tx, id)?;
        if revision != r.revision {
            return Err(err(
                "STALE_REVISION",
                "Goal run changed before cancellation",
            ));
        }
        if r.task_accepted || r.state == "accepted" {
            return Err(denied());
        }
        r.cancel_requested = true;
        if r.state != "outcome_unknown" {
            r.state = "cancel_requested".into();
        }
        save_run(
            &tx,
            &mut r,
            "goal.run_cancel_requested",
            json!({"stop_confirmed":false}),
        )?;
        remember(&tx, &scope, key_id, &payload, &r)?;
        tx.commit().map_err(db_error)?;
        Ok((r, true))
    }
    pub(crate) fn cancel_goal_with_stops(
        &mut self,
        id: &str,
        revision: u64,
        key_id: &str,
    ) -> Result<(GoalRun, Vec<String>)> {
        let (receipt, new) = self.record_goal_cancel(id, revision, key_id)?;
        if !new {
            return Ok((receipt, Vec::new()));
        }
        match self.goal_stop_requests() {
            Ok(stops) => Ok((receipt, stops)),
            Err(mut error) => {
                if error.side_effect_state == "not_started" {
                    error.side_effect_state = "confirmed".into();
                }
                let previous = error.details_ref.take();
                error.details_ref = Some(match previous {
                    Some(p) => format!("recorded-goal-cancel:{id};{p}"),
                    None => format!("recorded-goal-cancel:{id}"),
                });
                error.message=format!("{}; cancellation intent is durably recorded, but owned stop remains unconfirmed. Recover the original key and read the existing run",error.message);
                Err(error)
            }
        }
    }
}

pub(crate) fn owns_session(db: &Connection, id: &str) -> Result<bool> {
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM goal_run_nodes WHERE session_id=?1)",
        [id],
        |r| r.get(0),
    )
    .map_err(db_error)
}
pub(crate) fn permits_continuing_task(
    db: &Connection,
    intent: &crate::execution::DispatchIntent,
) -> Result<bool> {
    db.query_row("SELECT EXISTS(SELECT 1 FROM goal_run_nodes n JOIN sessions s ON s.id=n.session_id JOIN goal_runs r ON r.id=n.run_id WHERE json_extract(s.document,'$.intent_id')=?1 AND r.task_id=?2 AND r.state='running' AND n.state IN('reserved','running','stopping')) OR EXISTS(SELECT 1 FROM goal_graph_jobs j JOIN goal_run_nodes n ON n.attempt_id=j.attempt_id AND n.run_id=j.run_id JOIN goal_runs r ON r.id=j.run_id WHERE (EXISTS(SELECT 1 FROM goal_graph_capabilities c WHERE c.run_id=r.id) OR EXISTS(SELECT 1 FROM factory_child_goal_runs f WHERE f.goal_run_id=r.id)) AND json_extract(j.document,'$.job_kind')='validator' AND json_extract(j.document,'$.intent_id')=?1 AND r.task_id=?2 AND r.state='running' AND n.state IN('reserved','running','stopping'))",params![intent.id,intent.task_id],|r|r.get(0)).map_err(db_error)
}

fn checked_runtime_scope(
    db: &Connection,
    r: &GoalRun,
    data: Option<&crate::data_workspaces::DataInputWitness>,
) -> Result<(ScopeRecord, ApprovedGoalScope)> {
    checked_runtime_scope_at(db, r, data, true)
}
fn checked_runtime_scope_at(
    db: &Connection,
    r: &GoalRun,
    data: Option<&crate::data_workspaces::DataInputWitness>,
    require_head: bool,
) -> Result<(ScopeRecord, ApprovedGoalScope)> {
    if factory::marked_run(db, &r.id)? {
        return factory::live_scope(db, r, data, require_head);
    }
    let graph = graph::is_graph_run(db, &r.id)?;
    let kind = if graph {
        "goal_graph_execution"
    } else {
        "goal_execution"
    };
    let (p, approved) = native_confirmation(db, &r.id, kind)?;
    let approved = if graph {
        graph::checked_capability(db, r)?;
        approved["run"].clone()
    } else {
        approved
    };
    if approved["scope_id"] != r.scope_id
        || approved["task_id"] != r.task_id
        || approved["root_grant_id"] != r.root_grant_id
        || p.snapshot["intent"]["scope_id"] != r.scope_id
    {
        return Err(changed());
    }
    if p.snapshot["plan"]["digest"] != approved["plan_digest"]
        || p.snapshot["plan"]["proposal"]["revision"] != approved["plan_revision"]
    {
        return Err(changed());
    }
    let confirmation:Option<i64>=db.query_row("SELECT sequence FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.kind')='approval.confirmed' AND json_extract(document,'$.source')='local_owner_confirmation' AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.payload.kind')=?4 AND json_extract(document,'$.payload.run_id')=?1 AND json_extract(document,'$.payload.plan_digest')=?2 AND json_extract(document,'$.payload.execution_snapshot_digest')=?3 ORDER BY sequence LIMIT 1",params![r.id,approved["plan_digest"].as_str().ok_or_else(changed)?,sha(&p.snapshot)?,kind],|row|row.get(0)).optional().map_err(db_error)?;
    let authorized:Option<i64>=db.query_row("SELECT sequence FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.kind')='goal.run_authorized' AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.payload.scope_id')=?2 AND json_extract(document,'$.payload.plan_digest')=?3 AND json_extract(document,'$.payload.plan_revision')=?4 AND json_extract(document,'$.payload.dispatched')=0 AND json_extract(document,'$.payload.reservation_made')=0 AND json_extract(document,'$.payload.execution_snapshot_digest')=?5 ORDER BY sequence LIMIT 1",params![r.id,r.scope_id,approved["plan_digest"].as_str().ok_or_else(changed)?,approved["plan_revision"].as_i64().ok_or_else(changed)?,sha(&p.snapshot)?],|row|row.get(0)).optional().map_err(db_error)?;
    if !matches!((confirmation,authorized),(Some(a),Some(b)) if a>0 && b>a) {
        return Err(denied());
    }
    verify_plan_advances(db, r, &p, &approved, authorized.ok_or_else(denied)?)?;
    stored_scope_view(db, &r.scope_id)?;
    let record: ScopeRecord = raw(db, "goal_scopes", &r.scope_id)?;
    if record.revoked
        || record.refs.task_id != r.task_id
        || record.refs.project_id != r.project_id
        || record.refs.grant_after.id != r.root_grant_id
    {
        return Err(denied());
    }
    let task: Task = raw(db, "tasks", &r.task_id)?;
    if task.spec_revision != record.refs.spec_revision
        || task.goal != record.refs.task_goal
        || matches!(
            task.state,
            TaskState::Accepted
                | TaskState::CancelRequested
                | TaskState::Cancelled
                | TaskState::OutcomeUnknown
        )
    {
        return Err(changed());
    }
    let latest = crate::policy::validate_grant_live_from_with_data(
        db,
        &r.root_grant_id,
        &r.project_id,
        &record.refs.workspace.id,
        &record.refs.grant_after.scope.connection_ids[0],
        data,
    )?;
    if latest != record.refs.grant_after {
        return Err(changed());
    }
    let project: Project = raw(db, "projects", &r.project_id)?;
    if project.archived || project.revision != record.refs.project_revision {
        return Err(changed());
    }
    let workspace: ManagedWorkspace = raw(db, "managed_workspaces", &record.refs.workspace.id)?;
    if encode(&workspace)? != encode(&record.refs.workspace)? {
        return Err(changed());
    }
    for input in &record.refs.inputs {
        let actual = if matches!(
            workspace.identity.as_ref().map(|i| &i.kind),
            Some(agentbaton_workspaces::WorkspaceKind::Directory)
        ) {
            data.ok_or_else(changed)?
                .selected_input_hash(db, &workspace, &input.artifact)?
        } else {
            agentbaton_workspaces::validation::sha256(
                &agentbaton_workspaces::read_artifact(
                    &workspace.root,
                    &input.artifact.relative_path,
                    input.artifact.max_bytes,
                )
                .map_err(|_| changed())?,
            )
        };
        if actual != input.sha256 {
            return Err(changed());
        }
    }
    let scope = approved_scope(db, &task, &record)?;
    let (_, plan) = current_plan(db, &r.scope_id, &scope)?;
    require_output_mappings(&record.selection, &plan.proposal)?;
    require_metered_capabilities(&plan.proposal)?;
    if require_head && (plan.proposal.revision != r.plan_revision || plan.digest != r.plan_digest) {
        return Err(err("STALE_REVISION","An in-scope plan revision must advance atomically before dispatch; old receipts never start it"));
    }
    Ok((record, scope))
}
// This metadata can only pause scheduling; it cannot authorize a new plan.
fn adopted_plan_is_current_head(db: &Connection, r: &GoalRun) -> Result<bool> {
    if !factory::adopted_head_is_current(db, r)? {
        return Ok(false);
    }
    db.query_row("SELECT EXISTS(SELECT 1 FROM goal_plan_heads WHERE scope_id=?1 AND revision=?2 AND digest=?3)",params![r.scope_id,sql_num(r.plan_revision)?,r.plan_digest],|row|row.get(0)).map_err(db_error)
}
fn local_goal_slot_is_occupied(db: &Connection) -> Result<bool> {
    db.query_row("SELECT EXISTS(SELECT 1 FROM dispatch_intents WHERE state!='stopped') OR EXISTS(SELECT 1 FROM command_validation_runs WHERE state IN('prepared','running','cancel_requested','outcome_unknown')) OR EXISTS(SELECT 1 FROM goal_graph_jobs WHERE state IN('prepared','running','cancel_requested','outcome_unknown'))",[],|row|row.get(0)).map_err(db_error)
}
fn executable_work(n: &RuntimeNode) -> bool {
    n.state == "not_started"
        && n.node.dependencies.is_empty()
        && n.node.parent_id.is_none()
        && matches!(n.node.kind, NodeKind::Work | NodeKind::Summary)
        && matches!(n.node.executor, Executor::Connection { .. })
        && n.node
            .required_inputs
            .iter()
            .all(|input| matches!(input.source, InputSource::ApprovedExternal { .. }))
}
impl Store {
    pub(crate) fn goal_dispatch_context(
        &self,
        p: &GoalRunDispatch,
        revision: u64,
    ) -> Result<Option<GoalDispatchContext>> {
        let r = run(&self.conn, &p.run_id)?;
        if r.revision != revision || sequence(&self.conn)? != p.expected_ledger_revision {
            return Err(err(
                "STALE_REVISION",
                "Goal run or reservation ledger changed",
            ));
        }
        if r.cancel_requested || !matches!(r.state.as_str(), "queued" | "running") {
            return Ok(None);
        }
        let nodes = node_records(&self.conn, &r.id)?;
        if nodes.iter().any(|n| {
            matches!(
                n.state.as_str(),
                "reserved" | "running" | "stopping" | "outcome_unknown"
            )
        }) {
            return Ok(None);
        }
        if local_goal_slot_is_occupied(&self.conn)? {
            return Ok(None);
        }
        checked_runtime_scope(&self.conn, &r, self.data_input.as_ref())?;
        let selected = if graph::is_graph_run(&self.conn, &r.id)? {
            graph::ready_connection_node(&self.conn, &r, &nodes)?
        } else {
            nodes.iter().find(|n| executable_work(n))
        };
        let Some(n) = selected else {
            return Ok(None);
        };
        if n.node.external_action_id.is_some()
            || matches!(
                n.node.side_effect,
                SideEffect::ExternalWrite | SideEffect::Production
            )
        {
            return Err(denied());
        }
        if !graph::is_graph_run(&self.conn, &r.id)?
            && n.node
                .required_inputs
                .iter()
                .any(|i| !matches!(i.source, InputSource::ApprovedExternal { .. }))
        {
            return Ok(None);
        }
        Ok(Some(GoalDispatchContext {
            run: r,
            node: n.node.clone(),
            ledger_revision: p.expected_ledger_revision,
        }))
    }
    pub(crate) fn create_goal_session(
        &mut self,
        c: GoalDispatchContext,
    ) -> Result<crate::sessions::ManagedSession> {
        let Executor::Connection { binding, .. } = &c.node.executor else {
            return Err(denied());
        };
        let task = self.get(&c.run.task_id)?;
        let task_id = task.id.clone();
        let workspace = binding.workspace_id.clone();
        let connection = binding.connection_id.clone();
        let grant = c.run.root_grant_id.clone();
        let key = format!(
            "goal-session:{}",
            sha(&(c.run.id.clone(), c.run.plan_revision, c.node.id.clone()))?
        );
        self.create_session_for_goal(
            (
                &task_id,
                &workspace,
                &connection,
                &grant,
                task.revision,
                &key,
            ),
            c,
        )
    }
}
#[derive(Debug, Serialize, Deserialize)]
struct ReservationRecord {
    #[serde(skip)]
    shared_root_grant_id: Option<String>,
    run_id: String,
    node_id: String,
    plan_revision: u64,
    ancestors: Vec<String>,
    reservation: AttemptReservation,
    settled: bool,
    actual_runtime_ms: Option<u64>,
    actual_output_bytes: Option<u64>,
    tokens: Option<u64>,
    cost_microunits: Option<u64>,
}
fn reservations(db: &Connection, root: &str, project: &str) -> Result<Vec<ReservationRecord>> {
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(x.document)+octet_length(x.attempt_id)+octet_length(x.run_id)+octet_length(x.node_id)+COALESCE(octet_length(x.parent_node_id),0)),0),COALESCE(max(octet_length(x.document)),0) FROM goal_run_reservations x JOIN goal_runs r ON r.id=x.run_id WHERE r.project_id=?1 OR json_extract(r.document,'$.root_grant_id')=?2",params![project,root],100000,MAX_GOAL_PLAN_BYTES as i64)?;
    let mut q=db.prepare("SELECT CASE WHEN octet_length(x.document)<=262144 THEN x.document END FROM goal_run_reservations x JOIN goal_runs r ON r.id=x.run_id WHERE r.project_id=?1 OR json_extract(r.document,'$.root_grant_id')=?2 LIMIT 100001").map_err(db_error)?;
    let docs = q
        .query_map(params![project, root], |r| r.get::<_, Option<String>>(0))
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    if docs.len() > 100000 {
        return Err(limited());
    }
    let mut records = docs
        .into_iter()
        .map(|x| decode(&x.ok_or_else(limited)?))
        .collect::<Result<Vec<ReservationRecord>>>()?;
    for shared in crate::factory::budget_records(db, root, project)? {
        // These are the very same Goal rows already materialized above. Their
        // Factory ancestry projection must never charge the common ledger twice.
        if shared.authority_kind == "factory_child_goal" {
            continue;
        }
        records.push(ReservationRecord {
            shared_root_grant_id: Some(shared.root_grant_id.clone()),
            run_id: format!("shared-session:{}", shared.attempt_id),
            node_id: shared.task_id.clone(),
            plan_revision: 0,
            ancestors: shared.ancestors.clone(),
            reservation: shared.reservation.clone(),
            settled: shared.settled,
            actual_runtime_ms: shared.actual_runtime_ms,
            actual_output_bytes: shared.actual_output_bytes,
            tokens: None,
            cost_microunits: None,
        });
    }
    Ok(records)
}
fn reserve_meter(
    db: &Connection,
    key_id: &str,
    limits: &BudgetLimits,
    before_count: u32,
    res: &AttemptReservation,
    active: &[&ReservationRecord],
) -> Result<()> {
    let mut m = meter(db, key_id, limits, before_count)?;
    let runtime = m
        .runtime_ms
        .ok_or_else(|| err("BUDGET_UNKNOWN", "Historical actual runtime is unknown"))?;
    let output = m
        .output_bytes
        .ok_or_else(|| err("BUDGET_UNKNOWN", "Historical actual output is unknown"))?;
    let mut runtimes = runtime;
    let mut outputs = output;
    let mut tokens = m.tokens;
    let mut cost = m.cost_microunits;
    for v in active {
        runtimes = runtimes
            .checked_add(v.reservation.runtime_ms)
            .ok_or_else(limited)?;
        outputs = outputs
            .checked_add(v.reservation.output_bytes)
            .ok_or_else(limited)?;
        tokens = tokens.and_then(|a| v.reservation.tokens.and_then(|b| a.checked_add(b)));
        cost = cost.and_then(|a| v.reservation.cost_microunits.and_then(|b| a.checked_add(b)));
    }
    if runtimes
        .checked_add(res.runtime_ms)
        .is_none_or(|v| v > limits.runtime_ms)
        || outputs
            .checked_add(res.output_bytes)
            .is_none_or(|v| v > limits.output_bytes)
        || before_count >= limits.attempts
    {
        return Err(err(
            "RESOURCE_LIMIT",
            "Cumulative shared Goal budget is exhausted",
        ));
    }
    for (maximum, used, needed) in [
        (limits.tokens, tokens, res.tokens),
        (limits.cost_microunits, cost, res.cost_microunits),
    ] {
        if maximum.is_some_and(|max| {
            used.zip(needed)
                .and_then(|(a, b)| a.checked_add(b))
                .is_none_or(|v| v > max)
        }) {
            return Err(err(
                "BUDGET_UNKNOWN",
                "Provider token/cost usage or bounded reservation is unavailable",
            ));
        }
    }
    m.observed_attempts = before_count.checked_add(1).ok_or_else(limited)?;
    db.execute(
        "UPDATE goal_ledgers SET document=?1 WHERE scope=?2",
        params![encode(&m)?, key_id],
    )
    .map_err(db_error)?;
    Ok(())
}
pub(crate) fn reserve_session(
    db: &Connection,
    c: &GoalDispatchContext,
    s: &crate::sessions::ManagedSession,
    binding: &ExecutionBinding,
    data: Option<&crate::data_workspaces::DataInputWitness>,
    graph_input: Option<&GraphReadWitness>,
) -> Result<()> {
    reserve_attempt(
        db,
        c,
        GoalAttemptTarget {
            attempt_id: &s.attempt_id,
            intent_id: &s.intent_id,
            task_id: &s.task_id,
            grant_id: &s.grant_id,
            session: Some(s),
        },
        binding,
        data,
        graph_input,
    )
}
struct GoalAttemptTarget<'a> {
    attempt_id: &'a str,
    intent_id: &'a str,
    task_id: &'a str,
    grant_id: &'a str,
    session: Option<&'a crate::sessions::ManagedSession>,
}
/// Full strict readset consumption before generic attempt counters/rows are
/// changed, inside the SAME SQLite transaction. Native/ordinary paths retain
/// their original checks and never acquire the Factory phase exception.
pub(crate) fn preflight_factory_goal_reservation(
    db: &Connection,
    c: &GoalDispatchContext,
    data: Option<&crate::data_workspaces::DataInputWitness>,
) -> Result<()> {
    if !factory::marked_run(db, &c.run.id)? {
        return Ok(());
    }
    let current = run(db, &c.run.id)?;
    if current.revision != c.run.revision
        || current.plan_revision != c.run.plan_revision
        || current.plan_digest != c.run.plan_digest
        || current.task_id != c.run.task_id
        || current.root_grant_id != c.run.root_grant_id
        || current.cancel_requested
        || sequence(db)? != c.ledger_revision
    {
        return Err(changed());
    }
    checked_runtime_scope(db, &current, data)?;
    let n = node_records(db, &current.id)?
        .into_iter()
        .find(|n| n.node.id == c.node.id)
        .ok_or_else(changed)?;
    if n.node != c.node
        || n.state != "not_started"
        || n.session_id.is_some()
        || n.attempt_id.is_some()
    {
        return Err(changed());
    }
    Ok(())
}
/// Only the atomic reservation code constructs this phase token. No wire or
/// legacy/native caller can request a relaxed business-read comparison.
pub(crate) struct GoalReservationRead<'a> {
    context: &'a GoalDispatchContext,
    attempt_id: &'a str,
    intent_id: &'a str,
    session: Option<&'a crate::sessions::ManagedSession>,
    binding: &'a ExecutionBinding,
}
impl GoalReservationRead<'_> {
    pub(crate) fn run(&self) -> &GoalRun {
        &self.context.run
    }
    pub(crate) fn node(&self) -> &PlanNode {
        &self.context.node
    }
    pub(crate) fn attempt_id(&self) -> &str {
        self.attempt_id
    }
    pub(crate) fn intent_id(&self) -> &str {
        self.intent_id
    }
    pub(crate) fn session(&self) -> Option<&crate::sessions::ManagedSession> {
        self.session
    }
    pub(crate) fn binding(&self) -> &ExecutionBinding {
        self.binding
    }
}
fn reserve_attempt(
    db: &Connection,
    c: &GoalDispatchContext,
    target: GoalAttemptTarget<'_>,
    binding: &ExecutionBinding,
    data: Option<&crate::data_workspaces::DataInputWitness>,
    graph_input: Option<&GraphReadWitness>,
) -> Result<()> {
    let mut r = run(db, &c.run.id)?;
    if r.revision != c.run.revision
        || r.plan_digest != c.run.plan_digest
        || r.cancel_requested
        || sequence(db)? != c.ledger_revision
    {
        return Err(err(
            "STALE_REVISION",
            "Goal dispatch changed before atomic reservation",
        ));
    }
    let nodes = node_records(db, &r.id)?;
    let mut n = nodes
        .into_iter()
        .find(|n| n.node.id == c.node.id && n.state == "not_started")
        .ok_or_else(changed)?;
    if n.node != c.node || target.task_id != r.task_id || target.grant_id != r.root_grant_id {
        return Err(changed());
    }
    let phase = GoalReservationRead {
        context: c,
        attempt_id: target.attempt_id,
        intent_id: target.intent_id,
        session: target.session,
        binding,
    };
    let (scope, approved) = if factory::marked_run(db, &r.id)? {
        factory::live_scope_for_reservation(db, &r, data, &phase)?
    } else {
        checked_runtime_scope(db, &r, data)?
    };
    if graph::is_graph_run(db, &r.id)? {
        graph::require_attempt_read(db, &r, &c.node, graph_input, target.session)?;
    }
    let expected = match &n.node.executor {
        Executor::Connection { binding, .. } => binding.as_ref().clone(),
        Executor::Validator { .. } if graph::is_graph_run(db, &r.id)? => {
            scope_view(db, &scope, &approved)?.binding
        }
        _ => return Err(denied()),
    };
    if binding != &expected {
        return Err(changed());
    }
    let all_active: u32 = db
        .query_row(
            "SELECT (SELECT count(*) FROM dispatch_intents WHERE state!='stopped')+(SELECT count(*) FROM command_validation_runs WHERE state IN('prepared','running','cancel_requested','outcome_unknown'))+(SELECT count(*) FROM goal_graph_jobs WHERE state IN('prepared','running','cancel_requested','outcome_unknown'))",
            [],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    if all_active > 1 {
        return Err(err(
            "WORKSPACE_BUSY",
            "The approved conservative local Goal resource limit is occupied",
        ));
    }
    refuse_retained_overrun(db, &r.root_grant_id, &r.project_id)?;
    let records = reservations(db, &r.root_grant_id, &r.project_id)?;
    let task: Task = raw(db, "tasks", &r.task_id)?;
    let (pc, _, rc, _) = counts(db, &task, &scope.refs.grant_after)?;
    let project_active = records.iter().filter(|x| !x.settled).collect::<Vec<_>>();
    let mut root_active = Vec::new();
    for x in records.iter().filter(|x| !x.settled) {
        let own = if let Some(shared_root) = &x.shared_root_grant_id {
            shared_root == &r.root_grant_id
        } else {
            db.query_row(
                "SELECT json_extract(document,'$.root_grant_id')=?1 FROM goal_runs WHERE id=?2",
                params![r.root_grant_id, x.run_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(db_error)?
        };
        if own {
            root_active.push(x);
        }
    }
    reserve_meter(
        db,
        &format!("project:{}", r.project_id),
        &scope.selection.project_budget,
        pc.checked_sub(1).ok_or_else(changed)?,
        &n.node.reservation,
        &project_active,
    )?;
    reserve_meter(
        db,
        &format!("root-grant:{}", r.root_grant_id),
        &scope.selection.root_budget,
        rc.checked_sub(1).ok_or_else(changed)?,
        &n.node.reservation,
        &root_active,
    )?;
    let mut ancestors = vec![n.node.id.clone()];
    let mut parent = n.node.parent_id.clone();
    while let Some(p) = parent {
        if ancestors.contains(&p) || ancestors.len() > 256 {
            return Err(changed());
        }
        ancestors.push(p.clone());
        let old = node_records(db, &r.id)?
            .into_iter()
            .find(|x| x.node.id == p)
            .ok_or_else(changed)?;
        parent = old.node.parent_id;
    }
    for ancestor in &ancestors {
        let budget = node_records(db, &r.id)?
            .into_iter()
            .find(|x| &x.node.id == ancestor)
            .ok_or_else(changed)?
            .node
            .budget;
        let relevant = records
            .iter()
            .filter(|x| x.run_id == r.id && x.ancestors.contains(ancestor))
            .collect::<Vec<_>>();
        let used_runtime = relevant
            .iter()
            .try_fold(0u64, |a, x| {
                a.checked_add(x.actual_runtime_ms.unwrap_or(x.reservation.runtime_ms))
            })
            .ok_or_else(limited)?;
        let used_output = relevant
            .iter()
            .try_fold(0u64, |a, x| {
                a.checked_add(x.actual_output_bytes.unwrap_or(x.reservation.output_bytes))
            })
            .ok_or_else(limited)?;
        for (maximum, needed, is_token) in [
            (budget.tokens, n.node.reservation.tokens, true),
            (
                budget.cost_microunits,
                n.node.reservation.cost_microunits,
                false,
            ),
        ] {
            if let Some(max) = maximum {
                let mut used = Some(0u64);
                for old in &relevant {
                    let amount = if old.settled {
                        if is_token {
                            old.tokens
                        } else {
                            old.cost_microunits
                        }
                    } else if is_token {
                        old.reservation.tokens
                    } else {
                        old.reservation.cost_microunits
                    };
                    used = used.and_then(|a| amount.and_then(|b| a.checked_add(b)));
                }
                if used
                    .zip(needed)
                    .and_then(|(a, b)| a.checked_add(b))
                    .is_none_or(|sum| sum > max)
                {
                    return Err(err("BUDGET_UNKNOWN","Ancestor token/cost usage or reservation is unknown or exceeds its finite bound"));
                }
            }
        }
        if ancestor != &n.node.id {
            let descendants = relevant
                .iter()
                .filter(|x| x.node_id != *ancestor)
                .map(|x| x.node_id.as_str())
                .collect::<BTreeSet<_>>();
            if !descendants.contains(n.node.id.as_str())
                && descendants.len() >= budget.children as usize
            {
                return Err(err(
                    "RESOURCE_LIMIT",
                    "Ancestor child-node budget is exhausted",
                ));
            }
        }
        let previous_revisions = relevant
            .iter()
            .filter(|old| old.node_id == n.node.id)
            .map(|old| old.plan_revision)
            .collect::<BTreeSet<_>>();
        if previous_revisions.len() > budget.reworks as usize {
            return Err(err(
                "RESOURCE_LIMIT",
                "The cumulative node rework budget is exhausted",
            ));
        }
        if relevant.len() >= budget.attempts as usize
            || relevant.iter().filter(|x| !x.settled).count() >= budget.parallel as usize
            || used_runtime
                .checked_add(n.node.reservation.runtime_ms)
                .is_none_or(|v| v > budget.runtime_ms)
            || used_output
                .checked_add(n.node.reservation.output_bytes)
                .is_none_or(|v| v > budget.output_bytes)
        {
            return Err(err(
                "RESOURCE_LIMIT",
                "Cumulative ancestor Goal budget is exhausted",
            ));
        }
    }
    factory::reserve_ancestors(db, &r, &n.node.reservation)?;
    let receipt = ReservationRecord {
        shared_root_grant_id: None,
        run_id: r.id.clone(),
        node_id: n.node.id.clone(),
        plan_revision: r.plan_revision,
        ancestors,
        reservation: n.node.reservation.clone(),
        settled: false,
        actual_runtime_ms: None,
        actual_output_bytes: None,
        tokens: None,
        cost_microunits: None,
    };
    db.execute(
        "INSERT INTO goal_run_reservations VALUES(?1,?2,?3,?4,?5)",
        params![
            target.attempt_id,
            r.id,
            n.node.id,
            n.node.parent_id,
            encode(&receipt)?
        ],
    )
    .map_err(db_error)?;
    n.state = "reserved".into();
    n.session_id = target.session.map(|s| s.id.clone());
    n.attempt_id = Some(target.attempt_id.to_owned());
    n.started_at_ms = Some(ms()?);
    save_node(db, &r.id, &n)?;
    r.state = "running".into();
    let payload = json!({"node_id":n.node.id,"attempt_id":target.attempt_id,"session_id":target.session.map(|s|&s.id),"plan_revision":r.plan_revision,"plan_digest":r.plan_digest,"reservation":n.node.reservation,"actual_runtime_ms":null,"actual_output_bytes":null,"tokens":null,"cost_microunits":null,"task_accepted":false});
    save_run(db, &mut r, "goal.node_reserved", payload)?;
    Ok(())
}

impl Store {
    pub(crate) fn goal_session_duration(&self, id: &str) -> Result<Option<u64>> {
        if !owns_session(&self.conn, id)? {
            return Ok(None);
        }
        let (run_id,doc):(String,String)=self.conn.query_row("SELECT run_id,document FROM goal_run_nodes WHERE session_id=?1 AND octet_length(document)<=262144",[id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
        let r = run(&self.conn, &run_id)?;
        checked_runtime_scope(&self.conn, &r, self.data_input.as_ref())?;
        let n: RuntimeNode = decode(&doc)?;
        if n.session_id.as_deref() != Some(id) || n.state != "reserved" || r.cancel_requested {
            return Err(denied());
        }
        Ok(Some(n.node.reservation.runtime_ms))
    }
    pub(crate) fn goal_dispatch_session(
        &mut self,
        p: &GoalRunDispatch,
        revision: u64,
    ) -> Result<Option<crate::sessions::ManagedSession>> {
        if let Some(c) = self.goal_dispatch_context(p, revision)? {
            return self.create_goal_session(c).map(Some);
        }
        Ok(None)
    }
    pub(crate) fn goal_prompt(
        &mut self,
        p: &GoalRunDispatch,
        revision: u64,
    ) -> Result<Option<(String, String)>> {
        let r = run(&self.conn, &p.run_id)?;
        if r.revision != revision || sequence(&self.conn)? != p.expected_ledger_revision {
            return Err(err(
                "STALE_REVISION",
                "Goal run or current input/ledger snapshot changed",
            ));
        }
        if r.cancel_requested || r.state != "running" {
            return Ok(None);
        }
        let Some(mut n) = node_records(&self.conn, &r.id)?
            .into_iter()
            .find(|n| n.state == "reserved")
        else {
            return Ok(None);
        };
        let id = n.session_id.clone().ok_or_else(changed)?;
        let s = self.get_session(&id)?;
        if s.state != "ready" || s.turn_state.is_some() {
            return Ok(None);
        }
        checked_runtime_scope(&self.conn, &r, self.data_input.as_ref())?;
        let key = format!(
            "goal-turn:{}",
            sha(&(r.id.clone(), r.plan_revision, n.node.id.clone()))?
        );
        let record: ScopeRecord = raw(&self.conn, "goal_scopes", &r.scope_id)?;
        let is_graph = graph::is_graph_run(&self.conn, &r.id)?;
        let inputs = if is_graph {
            graph::prompt_inputs(self, &r, &n.node)?
        } else {
            json!(record.refs.inputs)
        };
        let mut task_request = json!({"schema_version":1,"kind":"agentbaton_goal_task_request","goal":n.node.goal,"node":n.node,"run_id":r.id,"plan_revision":r.plan_revision,"plan_digest":r.plan_digest,"workspace":{"id":record.refs.workspace.id,"canonical_root":record.refs.workspace.root,"input_revision":record.refs.workspace.base_revision},"inputs":inputs,"outputs":record.selection.outputs,"policy":{"requested":record.refs.grant_after.scope,"enforced":{"filesystem_sandbox":false,"network_sandbox":false,"output_quota":false}},"lineage":{"root_task_id":root_task(&record),"parent_node_id":n.node.parent_id,"attempt_id":s.attempt_id},"budget":{"reservation":n.node.reservation,"limits":n.node.budget},"idempotency_key":key,"return_location":{"run_id":r.id,"node_id":n.node.id,"attempt_id":s.attempt_id}});
        // The new graph capability owns these versioned bindings. Legacy v1
        // requests retain their original exact sixteen-field contract.
        if is_graph {
            task_request["validation_contracts"] =
                graph::prompt_validation_contracts(&record, &n.node, &s)?;
            task_request["task_binding"] = json!({"task_id":r.task_id,"attempt_id":s.attempt_id,"spec_revision":record.refs.spec_revision,"input_revision":record.refs.workspace.base_revision});
        }
        let text = encode(&task_request)?;
        if text.len() > 65536 {
            return Err(err("RESOURCE_LIMIT","Complete Goal TaskRequest exceeds the existing 64KiB turn bound; nothing was truncated"));
        }
        let (_, send) = self.send_goal_session(&id, &text, s.revision, &key)?;
        if !send {
            return Ok(None);
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        n.state = "running".into();
        save_node(&tx, &r.id, &n)?;
        event(
            &tx,
            &r.id,
            r.revision,
            "goal.node_prompt_queued",
            json!({"node_id":n.node.id,"session_id":id,"attempt_id":s.attempt_id,"plan_digest":r.plan_digest,"task_accepted":false}),
        )?;
        tx.commit().map_err(db_error)?;
        Ok(Some((id, text)))
    }
    /// Only a stored pending fixed execution proposal may reach its normal
    /// finalizer while another unrelated root is running. This is no approval.
    pub(crate) fn pending_goal_execution_approval(&self, id: &str) -> Result<bool> {
        key(id)?;
        let row:Option<Option<String>>=self.conn.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM approval_proposals WHERE id=?1 AND result IS NULL",[id],|r|r.get(0)).optional().map_err(db_error)?;
        let Some(raw) = row else { return Ok(false) };
        let proposal: ApprovalProposal = decode(&raw.ok_or_else(limited)?)?;
        Ok(proposal.id == id
            && matches!(
                proposal.kind.as_str(),
                "goal_execution" | "goal_graph_execution"
            )
            && !proposal.consumed)
    }
    pub(crate) fn active_goal_runs(&self) -> Result<Vec<GoalRun>> {
        raw_collection_budget(&self.conn,"SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)+octet_length(scope_id)+octet_length(project_id)+octet_length(task_id)+octet_length(state)),0),COALESCE(max(octet_length(document)),0) FROM goal_runs WHERE (state IN('queued','running','cancel_requested') OR (state='blocked' AND EXISTS(SELECT 1 FROM goal_run_nodes n WHERE n.run_id=goal_runs.id AND n.state IN('reserved','running','stopping'))))",[],256,MAX_STORED_BYTES)?;
        let mut q=self.conn.prepare("SELECT id FROM goal_runs WHERE (state IN('queued','running','cancel_requested') OR (state='blocked' AND EXISTS(SELECT 1 FROM goal_run_nodes n WHERE n.run_id=goal_runs.id AND n.state IN('reserved','running','stopping')))) ORDER BY rowid LIMIT 257").map_err(db_error)?;
        let ids = q
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        if ids.len() > 256 {
            return Err(limited());
        }
        ids.iter().map(|id| run(&self.conn, id)).collect()
    }
    /// Observe a correlated terminal turn, then request owned stop. No terminal
    /// or child process exit is itself a validation/Task acceptance receipt.
    pub(crate) fn goal_stop_requests(&mut self) -> Result<Vec<String>> {
        let mut stop = Vec::new();
        for mut r in self.active_goal_runs()? {
            let task = self.get(&r.task_id)?;
            let revoked = crate::policy::checked_lineage(&self.conn, &r.root_grant_id).is_err()
                || !factory::run_live(&self.conn, &r.id)?;
            let cancel = r.cancel_requested
                || r.state == "blocked"
                || revoked
                || matches!(
                    task.state,
                    TaskState::CancelRequested | TaskState::Cancelled
                );
            let nodes = node_records(&self.conn, &r.id)?;
            let graph_live:i64=self.conn.query_row("SELECT count(*) FROM goal_graph_jobs WHERE run_id=?1 AND state IN('prepared','running','cancel_requested','outcome_unknown')",[&r.id],|row|row.get(0)).map_err(db_error)?;
            let live = usize::try_from(graph_live).map_err(|_| limited())?
                + nodes
                    .iter()
                    .filter(|n| {
                        matches!(
                            n.state.as_str(),
                            "reserved" | "running" | "stopping" | "outcome_unknown"
                        ) || n.session_id.is_some() && !n.stop_confirmed
                    })
                    .count();
            if live == 0 && cancel && r.state != "blocked" {
                let tx = self.conn.transaction().map_err(db_error)?;
                r.cancel_requested = true;
                r.state = "cancelled".into();
                save_run(
                    &tx,
                    &mut r,
                    "goal.run_cancelled",
                    json!({"owned_workers":0,"new_dispatch":false,"task_accepted":false}),
                )?;
                tx.commit().map_err(db_error)?;
                continue;
            }
            if live == 0 && !cancel && !adopted_plan_is_current_head(&self.conn, &r)? {
                let tx = self.conn.transaction().map_err(db_error)?;
                r.state = "awaiting_acceptance".into();
                save_run(
                    &tx,
                    &mut r,
                    "goal.run_plan_pending",
                    json!({"cause":"validated_plan_requires_explicit_adoption","automatic_adoption":false,"new_dispatch":false,"task_accepted":false}),
                )?;
                tx.commit().map_err(db_error)?;
                continue;
            }
            for mut n in nodes {
                if !matches!(n.state.as_str(), "reserved" | "running" | "stopping") {
                    continue;
                }
                let Some(id) = n.session_id.clone() else {
                    continue;
                };
                let s = self.get_session(&id)?;
                if cancel
                    || matches!(
                        s.turn_state.as_deref(),
                        Some("completed" | "cancelled" | "interrupted" | "outcome_unknown")
                    )
                    || matches!(s.state.as_str(), "failed" | "outcome_unknown")
                {
                    if n.state != "stopping" {
                        let tx = self.conn.transaction().map_err(db_error)?;
                        n.state = "stopping".into();
                        save_node(&tx, &r.id, &n)?;
                        event(
                            &tx,
                            &r.id,
                            r.revision,
                            "goal.node_stop_requested",
                            json!({"node_id":n.node.id,"session_id":id,"turn_state":s.turn_state,"stop_confirmed":false,"task_accepted":false}),
                        )?;
                        tx.commit().map_err(db_error)?;
                    }
                    stop.push(id);
                }
            }
        }
        Ok(stop)
    }
    pub(crate) fn settle_goal_worker(
        &mut self,
        id: &str,
        elapsed_ms: u64,
        stopped: bool,
        never_started: bool,
    ) -> Result<()> {
        if owns_session(&self.conn, id)? && stopped && !never_started {
            let run_id: String = self
                .conn
                .query_row(
                    "SELECT run_id FROM goal_run_nodes WHERE session_id=?1",
                    [id],
                    |row| row.get(0),
                )
                .map_err(db_error)?;
            if graph::is_graph_run(&self.conn, &run_id)?
                && self.get_session(id)?.turn_state.as_deref() == Some("completed")
            {
                return self.queue_graph_capture(id, elapsed_ms);
            }
        }
        self.settle_goal_worker_observed(id, elapsed_ms, stopped, never_started, None)
    }
    fn settle_goal_worker_observed(
        &mut self,
        id: &str,
        elapsed_ms: u64,
        stopped: bool,
        never_started: bool,
        observed: Option<Option<u64>>,
    ) -> Result<()> {
        if !owns_session(&self.conn, id)? {
            return Ok(());
        }
        let (run_id,doc):(String,String)=self.conn.query_row("SELECT run_id,document FROM goal_run_nodes WHERE session_id=?1 AND octet_length(document)<=262144",[id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
        let mut r = run(&self.conn, &run_id)?;
        let was_blocked = r.state == "blocked";
        let can_continue = matches!(r.state.as_str(), "queued" | "running") && !r.cancel_requested;
        let prior_nodes = node_records(&self.conn, &run_id)?;
        let mut n: RuntimeNode = decode(&doc)?;
        let session = self.get_session(id)?;
        if !matches!(n.state.as_str(), "reserved" | "running" | "stopping") {
            return Ok(());
        }
        let record: ScopeRecord = raw(&self.conn, "goal_scopes", &r.scope_id)?;
        // Current available selected artifact bytes; cap work on the actor. A
        // missing/unsafe/too-large observation stays unknown rather than zero.
        let output_bytes_observed = if let Some(observed) = observed {
            observed
        } else {
            let root_matches = current_output_root(&self.conn, &record.refs.workspace);
            let mut output_bytes = if root_matches { Some(0u64) } else { None };
            for output in &n.node.outputs {
                let Some(approved) = record.selection.outputs.iter().find(|o| {
                    o.artifact.artifact_id == output.artifact_id
                        && output.contract_id == format!("output:{}", o.artifact.artifact_id)
                }) else {
                    output_bytes = None;
                    break;
                };
                let Some(measured) = output_bytes else { break };
                let limit = approved
                    .artifact
                    .max_bytes
                    .min(1_048_576u64.saturating_sub(measured));
                let pin = record
                    .refs
                    .workspace
                    .identity
                    .as_ref()
                    .ok_or_else(changed)?;
                match read_pinned_output(pin, &approved.artifact.relative_path, limit) {
                    Ok(bytes) => output_bytes = measured.checked_add(bytes),
                    Err(_) => {
                        output_bytes = None;
                        break;
                    }
                }
            }
            if !current_output_root(&self.conn, &record.refs.workspace) {
                output_bytes = None;
            }
            output_bytes
        };
        let mut output_bytes = output_bytes_observed;
        if never_started {
            output_bytes = Some(0);
        }
        n.stop_confirmed = stopped;
        n.actual_runtime_ms = if stopped { Some(elapsed_ms) } else { None };
        n.actual_output_bytes = if stopped { output_bytes } else { None };
        n.state = if !stopped {
            "outcome_unknown"
        } else if session.turn_state.as_deref() == Some("completed") {
            "candidate"
        } else if r.cancel_requested {
            "cancelled"
        } else if never_started {
            "failed"
        } else {
            "outcome_unknown"
        }
        .into();
        r.state = match n.state.as_str() {
            "candidate" if was_blocked => "blocked",
            "candidate" => "awaiting_acceptance",
            "cancelled" => "cancelled",
            "failed" => "failed",
            _ => "outcome_unknown",
        }
        .into();
        if r.cancel_requested && stopped {
            r.state = "cancelled".into();
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let raw:Option<String>=tx.query_row("SELECT document FROM goal_run_reservations WHERE attempt_id=?1 AND octet_length(document)<=262144",[&session.attempt_id],|row|row.get(0)).optional().map_err(db_error)?;
        let mut reservation: ReservationRecord = decode(&raw.ok_or_else(changed)?)?;
        if reservation.settled {
            return Ok(());
        }
        reservation.settled = stopped;
        reservation.actual_runtime_ms = n.actual_runtime_ms;
        reservation.actual_output_bytes = n.actual_output_bytes;
        tx.execute(
            "UPDATE goal_run_reservations SET document=?1 WHERE attempt_id=?2",
            params![encode(&reservation)?, session.attempt_id],
        )
        .map_err(db_error)?;
        for (key_id, limits) in [
            (
                format!("project:{}", r.project_id),
                &record.selection.project_budget,
            ),
            (
                format!("root-grant:{}", r.root_grant_id),
                &record.selection.root_budget,
            ),
        ] {
            let raw:String=tx.query_row("SELECT document FROM goal_ledgers WHERE scope=?1 AND octet_length(document)<=1048576",[&key_id],|row|row.get(0)).map_err(db_error)?;
            let mut m: MeterLedger = decode(&raw)?;
            m.runtime_ms = m
                .runtime_ms
                .and_then(|a| n.actual_runtime_ms.and_then(|b| a.checked_add(b)));
            m.output_bytes = m
                .output_bytes
                .and_then(|a| n.actual_output_bytes.and_then(|b| a.checked_add(b)));
            m.tokens = None;
            m.cost_microunits = None;
            if (m.runtime_ms.is_none_or(|v| v > limits.runtime_ms)
                || m.output_bytes.is_none_or(|v| v > limits.output_bytes))
                && r.state == "awaiting_acceptance"
            {
                r.state = "blocked".into();
            }
            tx.execute(
                "UPDATE goal_ledgers SET document=?1 WHERE scope=?2",
                params![encode(&m)?, key_id],
            )
            .map_err(db_error)?;
        }
        let overrun = n
            .actual_runtime_ms
            .is_some_and(|v| v > reservation.reservation.runtime_ms)
            || n.actual_output_bytes
                .is_some_and(|v| v > reservation.reservation.output_bytes)
            || n.actual_runtime_ms
                .is_some_and(|v| v > n.node.budget.runtime_ms)
            || n.actual_output_bytes
                .is_some_and(|v| v > n.node.budget.output_bytes);
        if overrun && r.state == "awaiting_acceptance" {
            r.state = "blocked".into();
        }
        if r.state == "awaiting_acceptance"
            && can_continue
            && !record.revoked
            && crate::policy::checked_lineage(&tx, &r.root_grant_id).is_ok()
            && prior_nodes.iter().all(|old| {
                old.node.id == n.node.id
                    || old.state == "not_started"
                    || old.state == "candidate" && old.stop_confirmed
            })
            && prior_nodes.iter().any(executable_work)
            && adopted_plan_is_current_head(&tx, &r)?
        {
            // Queued is readiness, never admission. The next daemon advance must
            // re-observe inputs/authority and reserve all budgets atomically.
            r.state = "queued".into();
        }
        save_node(&tx, &r.id, &n)?;
        let payload = json!({"budget_overrun":overrun,"node_id":n.node.id,"attempt_id":session.attempt_id,"session_id":id,"stop_confirmed":stopped,"state":n.state,"reservation":reservation.reservation,"observed_runtime_lower_bound_ms":elapsed_ms,"actual_runtime_ms":n.actual_runtime_ms,"actual_output_bytes":n.actual_output_bytes,"tokens":null,"cost_microunits":null,"task_accepted":false});
        save_run(&tx, &mut r, "goal.node_settled", payload)?;
        tx.commit().map_err(db_error)
    }
    pub(crate) fn recover_goal_runs(&mut self) -> Result<()> {
        raw_collection_budget(&self.conn,"SELECT count(*),COALESCE(sum(octet_length(id)),0),0 FROM goal_runs WHERE state IN('queued','running','cancel_requested')",[],256,0)?;
        let ids = {
            let mut q=self.conn.prepare("SELECT id FROM goal_runs WHERE state IN('queued','running','cancel_requested')").map_err(db_error)?;
            let values = q
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(db_error)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(db_error)?;
            values
        };
        for id in ids {
            let tx = self
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(db_error)?;
            let mut r = run(&tx, &id)?;
            if graph::is_graph_run(&tx, &r.id)? {
                graph::checked_capability(&tx, &r)?;
            } else {
                factory::verify_native_run(&tx, &r, false)?;
            }

            r.state = "outcome_unknown".into();
            let nodes = node_records(&tx, &id)?;
            if nodes
                .iter()
                .any(|n| n.session_id.is_some() && !n.stop_confirmed)
            {
                for key in [
                    format!("project:{}", r.project_id),
                    format!("root-grant:{}", r.root_grant_id),
                ] {
                    let document:Option<Option<String>>=tx.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM goal_ledgers WHERE scope=?1",[&key],|row|row.get(0)).optional().map_err(db_error)?;
                    if let Some(document) = document {
                        let mut meter: MeterLedger = decode(&document.ok_or_else(limited)?)?;
                        meter.runtime_ms = None;
                        meter.output_bytes = None;
                        meter.tokens = None;
                        meter.cost_microunits = None;
                        tx.execute(
                            "UPDATE goal_ledgers SET document=?1 WHERE scope=?2",
                            params![encode(&meter)?, key],
                        )
                        .map_err(db_error)?;
                    }
                }
            }
            for mut n in nodes {
                if matches!(n.state.as_str(), "reserved" | "running" | "stopping") {
                    n.state = "outcome_unknown".into();
                    save_node(&tx, &id, &n)?;
                }
            }
            save_run(
                &tx,
                &mut r,
                "goal.run_interrupted",
                json!({"automatic_replay":false,"trusted_stop_receipt":false,"reservation_refunded":false,"task_accepted":false}),
            )?;
            tx.commit().map_err(db_error)?;
        }
        Ok(())
    }
}

pub(crate) fn ordinary_session_budget_gate(db: &Connection, project_id: &str) -> Result<()> {
    let governed: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM goal_runs WHERE project_id=?1)",
            [project_id],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if governed {
        return Err(err("AUTH_REQUIRED","This project has a shared native-approved Goal budget. An unbounded ordinary session cannot bypass its multidimensional reservation; use a bounded Goal node."));
    }
    Ok(())
}

impl Store {
    pub(crate) fn block_goal_run(&mut self, id: &str, code: &str) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let mut r = run(&tx, id)?;
        if matches!(r.state.as_str(), "queued" | "running") {
            r.state = "blocked".into();
            save_run(
                &tx,
                &mut r,
                "goal.run_blocked",
                json!({"reason_code":code,"automatic_retry":false,"task_accepted":false}),
            )?;
        }
        tx.commit().map_err(db_error)
    }
}

impl Store {
    pub(crate) fn goal_execution_proposal_replay(
        &self,
        s: &GoalRunSelection,
        key_id: &str,
    ) -> Result<Option<ApprovalProposal>> {
        key(key_id)?;
        let Some((_, raw)) =
            idempotency_entry(&self.conn, "local:approvals.propose:goal_execution", key_id)?
        else {
            return Ok(None);
        };
        let p: ApprovalProposal = decode(&raw)?;
        if p.kind != "goal_execution" || p.snapshot["intent"] != json!(s) {
            return Err(err(
                "CONFIG_CONFLICT",
                "Goal execution key belongs to another exact intent",
            ));
        }
        Ok(Some(p))
    }
    pub(crate) fn goal_run_needs_advance(&self, id: &str) -> Result<bool> {
        let r = run(&self.conn, id)?;
        if r.cancel_requested || !matches!(r.state.as_str(), "queued" | "running") {
            return Ok(false);
        }
        if !adopted_plan_is_current_head(&self.conn, &r)? {
            return Ok(false);
        }
        let nodes = node_records(&self.conn, id)?;
        for n in &nodes {
            if n.state == "reserved" {
                let Some(id) = n.session_id.as_deref() else {
                    return Ok(false);
                };
                let session = self.get_session(id)?;
                return Ok(session.state == "ready" && session.turn_state.is_none());
            }
            if matches!(n.state.as_str(), "running" | "stopping" | "outcome_unknown") {
                return Ok(false);
            }
        }
        let ready = if graph::is_graph_run(&self.conn, id)? {
            graph::ready_dispatch_node(&self.conn, &r, &nodes)?.is_some()
        } else {
            nodes.iter().any(executable_work)
        };
        Ok(ready && !local_goal_slot_is_occupied(&self.conn)?)
    }
    /// A durable exact-request intent is not a dispatch receipt. It can always
    /// be recovered without repeating an execution boundary; get shows effects.
    pub(crate) fn admit_goal_advance(
        &mut self,
        p: &GoalRunDispatch,
        revision: u64,
        key_id: &str,
    ) -> Result<(Value, bool)> {
        self.admit_goal_advance_kind(p, revision, key_id, false)
    }
    pub(crate) fn admit_goal_graph_advance(
        &mut self,
        p: &GoalRunDispatch,
        revision: u64,
        key_id: &str,
    ) -> Result<(Value, bool)> {
        self.admit_goal_advance_kind(p, revision, key_id, true)
    }
    fn admit_goal_advance_kind(
        &mut self,
        p: &GoalRunDispatch,
        revision: u64,
        key_id: &str,
        graph: bool,
    ) -> Result<(Value, bool)> {
        key(key_id)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if graph {
            graph::checked_capability(&tx, &run(&tx, &p.run_id)?)?;
        } else {
            graph::require_legacy_run(&tx, &p.run_id)?;
        }
        let scope = format!("goal-run:{}:advance", p.run_id);
        let payload = encode(&json!({"intent":p,"expected_revision":revision}))?;
        if let Some(v) = replay(&tx, &scope, key_id, &payload)? {
            return Ok((v, false));
        }
        let r = run(&tx, &p.run_id)?;
        if r.revision != revision || sequence(&tx)? != p.expected_ledger_revision {
            return Err(err(
                "STALE_REVISION",
                "Goal advancement no longer matches its exact run/ledger observation",
            ));
        }
        if r.cancel_requested || !matches!(r.state.as_str(), "queued" | "running") {
            return Err(denied());
        }
        checked_runtime_scope(&tx, &r, self.data_input.as_ref())?;
        let receipt = json!({"run_id":r.id,"intent_id":Uuid::new_v4().to_string(),"requested_run_revision":revision,"requested_ledger_revision":p.expected_ledger_revision,"observation":"advance_intent_recorded","execution_state_requires_get":true,"task_accepted":false});
        remember(&tx, &scope, key_id, &payload, &receipt)?;
        event(
            &tx,
            &r.id,
            r.revision,
            "goal.advance_intent_recorded",
            json!({"receipt":receipt,"idempotency_key":key_id,"provider_started":false}),
        )?;
        tx.commit().map_err(db_error)?;
        Ok((receipt, true))
    }
    pub(crate) fn goal_advance_replay(
        &self,
        p: &GoalRunDispatch,
        revision: u64,
        key_id: &str,
    ) -> Result<Option<Value>> {
        key(key_id)?;
        graph::require_legacy_run(&self.conn, &p.run_id)?;
        let payload = encode(&json!({"intent":p,"expected_revision":revision}))?;
        replay(
            &self.conn,
            &format!("goal-run:{}:advance", p.run_id),
            key_id,
            &payload,
        )
    }
}

fn verify_plan_advances(
    db: &Connection,
    r: &GoalRun,
    p: &ApprovalProposal,
    approved: &Value,
    authorized_sequence: i64,
) -> Result<()> {
    let mut revision = approved["plan_revision"].as_u64().ok_or_else(changed)?;
    let mut digest = approved["plan_digest"]
        .as_str()
        .ok_or_else(changed)?
        .to_string();
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(document)),0),COALESCE(max(octet_length(document)),0) FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.run_plan_advanced'",[&r.id],256,MAX_STORED_BYTES)?;
    let mut q=db.prepare("SELECT sequence,CASE WHEN octet_length(document)<=1048576 THEN document END FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.run_plan_advanced' ORDER BY sequence LIMIT 257").map_err(db_error)?;
    let events = q
        .query_map([&r.id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    if events.len() > 256 {
        return Err(limited());
    }
    let history = bounded_plan_history(db, &r.scope_id)?;
    let mut previous = authorized_sequence;
    for (seq, raw) in events {
        let e: Event = decode(&raw.ok_or_else(limited)?)?;
        let next = e.payload["plan_revision"].as_u64().ok_or_else(changed)?;
        if p.snapshot["allow_in_scope_plan_revisions"] != true
            || e.source != "goal_core"
            || e.payload["scope_id"] != r.scope_id
            || e.payload["previous_plan_revision"] != revision
            || e.payload["previous_plan_digest"] != digest
            || seq <= previous
            || next <= revision
        {
            return Err(changed());
        }
        let plan = history
            .iter()
            .find(|plan| {
                plan.proposal.revision == next && json!(plan.digest) == e.payload["plan_digest"]
            })
            .ok_or_else(changed)?;
        revision = next;
        digest = plan.digest.clone();
        previous = seq;
    }
    if revision != r.plan_revision || digest != r.plan_digest {
        return Err(changed());
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanAdvance {
    pub run_id: String,
    pub expected_plan_revision: u64,
    pub expected_ledger_revision: u64,
}
pub(super) fn runtime_scope_for_revision(
    db: &Connection,
    scope_id: &str,
    data: Option<&crate::data_workspaces::DataInputWitness>,
) -> Result<Option<(ScopeRecord, ApprovedGoalScope)>> {
    let id:Option<String>=db.query_row("SELECT id FROM goal_runs WHERE scope_id=?1 AND state IN('queued','running','awaiting_acceptance','blocked') ORDER BY rowid DESC LIMIT 1",[scope_id],|row|row.get(0)).optional().map_err(db_error)?;
    let Some(id) = id else { return Ok(None) };
    let r = run(db, &id)?;
    let old = node_records(db, &id)?;
    if old.iter().any(|n| {
        n.session_id.is_some() && !n.stop_confirmed
            || matches!(
                n.state.as_str(),
                "reserved" | "running" | "stopping" | "outcome_unknown"
            )
    }) {
        return Err(err(
            "WORKSPACE_BUSY",
            "Stop and reconcile exact Goal attempts before proposing a replacement plan",
        ));
    }
    let out = checked_runtime_scope_at(db, &r, data, false)?;
    no_writers(
        db,
        &raw::<Task>(db, "tasks", &r.task_id)?,
        &out.0.refs.grant_after,
    )?;
    Ok(Some(out))
}
impl Store {
    pub fn advance_goal_plan(
        &mut self,
        p: &GoalPlanAdvance,
        revision: u64,
        key_id: &str,
    ) -> Result<GoalRun> {
        self.advance_goal_plan_kind(p, revision, key_id, false)
    }
    fn advance_goal_plan_kind(
        &mut self,
        p: &GoalPlanAdvance,
        revision: u64,
        key_id: &str,
        graph: bool,
    ) -> Result<GoalRun> {
        key(key_id)?;
        if factory::marked_run(&self.conn, &p.run_id)? {
            return Err(denied());
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if graph {
            graph::checked_capability(&tx, &run(&tx, &p.run_id)?)?;
        } else {
            graph::require_legacy_run(&tx, &p.run_id)?;
        }
        let idem = format!("goal-run:{}:advance-plan", p.run_id);
        let payload = encode(&json!({"intent":p,"expected_revision":revision}))?;
        if let Some(r) = replay(&tx, &idem, key_id, &payload)? {
            return Ok(r);
        }
        let mut r = run(&tx, &p.run_id)?;
        if r.revision != revision || sequence(&tx)? != p.expected_ledger_revision {
            return Err(err(
                "STALE_REVISION",
                "Plan advancement requires the exact current run and shared ledger",
            ));
        }
        if r.cancel_requested
            || matches!(
                r.state.as_str(),
                "accepted" | "outcome_unknown" | "cancelled" | "failed"
            )
        {
            return Err(denied());
        }
        refuse_retained_overrun(&tx, &r.root_grant_id, &r.project_id)?;
        let (_, scope) = runtime_scope_for_revision(&tx, &r.scope_id, self.data_input.as_ref())?
            .ok_or_else(denied)?;
        let (_, plan) = current_plan(&tx, &r.scope_id, &scope)?;
        if plan.proposal.revision != p.expected_plan_revision
            || plan.proposal.revision <= r.plan_revision
        {
            return Err(err(
                "STALE_REVISION",
                "The proposed plan revision is not a newer validated version",
            ));
        }
        let is_rework = node_records(&tx, &r.id)?
            .iter()
            .any(|n| n.attempt_id.is_some());
        if is_rework {
            let record: ScopeRecord = raw(&tx, "goal_scopes", &r.scope_id)?;
            for (project, id, max) in [
                (
                    true,
                    r.project_id.as_str(),
                    record.selection.project_budget.reworks,
                ),
                (
                    false,
                    r.root_grant_id.as_str(),
                    record.selection.root_budget.reworks,
                ),
            ] {
                let used = factory::shared_reworks(&tx, project, id)?;
                if used >= max {
                    return Err(err(
                        "RESOURCE_LIMIT",
                        "The cumulative shared rework budget is exhausted",
                    ));
                }
            }
        }
        let previous_digest = r.plan_digest.clone();
        let previous_revision = r.plan_revision;
        for (ordinal, node) in plan.proposal.nodes.iter().enumerate() {
            let n = RuntimeNode {
                node: node.clone(),
                plan_revision: plan.proposal.revision,
                state: "not_started".into(),
                session_id: None,
                attempt_id: None,
                started_at_ms: None,
                stop_confirmed: false,
                actual_runtime_ms: None,
                actual_output_bytes: None,
                tokens: None,
                cost_microunits: None,
            };
            tx.execute(
                "INSERT INTO goal_run_nodes VALUES(?1,?2,?3,?4,'not_started',NULL,NULL,?5)",
                params![
                    r.id,
                    sql_num(n.plan_revision)?,
                    node.id,
                    ordinal as i64,
                    encode(&n)?
                ],
            )
            .map_err(db_error)?;
        }
        r.plan_revision = plan.proposal.revision;
        r.plan_digest = plan.digest;
        r.state = "queued".into();
        let payload_event = json!({"scope_id":r.scope_id,"previous_plan_revision":previous_revision,"previous_plan_digest":previous_digest,"plan_revision":r.plan_revision,"plan_digest":r.plan_digest,"cumulative_consumption_reset":false,"new_permissions":false,"rework":is_rework});
        save_run(&tx, &mut r, "goal.run_plan_advanced", payload_event)?;
        remember(&tx, &idem, key_id, &payload, &r)?;
        tx.commit().map_err(db_error)?;
        Ok(r)
    }
    fn goal_node_history(&self, r: &GoalRun) -> Result<Vec<RuntimeNode>> {
        raw_collection_budget(&self.conn,"SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(run_id)+octet_length(node_id)+octet_length(state)+COALESCE(octet_length(session_id),0)+COALESCE(octet_length(attempt_id),0)),0),COALESCE(max(octet_length(document)),0) FROM goal_run_nodes WHERE run_id=?1",[&r.id],65536,MAX_GOAL_PLAN_BYTES as i64)?;

        let mut q=self.conn.prepare("SELECT DISTINCT plan_revision FROM goal_run_nodes WHERE run_id=?1 ORDER BY plan_revision LIMIT 257").map_err(db_error)?;
        let revisions = q
            .query_map([&r.id], |row| row.get::<_, i64>(0))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        if revisions.len() > 256 {
            return Err(limited());
        }
        let plans = bounded_plan_history(&self.conn, &r.scope_id)?;
        let mut nodes = Vec::new();
        let mut bytes = 0usize;
        for rev in revisions {
            let rev = u64::try_from(rev).map_err(|_| changed())?;
            let plan = plans
                .iter()
                .find(|p| p.proposal.revision == rev)
                .ok_or_else(changed)?;
            let current = node_records_at(&self.conn, r, rev, &plan.digest, plan)?;
            bytes = bytes
                .checked_add(encode(&current)?.len())
                .ok_or_else(limited)?;
            if bytes > 4_194_304 {
                return Err(limited());
            }
            nodes.extend(current);
        }
        Ok(nodes)
    }
}

// Read only from the approved directory inode. A separate path check followed
// by read_artifact(root, ...) could otherwise open a replacement root in between.
fn read_pinned_output(
    pin: &agentbaton_workspaces::WorkspaceIdentity,
    relative: &std::path::Path,
    limit: u64,
) -> Result<u64> {
    use rustix::fs::{open, openat, Mode, OFlags};
    use std::{fs::File, io::Read, os::unix::fs::MetadataExt, path::Component};
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let mut root: File = open("/", flags | OFlags::DIRECTORY, Mode::empty())
        .map_err(|_| changed())?
        .into();
    for part in pin.canonical_root.components() {
        match part {
            Component::RootDir => {}
            Component::Normal(name) => {
                root = openat(&root, name, flags | OFlags::DIRECTORY, Mode::empty())
                    .map_err(|_| changed())?
                    .into()
            }
            _ => return Err(changed()),
        }
    }
    let meta = root.metadata().map_err(|_| changed())?;
    if !pin.canonical_root.is_absolute()
        || !meta.is_dir()
        || (meta.dev(), meta.ino()) != (pin.device, pin.inode)
    {
        return Err(changed());
    }
    fn leaf(root: &File, relative: &std::path::Path) -> Result<File> {
        let parts: Vec<_> = relative.components().collect();
        if parts.is_empty() {
            return Err(changed());
        }
        let mut file = root.try_clone().map_err(|_| changed())?;
        for (i, part) in parts.iter().enumerate() {
            let Component::Normal(name) = part else {
                return Err(changed());
            };
            let mut flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
            if i + 1 < parts.len() {
                flags |= OFlags::DIRECTORY;
            }
            file = openat(&file, *name, flags, Mode::empty())
                .map_err(|_| changed())?
                .into();
        }
        Ok(file)
    }
    fn identity(m: &std::fs::Metadata) -> (u64, u64, u64, u64, u32, u32, u32, i64, i64, i64, i64) {
        (
            m.dev(),
            m.ino(),
            m.len(),
            m.nlink(),
            m.mode(),
            m.uid(),
            m.gid(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    }
    let file = leaf(&root, relative)?;
    let before = file.metadata().map_err(|_| changed())?;
    if !before.is_file() || before.nlink() != 1 {
        return Err(changed());
    }
    if before.len() > limit {
        return Err(limited());
    }
    let mut bytes = Vec::new();
    (&file)
        .take(limit.checked_add(1).ok_or_else(limited)?)
        .read_to_end(&mut bytes)
        .map_err(|_| changed())?;
    if bytes.len() as u64 > limit {
        return Err(limited());
    }
    let held = file.metadata().map_err(|_| changed())?;
    let current = leaf(&root, relative)?.metadata().map_err(|_| changed())?;
    if identity(&before) != identity(&held)
        || identity(&before) != identity(&current)
        || bytes.len() as u64 != held.len()
    {
        return Err(changed());
    }
    Ok(bytes.len() as u64)
}
fn current_output_root(db: &Connection, workspace: &ManagedWorkspace) -> bool {
    let Some(pin) = workspace.identity.as_ref() else {
        return false;
    };
    match pin.kind {
        agentbaton_workspaces::WorkspaceKind::Directory => {
            crate::data_workspaces::data_live_workspace(db, workspace).is_ok()
        }
        agentbaton_workspaces::WorkspaceKind::Git { .. } => {
            crate::policy::live_workspace_matches(&workspace.root, pin).unwrap_or(false)
        }
    }
}

fn trusted_session_stop(
    db: &Connection,
    run_id: &str,
    s: &crate::sessions::ManagedSession,
) -> Result<i64> {
    let (state,executor,raw):(String,Option<String>,String)=db.query_row("SELECT state,executor_instance,document FROM dispatch_intents WHERE id=?1 AND octet_length(document)<=1048576",[&s.intent_id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(db_error)?;
    let intent: crate::execution::DispatchIntent = decode(&raw)?;
    if state != "stopped"
        || intent.state != crate::execution::IntentState::Stopped
        || intent.id != s.intent_id
        || intent.attempt_id != s.attempt_id
        || intent.task_id != s.task_id
        || intent.executor_instance != executor
        || s.state != "stopped"
    {
        return Err(changed());
    }
    let mut q=db.prepare("SELECT sequence,CASE WHEN octet_length(document)<=1048576 THEN document END FROM events WHERE json_extract(document,'$.kind')='attempt.stopped' AND json_extract(document,'$.source')='user_action' AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.payload.intent_id')=?2 ORDER BY sequence LIMIT 2").map_err(db_error)?;
    let events = q
        .query_map(params![s.task_id, s.intent_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    if events.len() != 1 {
        return Err(denied());
    }
    let (stop_sequence, raw) = &events[0];
    let event: Event = decode(raw.as_deref().ok_or_else(limited)?)?;
    let evidence = &event.payload["stop_evidence"];
    let valid = match evidence["kind"].as_str() {
        Some("never_dispatched") => executor.is_none(),
        Some("executor_not_started") => {
            executor.as_deref() == Some(&s.id)
                && evidence["executor_instance"] == s.id
                && evidence["receipt_ref"] == format!("owned-worker-no-start:{}", s.id)
        }
        Some("executor_stop") => {
            executor.as_deref() == Some(&s.id)
                && evidence["executor_instance"] == s.id
                && evidence["receipt_ref"] == format!("owned-worker-stop:{}", s.id)
        }
        _ => false,
    };
    if !valid || *stop_sequence <= 0 {
        return Err(denied());
    }
    let reserved:Option<i64>=db.query_row("SELECT sequence FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.kind')='goal.node_reserved' AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.payload.attempt_id')=?2 AND json_extract(document,'$.payload.session_id')=?3 ORDER BY sequence LIMIT 1",params![run_id,s.attempt_id,s.id],|row|row.get(0)).optional().map_err(db_error)?;
    if reserved.is_none_or(|n| n <= 0 || n >= *stop_sequence) {
        return Err(denied());
    }
    let supervisor:Option<i64>=db.query_row("SELECT sequence FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.kind')='session.stopped' AND json_extract(document,'$.source')='baton_supervisor' AND json_extract(document,'$.aggregate_id')=?1 AND (json_extract(document,'$.payload.process_reaped')=1 OR json_extract(document,'$.payload.no_process_started')=1) ORDER BY sequence LIMIT 1",[&s.id],|row|row.get(0)).optional().map_err(db_error)?;
    let sequence = supervisor.ok_or_else(denied)?;
    if sequence <= *stop_sequence {
        return Err(denied());
    }
    Ok(sequence)
}

fn refuse_retained_overrun(db: &Connection, root: &str, project: &str) -> Result<()> {
    if reservations(db, root, project)?.iter().any(|r| {
        r.actual_runtime_ms
            .is_some_and(|n| n > r.reservation.runtime_ms)
            || r.actual_output_bytes
                .is_some_and(|n| n > r.reservation.output_bytes)
    }) {
        return Err(err("RESOURCE_LIMIT","Retained actual execution exceeded a declared per-attempt reservation; reconcile the budget before any new dispatch"));
    }
    Ok(())
}
impl Store {
    pub(crate) fn goal_plan_advance_replay(
        &self,
        p: &GoalPlanAdvance,
        revision: u64,
        key_id: &str,
    ) -> Result<Option<GoalRun>> {
        key(key_id)?;
        graph::require_legacy_run(&self.conn, &p.run_id)?;
        let payload = encode(&json!({"intent":p,"expected_revision":revision}))?;
        replay(
            &self.conn,
            &format!("goal-run:{}:advance-plan", p.run_id),
            key_id,
            &payload,
        )
    }
}

fn raw_collection_budget(
    db: &Connection,
    sql: &str,
    arguments: impl rusqlite::Params,
    max_count: i64,
    max_document: i64,
) -> Result<()> {
    let (count, bytes, largest): (i64, i64, i64) = db
        .query_row(sql, arguments, |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(db_error)?;
    if count < 0
        || count > max_count
        || !(0..=4_194_304).contains(&bytes)
        || largest < 0
        || largest > max_document
    {
        return Err(limited());
    }
    Ok(())
}
fn bounded_plan_history(db: &Connection, scope_id: &str) -> Result<Vec<GoalPlanView>> {
    key(scope_id)?;
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)+octet_length(task_id)+octet_length(root_grant_id)),0),COALESCE(max(octet_length(document)),0) FROM goal_scopes WHERE id=?1",[scope_id],1,MAX_STORED_BYTES)?;
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(document)+COALESCE(octet_length(result),0)+octet_length(id)),0),COALESCE(max(max(octet_length(document),COALESCE(octet_length(result),0))),0) FROM approval_proposals WHERE id=?1",[scope_id],1,MAX_STORED_BYTES)?;
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(bytes),0),COALESCE(max(doc_bytes),0) FROM (SELECT octet_length(document) AS doc_bytes,octet_length(document)+octet_length(scope_id)+octet_length(plan_id)+octet_length(digest)+COALESCE(octet_length(previous_digest),0)+octet_length(created_at) AS bytes FROM goal_plan_revisions WHERE scope_id=?1 UNION ALL SELECT octet_length(document),octet_length(document)+octet_length(id)+octet_length(scope_id)+octet_length(plan_id)+octet_length(digest) FROM goal_plan_proposals WHERE scope_id=?1)",[scope_id],512,MAX_STORED_BYTES)?;
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(scope_id)+octet_length(plan_id)+octet_length(digest)),0),0 FROM goal_plan_heads WHERE scope_id=?1",[scope_id],1,0)?;
    super::historical_plan_views(db, scope_id)
}

impl Store {
    /// Existing shared Goal meters account both Factory and bounded ordinary
    /// sessions. The caller's private reservation context is checked separately.
    pub(crate) fn reserve_shared_execution_meter(
        db: &Connection,
        record: &crate::factory::FactoryReservation,
        root_limits: &BudgetLimits,
        project_limits: &BudgetLimits,
    ) -> Result<()> {
        let task: Task = raw(db, "tasks", &record.root_task_id)?;
        let grant = crate::policy::checked_lineage(db, &record.root_grant_id)?;
        let (pc, _, rc, _) = counts(db, &task, &grant)?;
        let pc = pc.checked_sub(1).ok_or_else(changed)?;
        let rc = rc.checked_sub(1).ok_or_else(changed)?;
        let records = reservations(db, &record.root_grant_id, &record.project_id)?;
        let active = records.iter().filter(|x| !x.settled).collect::<Vec<_>>();
        let mut root_active = Vec::new();
        for old in &active {
            let same = if let Some(root) = &old.shared_root_grant_id {
                root == &record.root_grant_id
            } else {
                db.query_row(
                    "SELECT json_extract(document,'$.root_grant_id')=?1 FROM goal_runs WHERE id=?2",
                    params![record.root_grant_id, old.run_id],
                    |r| r.get::<_, bool>(0),
                )
                .map_err(db_error)?
            };
            if same {
                root_active.push(*old);
            }
        }
        for (key, limits, before, active) in [
            (
                format!("project:{}", record.project_id),
                project_limits,
                pc,
                &active,
            ),
            (
                format!("root-grant:{}", record.root_grant_id),
                root_limits,
                rc,
                &root_active,
            ),
        ] {
            remember_meter(db, &key, limits, before)?;
            reserve_meter(db, &key, limits, before, &record.reservation, active)?;
        }
        Ok(())
    }
    pub(crate) fn settle_shared_execution_meter(
        db: &Connection,
        record: &crate::factory::FactoryReservation,
    ) -> Result<()> {
        for key in [
            format!("project:{}", record.project_id),
            format!("root-grant:{}", record.root_grant_id),
        ] {
            let raw:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM goal_ledgers WHERE scope=?1",[&key],|r|r.get(0)).optional().map_err(db_error)?;
            let mut meter: MeterLedger = decode(&raw.flatten().ok_or_else(changed)?)?;
            meter.runtime_ms = meter.runtime_ms.and_then(|before| {
                record
                    .actual_runtime_ms
                    .and_then(|elapsed| before.checked_add(elapsed))
            });
            meter.output_bytes = meter.output_bytes.and_then(|before| {
                record
                    .actual_output_bytes
                    .and_then(|bytes| before.checked_add(bytes))
            });
            meter.tokens = None;
            meter.cost_microunits = None;
            db.execute(
                "UPDATE goal_ledgers SET document=?1 WHERE scope=?2",
                params![encode(&meter)?, key],
            )
            .map_err(db_error)?;
        }
        Ok(())
    }
}

impl Store {
    pub(crate) fn remaining_shared_runtime(
        db: &Connection,
        root: &str,
        project: &str,
    ) -> Result<Option<u64>> {
        Self::remaining_shared_runtime_with_limits(db, root, project, None)
    }
    /// Current request limits can only narrow the original persisted ceilings.
    /// The optional pair is (root, project), derived from an approved internal
    /// scope; it is not wire authority and never changes stored history/limits.
    pub(crate) fn remaining_shared_runtime_with_limits(
        db: &Connection,
        root: &str,
        project: &str,
        current_limits: Option<(u64, u64)>,
    ) -> Result<Option<u64>> {
        let all = reservations(db, root, project)?;
        let mut remaining = None::<u64>;
        for (key, is_root) in [
            (format!("project:{project}"), false),
            (format!("root-grant:{root}"), true),
        ] {
            let row:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM goal_ledgers WHERE scope=?1",[key],|r|r.get(0)).optional().map_err(db_error)?;
            let Some(row) = row else { continue };
            let ledger: MeterLedger = decode(&row.ok_or_else(limited)?)?;
            let mut spent=ledger.runtime_ms.ok_or_else(||err("BUDGET_UNKNOWN","The original shared runtime observation is unknown; do not reset it with a new session/root"))?;
            for old in all.iter().filter(|r| !r.settled) {
                let same = if !is_root {
                    true
                } else if let Some(origin) = &old.shared_root_grant_id {
                    origin == root
                } else {
                    db.query_row("SELECT json_extract(document,'$.root_grant_id')=?1 FROM goal_runs WHERE id=?2",params![root,old.run_id],|r|r.get::<_,bool>(0)).map_err(db_error)?
                };
                if same {
                    spent = spent
                        .checked_add(old.reservation.runtime_ms)
                        .ok_or_else(limited)?;
                }
            }
            let cap = current_limits.map_or(ledger.limits.runtime_ms, |(root, project)| {
                ledger
                    .limits
                    .runtime_ms
                    .min(if is_root { root } else { project })
            });
            let left = cap.checked_sub(spent).ok_or_else(limited)?;
            remaining = Some(remaining.map_or(left, |n| n.min(left)));
        }
        Ok(remaining)
    }
}

impl Store {
    /// Recording owned queue/preflight work is cleanup/accounting, not permission
    /// to start a Provider. Existing caps and unknown history are never reset.
    pub(crate) fn charge_factory_preflight(
        db: &Connection,
        scope: &crate::factory::FactoryScope,
        elapsed: u64,
    ) -> Result<()> {
        let (pc, _, rc, _) = counts(
            db,
            &scope.bindings.root_task,
            &scope.bindings.approved_grant,
        )?;
        for (key, limits, before) in [
            (
                format!("project:{}", scope.project_id),
                &scope.selection.project_budget,
                pc,
            ),
            (
                format!("root-grant:{}", scope.root_grant_id),
                &scope.selection.root_budget,
                rc,
            ),
        ] {
            remember_meter(db, &key, limits, before)?;
            let raw: String = db
                .query_row(
                    "SELECT document FROM goal_ledgers WHERE scope=?1",
                    [&key],
                    |r| r.get(0),
                )
                .map_err(db_error)?;
            let mut ledger: MeterLedger = decode(&raw)?;
            ledger.runtime_ms = ledger.runtime_ms.and_then(|n| n.checked_add(elapsed));
            db.execute(
                "UPDATE goal_ledgers SET document=?1 WHERE scope=?2",
                params![encode(&ledger)?, key],
            )
            .map_err(db_error)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};
    struct Fixture {
        store: Store,
        _directory: tempfile::TempDir,
        scope: GoalScopeView,
        run: GoalRun,
    }
    fn git(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args([
                "-c",
                "user.name=Generated fixture",
                "-c",
                "user.email=fixture@invalid",
                "-c",
                "core.hooksPath=/dev/null",
                "-C",
            ])
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().into()
    }
    fn fixture(capacity: u32) -> Fixture {
        fixture_with_outputs(capacity, false)
    }
    fn fixture_with_outputs(capacity: u32, two_outputs: bool) -> Fixture {
        let dir = match std::env::var_os("AB_GOAL_CRASH_TEST_PARENT") {
            Some(parent) => tempfile::tempdir_in(parent).unwrap(),
            None => tempfile::tempdir().unwrap(),
        };
        let base = dir.path().canonicalize().unwrap();
        let source = base.join("source");
        fs::create_dir(&source).unwrap();
        git(&source, &["init", "--initial-branch=main"]);
        fs::write(source.join("file"), b"baseline").unwrap();
        git(&source, &["add", "file"]);
        git(&source, &["commit", "-m", "generated"]);
        let head = git(&source, &["rev-parse", "HEAD"]);
        let runtime = base.join("runtime");
        fs::create_dir(&runtime).unwrap();
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
        let mut store = Store::open(&runtime.join("baton.sqlite3")).unwrap();
        let project = store
            .import_project(&source, "Generated unit project", "project")
            .unwrap();
        let managed = runtime.join("workspaces");
        fs::create_dir(&managed).unwrap();
        fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
        let workspace = store
            .create_workspace(&project.id, project.revision, &head, &managed, "workspace")
            .unwrap();
        // Synthetic ready-version metadata only. No Provider is run by these
        // authority/raw-byte unit controls. Actual protocol tests are separate.
        let proposal = store
            .propose_connection(
                "Synthetic no-process unit connection",
                "custom_acp",
                Path::new("/usr/bin/true"),
                &[],
                "connection",
            )
            .unwrap();
        let connection = store.finalize_proposal(&proposal.id, 1).unwrap();
        let mut full = store
            .get_connection(connection["id"].as_str().unwrap())
            .unwrap();
        full.probe_state = "ready".into();
        store
            .conn
            .execute(
                "UPDATE connections SET document=?1 WHERE id=?2",
                params![encode(&full).unwrap(), full.id],
            )
            .unwrap();
        let grant = store
            .propose_grant(&project.id, &workspace.id, &full.id, 4, 1, 60_000, "grant")
            .unwrap();
        let grant: ExecutionGrant =
            serde_json::from_value(store.finalize_proposal(&grant.id, 1).unwrap()).unwrap();
        let task = store
            .create_registered(
                &project.id,
                "Produce an independently reviewed report",
                "task",
            )
            .unwrap();
        let limits = BudgetLimits {
            attempts: 4,
            parallel: 1,
            runtime_ms: 20_000,
            output_bytes: 16384,
            children: capacity - 1,
            reworks: 4,
            tokens: None,
            cost_microunits: None,
        };
        let mut declared_outputs = vec![GoalOutputSelection {
            artifact: ArtifactRequirement {
                artifact_id: "report".into(),
                relative_path: "reports/report.txt".into(),
                max_bytes: 1024,
                expected_sha256: None,
            },
            validators: vec![],
        }];
        if two_outputs {
            declared_outputs.push(GoalOutputSelection {
                artifact: ArtifactRequirement {
                    artifact_id: "intermediate".into(),
                    relative_path: "reports/intermediate.json".into(),
                    max_bytes: 1024,
                    expected_sha256: None,
                },
                validators: vec![],
            });
        }
        let selection = GoalScopeSelection {
            task_id: task.id.clone(),
            expected_task_revision: task.revision,
            grant_id: grant.id.clone(),
            expected_grant_revision: grant.revision,
            max_nodes: capacity,
            new_max_children: capacity - 1,
            new_max_depth: 1,
            root_budget: limits.clone(),
            project_budget: limits,
            inputs: vec![ArtifactRequirement {
                artifact_id: "input".into(),
                relative_path: "file".into(),
                max_bytes: 1024,
                expected_sha256: Some(agentbaton_workspaces::validation::sha256(b"baseline")),
            }],
            outputs: declared_outputs,
            writable_prefixes: vec!["reports".into()],
            owner_acceptance_criteria: "Owner independently reviews exact bound artifacts".into(),
        };
        let approved = store.propose_goal_scope(&selection, "scope").unwrap();
        let scope: GoalScopeView =
            serde_json::from_value(store.finalize_proposal(&approved.id, 1).unwrap()).unwrap();
        let mut node = PlanNode {
            id: "produce-report".into(),
            parent_id: None,
            kind: NodeKind::Work,
            goal: "Produce a report".into(),
            dependencies: vec![],
            workspace_id: workspace.id,
            executor: Executor::Connection {
                binding: Box::new(scope.binding.clone()),
                program_sha256: scope.program_sha256.clone(),
            },
            recipients: vec![],
            required_inputs: vec![RequiredInput {
                goal_task_id: task.id,
                artifact_id: "input".into(),
                contract_id: scope.inputs["input"].contract_id.clone(),
                source: InputSource::ApprovedExternal {
                    sha256: scope.inputs["input"].sha256.clone(),
                },
            }],
            outputs: vec![PlannedOutput {
                artifact_id: "report".into(),
                contract_id: scope.outputs["report"].clone(),
            }],
            required_validators: vec!["goal-owner-review".into()],
            side_effect: SideEffect::WorkspaceWrite,
            write_set: vec![WriteTarget::Workspace {
                path: "reports/report.txt".into(),
            }],
            budget: BudgetLimits {
                attempts: 4,
                parallel: 1,
                runtime_ms: 10000,
                output_bytes: 8192,
                children: 0,
                reworks: 4,
                tokens: None,
                cost_microunits: None,
            },
            reservation: AttemptReservation {
                runtime_ms: 4000,
                output_bytes: 2048,
                tokens: None,
                cost_microunits: None,
            },
            retry: RetryPolicy {
                max_attempts: 4,
                max_reworks: 4,
                retry_classes: vec![
                    FailureClass::BeforeDispatchTransient,
                    FailureClass::ValidationFailed,
                ],
            },
            idempotency_key: "produce-report".into(),
            external_action_id: None,
        };
        if two_outputs {
            node.outputs.push(PlannedOutput {
                artifact_id: "intermediate".into(),
                contract_id: scope.outputs["intermediate"].clone(),
            });
        }
        let plan = GoalPlanProposal {
            schema_version: 1,
            plan_id: "unit-runtime-plan".into(),
            goal: scope.goal.clone(),
            scope_revision: scope.authority_revision,
            revision: 1,
            expected_revision: 0,
            acceptance_rules: scope.required_acceptance_rules.clone(),
            nodes: vec![node],
        };
        store
            .submit_goal_plan(
                &GoalPlanSubmission {
                    scope_id: scope.id.clone(),
                    proposal: plan,
                },
                "plan",
                0,
            )
            .unwrap();
        let selection = GoalRunSelection {
            scope_id: scope.id.clone(),
            expected_plan_revision: 1,
            expected_ledger_revision: store.sequence().unwrap(),
        };
        let proposal = store.propose_goal_run(&selection, "execute").unwrap();
        let run: GoalRun =
            serde_json::from_value(store.finalize_proposal(&proposal.id, 1).unwrap()).unwrap();
        Fixture {
            store,
            _directory: dir,
            scope,
            run,
        }
    }
    fn reserve(f: &mut Fixture) -> crate::sessions::ManagedSession {
        let p = GoalRunDispatch {
            run_id: f.run.id.clone(),
            expected_ledger_revision: f.store.sequence().unwrap(),
        };
        let context = f
            .store
            .goal_dispatch_context(&p, f.store.get_goal_run(&f.run.id).unwrap().revision)
            .unwrap()
            .unwrap();
        f.store.create_goal_session(context).unwrap()
    }
    fn grow(f: &mut Fixture, count: usize, revision: u64) {
        let mut plan = f.store.get_goal_plan(&f.scope.id).unwrap().proposal;
        let work = plan.nodes[0].clone();
        plan.nodes = vec![work.clone()];
        for i in 1..count {
            let mut owner = work.clone();
            owner.id = format!("owner-{i}");
            owner.idempotency_key = format!("review-{i}");
            owner.kind = NodeKind::OwnerApproval;
            owner.executor = Executor::Owner {
                rule_id: "goal-owner-review".into(),
            };
            owner.side_effect = SideEffect::AdvisoryReadOnly;
            owner.write_set.clear();
            owner.outputs.clear();
            owner.dependencies = vec![work.id.clone()];
            owner.required_inputs = vec![RequiredInput {
                goal_task_id: f.scope.task_id.clone(),
                artifact_id: "report".into(),
                contract_id: f.scope.outputs["report"].clone(),
                source: InputSource::Node {
                    node_id: work.id.clone(),
                    candidate_for_validation: true,
                },
            }];
            plan.nodes.push(owner);
        }
        plan.revision = revision;
        plan.expected_revision = revision - 1;
        f.store
            .submit_goal_plan(
                &GoalPlanSubmission {
                    scope_id: f.scope.id.clone(),
                    proposal: plan,
                },
                &format!("plan-{revision}"),
                revision - 1,
            )
            .unwrap();
        let r = f.store.get_goal_run(&f.run.id).unwrap();
        let p = GoalPlanAdvance {
            run_id: r.id,
            expected_plan_revision: revision,
            expected_ledger_revision: f.store.sequence().unwrap(),
        };
        f.store
            .advance_goal_plan(&p, r.revision, &format!("advance-{revision}"))
            .unwrap();
    }
    #[test]
    fn three_hundred_persisted_nodes_near_four_mib_remain_complete_and_oversize_refuses_before_decode(
    ) {
        let mut f = fixture(150);
        grow(&mut f, 150, 2);
        grow(&mut f, 149, 3);
        let original = f.store.goal_run_details(&f.run.id).unwrap();
        assert_eq!(original["nodes"].as_array().unwrap().len(), 300);
        let size = |db: &Connection| {
            db.query_row("SELECT sum(octet_length(document)+octet_length(run_id)+octet_length(node_id)+octet_length(state)+COALESCE(octet_length(session_id),0)+COALESCE(octet_length(attempt_id),0)) FROM goal_run_nodes",[],|row|row.get::<_,i64>(0)).unwrap()
        };
        let before = size(&f.store.conn);
        let target = 4_194_304 - 4096;
        let padding = (target - before) / 300;
        assert!(padding > 0);
        f.store
            .conn
            .execute(
                "UPDATE goal_run_nodes SET document=document||printf('%*s',?1,'')",
                [padding],
            )
            .unwrap();
        let near = size(&f.store.conn);
        assert!(near <= target && near > 4_180_000);
        assert_eq!(f.store.goal_run_details(&f.run.id).unwrap(), original);
        f.store.conn.execute("UPDATE goal_run_nodes SET document=document||printf('%*s',8192,'') WHERE rowid=(SELECT min(rowid) FROM goal_run_nodes)",[]).unwrap();
        assert!(size(&f.store.conn) > 4_194_304);
        assert_eq!(
            f.store.goal_run_details(&f.run.id).unwrap_err().code,
            "RESOURCE_LIMIT"
        );
        assert_eq!(
            f.store
                .conn
                .query_row("SELECT count(*) FROM goal_run_nodes", [], |row| row
                    .get::<_, u32>(0))
                .unwrap(),
            300
        );
    }
    #[test]
    fn reservation_collection_has_total_raw_limit_not_only_per_row_caps() {
        let mut f = fixture(2);
        let session = reserve(&mut f);
        let original: String = f
            .store
            .conn
            .query_row("SELECT document FROM goal_run_reservations", [], |row| {
                row.get(0)
            })
            .unwrap();
        let binding = encode(&f.scope.binding).unwrap();
        f.store
            .conn
            .execute(
                "UPDATE goal_run_reservations SET document=document||printf('%*s',?1,'')",
                [220000 - original.len() as i64],
            )
            .unwrap();
        for i in 1..20 {
            let attempt = format!("synthetic-pressure-{i}");
            f.store.conn.execute("INSERT INTO attempts(id,task_id,root_task_id,binding,idempotency_key) VALUES(?1,?2,?2,?3,?1)",params![attempt,f.scope.task_id,binding]).unwrap();
            f.store.conn.execute("INSERT INTO goal_run_reservations SELECT ?1,run_id,node_id,parent_node_id,document FROM goal_run_reservations WHERE attempt_id=?2",params![attempt,session.attempt_id]).unwrap();
        }
        assert_eq!(
            f.store
                .conn
                .query_row(
                    "SELECT max(octet_length(document)) FROM goal_run_reservations",
                    [],
                    |row| row.get::<_, u32>(0)
                )
                .unwrap(),
            220000
        );
        assert_eq!(
            reservations(&f.store.conn, &f.run.root_grant_id, &f.run.project_id)
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT"
        );
    }
    #[test]
    fn selected_output_measurement_requires_original_root_and_stable_regular_nofollow_leaf() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("output-root");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("report"), b"actual").unwrap();
        let pin = agentbaton_workspaces::inspect(&root).unwrap();
        assert_eq!(read_pinned_output(&pin, Path::new("report"), 6).unwrap(), 6);
        assert_eq!(
            read_pinned_output(&pin, Path::new("report"), 5)
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT"
        );
        assert!(read_pinned_output(&pin, Path::new("../report"), 100).is_err());
        let old = dir.path().join("preserved-original");
        fs::rename(&root, &old).unwrap();
        fs::create_dir(&root).unwrap();
        fs::write(root.join("report"), b"foreign").unwrap();
        assert!(read_pinned_output(&pin, Path::new("report"), 100).is_err());
        fs::remove_dir_all(&root).unwrap();
        fs::rename(old, &root).unwrap();
        fs::rename(root.join("report"), root.join("original-report")).unwrap();
        std::os::unix::fs::symlink("original-report", root.join("report")).unwrap();
        assert!(read_pinned_output(&pin, Path::new("report"), 100).is_err());
        fs::remove_file(root.join("report")).unwrap();
        fs::hard_link(root.join("original-report"), root.join("report")).unwrap();
        assert!(read_pinned_output(&pin, Path::new("report"), 100).is_err());
        fs::remove_file(root.join("report")).unwrap();
        assert!(Command::new("/usr/bin/mkfifo")
            .args(["-m", "600"])
            .arg(root.join("report"))
            .status()
            .unwrap()
            .success());
        assert!(read_pinned_output(&pin, Path::new("report"), 100).is_err());
    }
    #[test]
    fn unknown_persisted_run_state_is_never_treated_as_an_inactive_finished_run() {
        let f = fixture(2);
        f.store.conn.execute("UPDATE goal_runs SET state='unexpected_tag',document=json_set(document,'$.state','unexpected_tag')",[]).unwrap();
        assert_eq!(
            f.store.get_goal_run(&f.run.id).unwrap_err().code,
            "INPUT_CHANGED"
        );
        assert!(f.store.goal_run_details(&f.run.id).is_err());
    }
    #[test]
    fn oversized_sql_metadata_is_bounded_before_node_comparison() {
        let f = fixture(2);
        f.store
            .conn
            .execute(
                "UPDATE goal_run_nodes SET node_id=printf('%*s',4194304,'x')",
                [],
            )
            .unwrap();
        assert_eq!(
            f.store.goal_run_details(&f.run.id).unwrap_err().code,
            "RESOURCE_LIMIT"
        );
    }
    #[test]
    fn recorded_cancel_fault_never_reports_not_started_and_same_key_recovery_does_not_repeat_stop()
    {
        for fault in ["journal", "raw_budget"] {
            let mut f = fixture(2);
            reserve(&mut f);
            let current = f.store.get_goal_run(&f.run.id).unwrap();
            if fault == "journal" {
                f.store.conn.execute_batch("CREATE TRIGGER generated_goal_stop_journal_fault BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='goal.node_stop_requested' BEGIN SELECT RAISE(ABORT,'generated stop receipt failure'); END;").unwrap();
            } else {
                f.store
                    .conn
                    .execute(
                        "UPDATE goal_run_nodes SET document=document||printf('%*s',300000,'')",
                        [],
                    )
                    .unwrap();
            }
            let error = f
                .store
                .cancel_goal_with_stops(&f.run.id, current.revision, "recorded-cancel")
                .unwrap_err();
            assert_ne!(error.side_effect_state, "not_started");
            assert!(error
                .details_ref
                .as_deref()
                .unwrap()
                .starts_with("recorded-goal-cancel:"));
            assert!(error.message.contains("stop remains unconfirmed"));
            let persisted = f.store.get_goal_run(&f.run.id).unwrap();
            assert!(persisted.cancel_requested);
            assert_eq!(persisted.state, "cancel_requested");
            let sequence = f.store.sequence().unwrap();
            let (recovered, stops) = f
                .store
                .cancel_goal_with_stops(&f.run.id, current.revision, "recorded-cancel")
                .unwrap();
            assert_eq!(recovered.revision, persisted.revision);
            assert!(stops.is_empty());
            assert_eq!(f.store.sequence().unwrap(), sequence);
        }
    }
    #[test]
    fn actual_process_exit_before_or_after_sent_marker_recovers_unknown_without_replay_or_false_cancel(
    ) {
        if let Some(manifest) = std::env::var_os("AB_GOAL_CRASH_TEST_MANIFEST") {
            let mut f = fixture(2);
            let session = reserve(&mut f);
            if std::env::var("AB_GOAL_CRASH_TEST_MODE").unwrap() == "sent" {
                f.store
                    .mark_dispatch_started(&session.intent_id, &session.id)
                    .unwrap();
            }
            let payload = json!({"database":f._directory.path().join("runtime/baton.sqlite3"),"run_id":f.run.id,"project_id":f.run.project_id,"session_id":session.id,"intent_id":session.intent_id});
            {
                use std::io::Write;
                let mut file = fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(manifest)
                    .unwrap();
                file.write_all(encode(&payload).unwrap().as_bytes())
                    .unwrap();
                file.sync_all().unwrap();
            }
            std::mem::forget(f);
            std::process::exit(73);
        }
        for mode in ["prepared", "sent"] {
            let owned = tempfile::tempdir().unwrap();
            let manifest = owned.path().join("crash-manifest.json");
            let mut child=Command::new(std::env::current_exe().unwrap()).args(["--exact","goal_store::runtime::tests::actual_process_exit_before_or_after_sent_marker_recovers_unknown_without_replay_or_false_cancel","--nocapture","--test-threads=1"]).env("AB_GOAL_CRASH_TEST_PARENT",owned.path()).env("AB_GOAL_CRASH_TEST_MANIFEST",&manifest).env("AB_GOAL_CRASH_TEST_MODE",mode).spawn().unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                if std::time::Instant::now() >= deadline {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    panic!("owned generated crash child exceeded its bound");
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            };
            assert_eq!(status.code(), Some(73));
            let evidence: Value = serde_json::from_slice(&fs::read(manifest).unwrap()).unwrap();
            let database = std::path::PathBuf::from(evidence["database"].as_str().unwrap());
            assert!(database
                .canonicalize()
                .unwrap()
                .starts_with(owned.path().canonicalize().unwrap()));
            let mut store = Store::open(&database).unwrap();
            let id = evidence["run_id"].as_str().unwrap();
            let current = store.goal_run_details(id).unwrap();
            assert_eq!(current["run"]["state"], "outcome_unknown");
            assert_eq!(current["nodes"][0]["state"], "outcome_unknown");
            assert_eq!(current["nodes"][0]["stop_confirmed"], false);
            assert!(current["nodes"][0]["actual_runtime_ms"].is_null());
            assert!(current["nodes"][0]["actual_output_bytes"].is_null());
            let revision = current["run"]["revision"].as_u64().unwrap();
            let (cancelled, stops) = store
                .cancel_goal_with_stops(id, revision, "unknown-cancel")
                .unwrap();
            assert_eq!(cancelled.state, "outcome_unknown");
            assert!(cancelled.cancel_requested);
            assert!(stops.is_empty());
            let meter: String = store
                .conn
                .query_row(
                    "SELECT document FROM goal_ledgers WHERE scope=?1",
                    [format!(
                        "project:{}",
                        evidence["project_id"].as_str().unwrap()
                    )],
                    |row| row.get(0),
                )
                .unwrap();
            let meter: MeterLedger = decode(&meter).unwrap();
            assert!(
                meter.runtime_ms.is_none()
                    && meter.output_bytes.is_none()
                    && meter.tokens.is_none()
                    && meter.cost_microunits.is_none()
            );
            let usage = store
                .budget(true, evidence["project_id"].as_str().unwrap())
                .unwrap()
                .unwrap();
            assert_eq!((usage.consumed, usage.active), (1, 1));
            assert_eq!(
                store
                    .conn
                    .query_row("SELECT count(*) FROM attempts", [], |row| row
                        .get::<_, u32>(0))
                    .unwrap(),
                1
            );
            assert_eq!(
                store
                    .conn
                    .query_row("SELECT count(*) FROM sessions", [], |row| row
                        .get::<_, u32>(0))
                    .unwrap(),
                1
            );
        }
    }
    #[test]
    fn native_execution_origin_requires_full_snapshot_commitment_source_and_positive_order() {
        for mode in ["missing", "source", "snapshot", "late_confirmation"] {
            let f = fixture(2);
            match mode {
                "missing" => {
                    f.store.conn.execute("DELETE FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.run_authorized'",[&f.run.id]).unwrap();
                }
                "source" => {
                    f.store.conn.execute("UPDATE events SET document=json_set(document,'$.source','generated_untrusted_agent_output') WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.run_authorized'",[&f.run.id]).unwrap();
                }
                "snapshot" => {
                    f.store.conn.execute("UPDATE approval_proposals SET document=json_set(document,'$.snapshot.warning','not the native-reviewed snapshot') WHERE id=?1",[&f.run.id]).unwrap();
                }
                _ => {
                    let doc:String=f.store.conn.query_row("SELECT document FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='approval.confirmed'",[&f.run.id],|row|row.get(0)).unwrap();
                    f.store.conn.execute("DELETE FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='approval.confirmed'",[&f.run.id]).unwrap();
                    f.store
                        .conn
                        .execute("INSERT INTO events(document) VALUES(?1)", [doc])
                        .unwrap();
                }
            }
            let p = GoalRunDispatch {
                run_id: f.run.id.clone(),
                expected_ledger_revision: f.store.sequence().unwrap(),
            };
            assert!(
                f.store.goal_dispatch_context(&p, f.run.revision).is_err(),
                "{mode}"
            );
            assert_eq!(
                f.store
                    .conn
                    .query_row("SELECT count(*) FROM attempts", [], |row| row
                        .get::<_, u32>(0))
                    .unwrap(),
                0
            );
        }
    }
    #[test]
    fn reserved_node_tampering_cannot_change_a_fixed_prompt_or_its_session_and_attempt() {
        for mode in [
            "goal",
            "budget",
            "sql_session",
            "sql_node_id",
            "unknown_state",
        ] {
            let mut f = fixture(2);
            let session = reserve(&mut f);
            let mut fake_ready = f.store.get_session(&session.id).unwrap();
            fake_ready.state = "ready".into();
            f.store
                .conn
                .execute(
                    "UPDATE sessions SET state='ready',document=?1 WHERE id=?2",
                    params![encode(&fake_ready).unwrap(), session.id],
                )
                .unwrap();
            match mode {
                "goal" => {
                    f.store.conn.execute("UPDATE goal_run_nodes SET document=json_set(document,'$.node.goal','unreviewed replacement prompt')",[]).unwrap();
                }
                "budget" => {
                    f.store.conn.execute("UPDATE goal_run_nodes SET document=json_set(document,'$.node.reservation.runtime_ms',9000)",[]).unwrap();
                }
                "unknown_state" => {
                    f.store.conn.execute("UPDATE goal_run_nodes SET state='unexpected_tag',document=json_set(document,'$.state','unexpected_tag')",[]).unwrap();
                }
                "sql_session" => {
                    f.store
                        .conn
                        .execute("UPDATE goal_run_nodes SET session_id=NULL", [])
                        .unwrap();
                }
                _ => {
                    f.store
                        .conn
                        .execute("UPDATE goal_run_nodes SET node_id='foreign-node'", [])
                        .unwrap();
                }
            }
            let p = GoalRunDispatch {
                run_id: f.run.id.clone(),
                expected_ledger_revision: f.store.sequence().unwrap(),
            };
            let revision = f.store.get_goal_run(&f.run.id).unwrap().revision;
            assert!(f.store.goal_prompt(&p, revision).is_err(), "{mode}");
            assert_eq!(
                f.store
                    .conn
                    .query_row("SELECT count(*) FROM session_turns", [], |row| row
                        .get::<_, u32>(0))
                    .unwrap(),
                0
            );
        }
    }
    fn adopt(f: &mut Fixture, plan: GoalPlanProposal) {
        let revision = plan.revision;
        f.store
            .submit_goal_plan(
                &GoalPlanSubmission {
                    scope_id: f.scope.id.clone(),
                    proposal: plan,
                },
                &format!("edit-{revision}"),
                revision - 1,
            )
            .unwrap();
        let current = f.store.get_goal_run(&f.run.id).unwrap();
        let p = GoalPlanAdvance {
            run_id: f.run.id.clone(),
            expected_plan_revision: revision,
            expected_ledger_revision: f.store.sequence().unwrap(),
        };
        f.store
            .advance_goal_plan(&p, current.revision, &format!("adopt-{revision}"))
            .unwrap();
    }
    #[test]
    fn newer_validated_head_pauses_once_without_intent_growth_and_explicit_adoption_replays() {
        let mut f = fixture(2);
        let mut plan = f.store.get_goal_plan(&f.scope.id).unwrap().proposal;
        plan.revision = 2;
        plan.expected_revision = 1;
        f.store
            .submit_goal_plan(
                &GoalPlanSubmission {
                    scope_id: f.scope.id.clone(),
                    proposal: plan,
                },
                "save-only-v2",
                1,
            )
            .unwrap();
        assert!(!f.store.goal_run_needs_advance(&f.run.id).unwrap());
        f.store.goal_stop_requests().unwrap();
        let paused = f.store.get_goal_run(&f.run.id).unwrap();
        assert_eq!(paused.state, "awaiting_acceptance");
        assert_eq!(paused.plan_revision, 1);
        let seq = f.store.sequence().unwrap();
        for _ in 0..10 {
            assert!(!f.store.goal_run_needs_advance(&f.run.id).unwrap());
            assert!(f.store.goal_stop_requests().unwrap().is_empty());
        }
        assert_eq!(f.store.sequence().unwrap(), seq);
        assert_eq!(
            f.store
                .conn
                .query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            0
        );
        let request = GoalPlanAdvance {
            run_id: f.run.id.clone(),
            expected_plan_revision: 2,
            expected_ledger_revision: seq,
        };
        let adopted = f
            .store
            .advance_goal_plan(&request, paused.revision, "explicit-adopt-v2")
            .unwrap();
        assert_eq!(adopted.plan_revision, 2);
        assert_eq!(adopted.state, "queued");
        let seq = f.store.sequence().unwrap();
        assert_eq!(
            encode(
                &f.store
                    .advance_goal_plan(&request, paused.revision, "explicit-adopt-v2")
                    .unwrap()
            )
            .unwrap(),
            encode(&adopted).unwrap()
        );
        assert_eq!(f.store.sequence().unwrap(), seq);
        assert!(f.store.goal_run_needs_advance(&f.run.id).unwrap());
        let dispatched = reserve(&mut f);
        assert_eq!(dispatched.task_id, f.run.task_id);
        assert_eq!(
            f.store
                .conn
                .query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            1
        );
    }
    #[test]
    fn implicit_input_and_parent_edges_do_not_starve_the_real_eligible_producer() {
        for mode in ["implicit_input", "parent"] {
            let mut f = fixture_with_outputs(2, true);
            let mut plan = f.store.get_goal_plan(&f.scope.id).unwrap().proposal;
            let mut producer = plan.nodes[0].clone();
            producer.id = "producer".into();
            producer.idempotency_key = "producer-action".into();
            producer.outputs.retain(|p| p.artifact_id == "intermediate");
            producer.budget.children = 1;
            let mut consumer = plan.nodes[0].clone();
            consumer.id = "consumer".into();
            consumer.idempotency_key = "consumer-action".into();
            consumer.outputs.retain(|p| p.artifact_id == "report");
            consumer.dependencies.clear();
            if mode == "implicit_input" {
                consumer.required_inputs = vec![RequiredInput {
                    goal_task_id: f.scope.task_id.clone(),
                    artifact_id: "intermediate".into(),
                    contract_id: f.scope.outputs["intermediate"].clone(),
                    source: InputSource::Node {
                        node_id: producer.id.clone(),
                        candidate_for_validation: false,
                    },
                }];
            } else {
                consumer.parent_id = Some(producer.id.clone());
            }
            plan.nodes = vec![consumer, producer];
            plan.revision = 2;
            plan.expected_revision = 1;
            adopt(&mut f, plan);
            assert!(f.store.goal_run_needs_advance(&f.run.id).unwrap());
            let p = GoalRunDispatch {
                run_id: f.run.id.clone(),
                expected_ledger_revision: f.store.sequence().unwrap(),
            };
            let current = f.store.get_goal_run(&f.run.id).unwrap();
            let context = f
                .store
                .goal_dispatch_context(&p, current.revision)
                .unwrap()
                .unwrap();
            assert_eq!(context.node.id, "producer", "{mode}");
            f.store.create_goal_session(context).unwrap();
            let sequence = f.store.sequence().unwrap();
            for _ in 0..10 {
                assert!(!f.store.goal_run_needs_advance(&f.run.id).unwrap());
            }
            assert_eq!(f.store.sequence().unwrap(), sequence);
            assert_eq!(
                f.store
                    .conn
                    .query_row("SELECT count(*) FROM attempts", [], |row| row
                        .get::<_, u32>(0))
                    .unwrap(),
                1
            );
        }
    }
    #[test]
    fn unapproved_output_alias_never_becomes_zero_observed_output_or_execution_authority() {
        let mut f = fixture(2);
        let mut plan = f.store.get_goal_plan(&f.scope.id).unwrap().proposal;
        let mut alias = plan.nodes[0].clone();
        alias.id = "alias-first".into();
        alias.idempotency_key = "alias-action".into();
        alias.outputs[0].artifact_id = "unapproved-alias".into();
        plan.nodes.insert(0, alias);
        plan.revision = 2;
        plan.expected_revision = 1;
        f.store
            .submit_goal_plan(
                &GoalPlanSubmission {
                    scope_id: f.scope.id.clone(),
                    proposal: plan,
                },
                "valid-structural-alias",
                1,
            )
            .unwrap();
        let current = f.store.get_goal_run(&f.run.id).unwrap();
        let p = GoalPlanAdvance {
            run_id: f.run.id.clone(),
            expected_plan_revision: 2,
            expected_ledger_revision: f.store.sequence().unwrap(),
        };
        assert_eq!(
            f.store
                .advance_goal_plan(&p, current.revision, "no-physical-alias")
                .unwrap_err()
                .code,
            "CAPABILITY_UNSUPPORTED"
        );
        assert_eq!(
            f.store
                .conn
                .query_row("SELECT count(*) FROM attempts", [], |row| row
                    .get::<_, u32>(0))
                .unwrap(),
            0
        );
        let status = f.store.goal_run_details(&f.run.id).unwrap();
        assert!(status["nodes"][0]["actual_output_bytes"].is_null());
    }
    #[test]
    fn autonomous_plan_advance_requires_an_unchanged_ordered_core_receipt_chain() {
        for mode in ["missing", "source", "digest", "order"] {
            let mut f = fixture(2);
            grow(&mut f, 2, 2);
            let r = f.store.get_goal_run(&f.run.id).unwrap();
            let request = GoalRunDispatch {
                run_id: r.id.clone(),
                expected_ledger_revision: f.store.sequence().unwrap(),
            };
            assert!(f
                .store
                .goal_dispatch_context(&request, r.revision)
                .unwrap()
                .is_some());
            let filter = "json_extract(document,'$.kind')='goal.run_plan_advanced'";
            let sql=match mode {
                "missing"=>format!("DELETE FROM events WHERE {filter}"),
                "source"=>format!("UPDATE events SET document=json_set(document,'$.source','generated_agent_output') WHERE {filter}"),
                "digest"=>format!("UPDATE events SET document=json_set(document,'$.payload.previous_plan_digest','wrong') WHERE {filter}"),
                _=>format!("UPDATE events SET sequence=-sequence WHERE {filter}"),
            };
            f.store.conn.execute(&sql, []).unwrap();
            let request = GoalRunDispatch {
                run_id: r.id,
                expected_ledger_revision: f.store.sequence().unwrap(),
            };
            assert!(
                f.store.goal_dispatch_context(&request, r.revision).is_err(),
                "{mode}"
            );
            assert_eq!(
                f.store
                    .conn
                    .query_row("SELECT count(*) FROM attempts", [], |row| row
                        .get::<_, u32>(0))
                    .unwrap(),
                0
            );
        }
    }
    #[test]
    fn persisted_project_optional_limits_cannot_be_raised_or_removed_by_a_later_root() {
        let f = fixture(2);
        let key = format!("project:{}", f.run.project_id);
        let mut historical = f.scope.project_budget.clone();
        historical.tokens = Some(100);
        historical.cost_microunits = Some(100);
        let record = MeterLedger {
            limits: historical.clone(),
            observed_attempts: 0,
            runtime_ms: Some(0),
            output_bytes: Some(0),
            tokens: Some(0),
            cost_microunits: Some(0),
        };
        let raw = encode(&record).unwrap();
        f.store
            .conn
            .execute(
                "INSERT OR REPLACE INTO goal_ledgers(scope,document) VALUES(?1,?2)",
                params![key, raw],
            )
            .unwrap();
        for dimension in ["tokens", "cost"] {
            for value in [Some(101), None] {
                let mut later_root_limits = historical.clone();
                if dimension == "tokens" {
                    later_root_limits.tokens = value;
                } else {
                    later_root_limits.cost_microunits = value;
                }
                assert_eq!(
                    meter(&f.store.conn, &key, &later_root_limits, 0)
                        .unwrap_err()
                        .code,
                    "PERMISSION_DENIED"
                );
                let after: String = f
                    .store
                    .conn
                    .query_row(
                        "SELECT document FROM goal_ledgers WHERE scope=?1",
                        [&key],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(after, raw);
            }
        }
        let unchanged = meter(&f.store.conn, &key, &historical, 0).unwrap();
        assert_eq!(unchanged.tokens, Some(0));
        assert_eq!(unchanged.cost_microunits, Some(0));
        let mut narrowed = historical;
        narrowed.tokens = Some(99);
        narrowed.cost_microunits = Some(99);
        assert!(meter(&f.store.conn, &key, &narrowed, 0).is_ok());
    }
    #[test]
    fn zero_history_declared_token_and_cost_reservations_do_not_prove_metering_capability() {
        for dimension in ["tokens", "cost"] {
            let mut f = fixture(2);
            let mut plan = f.store.get_goal_plan(&f.scope.id).unwrap().proposal;
            plan.revision = 2;
            plan.expected_revision = 1;
            if dimension == "tokens" {
                plan.nodes[0].budget.tokens = Some(100);
                plan.nodes[0].reservation.tokens = Some(10);
            } else {
                plan.nodes[0].budget.cost_microunits = Some(100);
                plan.nodes[0].reservation.cost_microunits = Some(10);
            }
            f.store
                .submit_goal_plan(
                    &GoalPlanSubmission {
                        scope_id: f.scope.id.clone(),
                        proposal: plan,
                    },
                    "finite-unmeasured",
                    1,
                )
                .unwrap();
            let p = GoalRunSelection {
                scope_id: f.scope.id.clone(),
                expected_plan_revision: 2,
                expected_ledger_revision: f.store.sequence().unwrap(),
            };
            assert_eq!(
                f.store
                    .propose_goal_run(&p, "finite-cannot-start")
                    .unwrap_err()
                    .code,
                "CAPABILITY_UNSUPPORTED"
            );
            assert_eq!(
                f.store
                    .conn
                    .query_row("SELECT count(*) FROM attempts", [], |row| row
                        .get::<_, u32>(0))
                    .unwrap(),
                0
            );
        }
    }
}
