//! Explicit Factory issuer for the existing Graph scheduler. No native Goal
//! proposal is invented. Every private token is re-derived from durable proofs.
use super::*;
use crate::factory::{FactoryChildGoalSummary, FactoryChildPlanRef, FactoryGoalSource};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FactoryGoalOrigin {
    pub(crate) factory_scope_id: String,
    pub(crate) factory_run_id: String,
    pub(crate) root_task_id: String,
    pub(crate) parent_task_id: Option<String>,
    pub(crate) ancestors: Vec<String>,
    pub(crate) execution_policy_digest: String,
    pub(crate) planning_policy_digest: String,
    pub(crate) initial_plan: FactoryChildPlanRef,
    pub(crate) policy: HostGoalPolicy,
    pub(crate) approval_revision: u64,
    pub(crate) expires_at_ms: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DerivedRecord {
    run_id: String,
    scope_id: String,
    origin: FactoryGoalOrigin,
    scope_record_digest: String,
    initial_run: GoalRun,
    snapshot: Value,
    caller_task_id: String,
    caller_session_id: Option<String>,
    caller_attempt_id: Option<String>,
    caller_fencing_token: Option<u64>,
    receipt_sequence: u64,
}
fn commitment(record: &DerivedRecord) -> Result<String> {
    let mut v = json!(record);
    v.as_object_mut()
        .ok_or_else(changed)?
        .remove("receipt_sequence");
    sha(&v)
}
pub(super) fn marked_scope(db: &Connection, scope_id: &str) -> Result<bool> {
    db.query_row("SELECT EXISTS(SELECT 1 FROM goal_scopes WHERE id=?1 AND json_type(document,'$.factory_origin') IS NOT NULL AND json_type(document,'$.factory_origin')!='null')",[scope_id],|r|r.get(0)).map_err(db_error)
}
pub(super) fn marked_run(db: &Connection, id: &str) -> Result<bool> {
    let version: u32 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(db_error)?;
    if version < 15 {
        return Ok(false);
    }
    db.query_row("SELECT EXISTS(SELECT 1 FROM factory_child_goal_runs WHERE goal_run_id=?1) OR EXISTS(SELECT 1 FROM goal_runs r JOIN goal_scopes s ON s.id=r.scope_id WHERE r.id=?1 AND json_type(s.document,'$.factory_origin') IS NOT NULL AND json_type(s.document,'$.factory_origin')!='null')",[id],|r|r.get(0)).map_err(db_error)
}
fn checked_record(db: &Connection, id: &str) -> Result<DerivedRecord> {
    key(id)?;
    let rows:i64=db.query_row("SELECT COALESCE(sum(octet_length(document)+octet_length(goal_run_id)+octet_length(goal_scope_id)+octet_length(factory_scope_id)+octet_length(factory_run_id)+octet_length(task_id)+octet_length(root_task_id)+COALESCE(octet_length(parent_task_id),0)+octet_length(plan_revision_id)+octet_length(record_digest)),0) FROM factory_child_goal_runs WHERE goal_run_id=?1",[id],|r|r.get(0)).map_err(db_error)?;
    if rows > MAX_STORED_BYTES {
        return Err(limited());
    }
    let stored:Option<(String,String,u64)>=db.query_row("SELECT document,record_digest,receipt_sequence FROM factory_child_goal_runs WHERE goal_run_id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(db_error)?;
    let (doc, digest, seq) = stored.ok_or_else(denied)?;
    let record: DerivedRecord = decode(&doc)?;
    let s = crate::factory::checked_child_goal_scope(db, &record.origin.factory_scope_id)?;
    let execution = s
        .bindings
        .child_goal_execution
        .as_ref()
        .ok_or_else(denied)?;
    let scope: ScopeRecord = raw(db, "goal_scopes", &record.scope_id)?;
    let plan = crate::factory::checked_child_goal_plan(db, &record.origin.initial_plan)?;
    let columns:bool=db.query_row("SELECT goal_scope_id=?2 AND factory_scope_id=?3 AND factory_run_id=?4 AND task_id=?5 AND root_task_id=?6 AND parent_task_id IS ?7 AND plan_revision_id=?8 AND node_count=?9 FROM factory_child_goal_runs WHERE goal_run_id=?1",params![id,record.scope_id,s.id,record.origin.factory_run_id,record.initial_run.task_id,s.root_task_id,record.origin.parent_task_id,plan.reference.id,plan.proposal.nodes.len() as i64],|r|r.get(0)).map_err(db_error)?;
    if !columns
        || record.run_id != id
        || record.initial_run.id != id
        || record.initial_run.scope_id != record.scope_id
        || record.initial_run.root_grant_id != s.root_grant_id
        || record.initial_run.project_id != s.project_id
        || record.origin.root_task_id != s.root_task_id
        || record.origin.execution_policy_digest != execution.definition_digest
        || record.origin.planning_policy_digest != execution.planning_policy_digest
        || record.origin.initial_plan != plan.reference
        || record.origin.factory_run_id != plan.reference.run_id
        || record.initial_run.task_id != plan.reference.task_id
        || record.initial_run.plan_digest != plan.reference.plan_digest
        || record.initial_run.plan_revision != plan.reference.revision
        || scope.factory_origin.as_ref().map(json_value).transpose()? != Some(json!(record.origin))
        || sha(&scope)? != record.scope_record_digest
        || commitment(&record)? != digest
        || seq != record.receipt_sequence
        || record.snapshot["factory_origin"] != json!(record.origin)
        || record.snapshot["scope"]["scope_digest"] != scope.scope_digest
        || record.snapshot["plan"]["digest"] != plan.reference.plan_digest
        || record.snapshot["plan"]["proposal"] != json!(plan.proposal)
        || record.snapshot["new_workspace_authority"] != false
        || record.snapshot["execution"] != "factory_child_goal_graph_v1_existing_workspace"
        || record.origin.approval_revision != 1
        || record.origin.expires_at_ms != s.expires_at_ms
    {
        return Err(changed());
    }
    crate::factory::verify_child_goal_lineage(
        db,
        &s,
        &record.initial_run.task_id,
        &record.origin.ancestors,
        record.origin.parent_task_id.as_deref(),
    )?;
    let e:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM events WHERE sequence=?1",[sql_num(seq)?],|r|r.get(0)).optional().map_err(db_error)?;
    let e: Event = decode(&e.flatten().ok_or_else(denied)?)?;
    if e.aggregate_id != id
        || e.source != "factory_goal_core"
        || e.kind != "factory.child_goal_authorized"
        || e.payload["record_digest"] != digest
        || e.payload["factory_scope_id"] != s.id
        || e.payload["factory_run_id"] != record.origin.factory_run_id
        || e.payload["task_id"] != record.initial_run.task_id
        || e.payload["plan"] != json!(record.origin.initial_plan)
        || e.payload["execution_policy_digest"] != execution.definition_digest
        || seq <= plan.reference.receipt_sequence
    {
        return Err(denied());
    }
    let native_conflict:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM goal_graph_capabilities WHERE run_id=?1) OR EXISTS(SELECT 1 FROM approval_proposals WHERE id=?1)",[id],|r|r.get(0)).map_err(db_error)?;
    if native_conflict {
        return Err(changed());
    }
    Ok(record)
}
fn json_value<T: Serialize>(v: &T) -> Result<Value> {
    serde_json::to_value(v).map_err(|_| changed())
}

pub(super) fn capability(db: &Connection, r: &GoalRun) -> Result<graph::GraphAuthority> {
    let record = checked_record(db, &r.id)?;
    if r.scope_id != record.scope_id
        || r.task_id != record.initial_run.task_id
        || r.project_id != record.initial_run.project_id
        || r.root_grant_id != record.initial_run.root_grant_id
    {
        return Err(changed());
    }
    if r.plan_revision != record.initial_run.plan_revision
        || r.plan_digest != record.initial_run.plan_digest
    {
        // Explicit Factory adoption will verify its own ordered receipt chain;
        // native Goal advances never stand in for Factory plan adoption.
        return Err(err(
            "AUTH_REQUIRED",
            "Factory child Goal plan adoption requires its exact Factory receipt chain",
        ));
    }
    Ok(graph::GraphAuthority {
        issuer: graph::GraphAuthorityIssuer::FactoryScope {
            scope_id: record.origin.factory_scope_id,
        },
        snapshot: record.snapshot,
    })
}
pub(super) fn historical_scope(db: &Connection, id: &str) -> Result<GoalScopeView> {
    let run_id: String = db
        .query_row(
            "SELECT goal_run_id FROM factory_child_goal_runs WHERE goal_scope_id=?1",
            [id],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    let record = checked_record(db, &run_id)?;
    let scope: ScopeRecord = raw(db, "goal_scopes", id)?;
    if record.scope_id != id {
        return Err(changed());
    }
    recorded_scope_view(&scope)
}
pub(super) fn live_scope(
    db: &Connection,
    r: &GoalRun,
    data: Option<&crate::data_workspaces::DataInputWitness>,
    require_head: bool,
) -> Result<(ScopeRecord, ApprovedGoalScope)> {
    let record = checked_record(db, &r.id)?;
    capability(db, r)?;
    crate::factory::live_child_goal_scope(db, &record.origin.factory_scope_id, &r.task_id, data)?;
    let scope: ScopeRecord = raw(db, "goal_scopes", &r.scope_id)?;
    let task: Task = raw(db, "tasks", &r.task_id)?;
    if scope.revoked
        || task.project_id != r.project_id
        || task.spec_revision != scope.refs.spec_revision
        || task.goal != scope.refs.task_goal
        || matches!(
            task.state,
            TaskState::Accepted | TaskState::Cancelled | TaskState::CancelRequested
        )
    {
        return Err(denied());
    }
    let original = crate::policy::validate_grant_live_from_with_data(
        db,
        &r.root_grant_id,
        &r.project_id,
        &scope.refs.workspace.id,
        scope.refs.connection["id"].as_str().ok_or_else(changed)?,
        data,
    )?;
    if original != scope.refs.grant_after {
        return Err(changed());
    }
    let approved = approved_scope(&task, &scope)?;
    let (_, plan) = current_plan(db, &r.scope_id, &approved)?;
    if require_head && (plan.digest != r.plan_digest || plan.proposal.revision != r.plan_revision) {
        return Err(err(
            "STALE_REVISION",
            "Factory child Goal must explicitly adopt its saved head",
        ));
    }
    Ok((scope, approved))
}
pub(super) fn run_live(db: &Connection, id: &str) -> Result<bool> {
    if !marked_run(db, id)? {
        return Ok(true);
    }
    let record = checked_record(db, id)?;
    Ok(crate::factory::child_goal_policy_live(
        db,
        &record.origin.factory_scope_id,
        &record.initial_run.task_id,
    )
    .is_ok())
}

/// Migration gate for historical native rows. This is immutable provenance
/// verification, not a live dispatch/readiness check, so expiry or legitimate
/// later cancellation cannot erase old authority history.
pub(crate) fn verify_native_history_before_migration(db: &Connection) -> Result<()> {
    let mut scopes = db
        .prepare(
            "SELECT CASE WHEN octet_length(id)<=128 THEN id END FROM goal_scopes ORDER BY rowid",
        )
        .map_err(db_error)?;
    let mut rows = scopes.query([]).map_err(db_error)?;
    while let Some(row) = rows.next().map_err(db_error)? {
        let id: Option<String> = row.get(0).map_err(db_error)?;
        let id = id.ok_or_else(limited)?;
        let scope: ScopeRecord = raw(db, "goal_scopes", &id)?;
        if scope.factory_origin.is_some() {
            return Err(changed());
        }
        stored_scope_view(db, &id)?;
    }
    let mut runs = db
        .prepare("SELECT CASE WHEN octet_length(id)<=128 THEN id END FROM goal_runs ORDER BY rowid")
        .map_err(db_error)?;
    let mut rows = runs.query([]).map_err(db_error)?;
    while let Some(row) = rows.next().map_err(db_error)? {
        let id: Option<String> = row.get(0).map_err(db_error)?;
        let r = run(db, &id.ok_or_else(limited)?)?;
        let graph = graph::is_graph_run(db, &r.id)?;
        let (proposal, result) = native_confirmation(
            db,
            &r.id,
            if graph {
                "goal_graph_execution"
            } else {
                "goal_execution"
            },
        )?;
        let initial = if graph {
            graph::checked_capability(db, &r)?;
            result["run"].clone()
        } else {
            result
        };
        if initial["id"] != r.id
            || initial["scope_id"] != r.scope_id
            || initial["task_id"] != r.task_id
            || initial["project_id"] != r.project_id
            || initial["root_grant_id"] != r.root_grant_id
            || proposal.snapshot["plan"]["digest"] != initial["plan_digest"]
            || proposal.snapshot["plan"]["proposal"]["revision"] != initial["plan_revision"]
        {
            return Err(changed());
        }
        let digest = sha(&proposal.snapshot)?;
        let (confirmed,authorized):(Option<u64>,Option<u64>)=db.query_row("SELECT (SELECT min(sequence) FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.source')='local_owner_confirmation' AND json_extract(document,'$.kind')='approval.confirmed' AND json_extract(document,'$.payload.kind')=?2 AND json_extract(document,'$.payload.run_id')=?1 AND json_extract(document,'$.payload.execution_snapshot_digest')=?3),(SELECT min(sequence) FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.kind')='goal.run_authorized' AND json_extract(document,'$.payload.scope_id')=?4 AND json_extract(document,'$.payload.plan_digest')=?5 AND json_extract(document,'$.payload.plan_revision')=?6 AND json_extract(document,'$.payload.execution_snapshot_digest')=?3 AND json_extract(document,'$.payload.reservation_made')=0 AND json_extract(document,'$.payload.dispatched')=0)",params![r.id,proposal.kind,digest,r.scope_id,initial["plan_digest"].as_str().ok_or_else(changed)?,initial["plan_revision"].as_u64().ok_or_else(changed)?],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
        if !matches!((confirmed,authorized),(Some(a),Some(b)) if a>0&&b>a) {
            return Err(denied());
        }
        verify_plan_advances(db, &r, &proposal, &initial, authorized.ok_or_else(denied)?)?;
    }
    Ok(())
}

impl Store {
    /// One atomic issuer record, derived scope, original plan history, Graph run,
    /// nodes and action receipt. No Provider boundary is crossed by this action.
    pub(crate) fn create_factory_child_graph(
        &mut self,
        source: FactoryGoalSource,
        caller: Option<(String, String, String, u64)>,
        key_id: &str,
        action_payload: &str,
    ) -> Result<FactoryChildGoalSummary> {
        key(key_id)?;
        let selected = source.plans.last().ok_or_else(changed)?;
        let execution = source
            .scope
            .bindings
            .child_goal_execution
            .as_ref()
            .ok_or_else(denied)?;
        let scope_id = Uuid::new_v4().to_string();
        let run_id = Uuid::new_v4().to_string();
        let origin = FactoryGoalOrigin {
            factory_scope_id: source.scope.id.clone(),
            factory_run_id: source.factory_run.id.clone(),
            root_task_id: source.scope.root_task_id.clone(),
            parent_task_id: source.parent_task_id.clone(),
            ancestors: source.ancestors.clone(),
            execution_policy_digest: execution.definition_digest.clone(),
            planning_policy_digest: execution.planning_policy_digest.clone(),
            initial_plan: selected.reference.clone(),
            policy: source.host_policy.clone(),
            approval_revision: 1,
            expires_at_ms: source.scope.expires_at_ms,
        };
        let (pc, pa, rc, ra) = counts(
            &self.conn,
            &source.task,
            &source.scope.bindings.approved_grant,
        )?;
        let outputs = source
            .scope
            .selection
            .child_planning_policy
            .as_ref()
            .ok_or_else(denied)?
            .outputs
            .iter()
            .filter(|o| {
                source
                    .request
                    .output_artifact_ids
                    .contains(&o.artifact.artifact_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        let refs = GoalRefs {
            task_id: source.task.id.clone(),
            project_id: source.task.project_id.clone(),
            task_goal: source.task.goal.clone(),
            spec_revision: source.task.spec_revision,
            project_revision: source.scope.bindings.project.revision,
            workspace: source
                .scope
                .bindings
                .workspaces
                .iter()
                .find(|w| w.id == source.request.workspace_id)
                .ok_or_else(changed)?
                .clone(),
            connection: json!(source
                .scope
                .bindings
                .connections
                .iter()
                .find(|c| c.id == source.request.connection_id)
                .ok_or_else(changed)?),
            grant_before: source.scope.bindings.approved_grant.clone(),
            grant_after: source.scope.bindings.approved_grant.clone(),
            grant_confirmation_id: grant_origin(&self.conn, &source.scope.bindings.approved_grant)?,
            inputs: source
                .scope
                .bindings
                .inputs
                .iter()
                .filter(|i| {
                    source
                        .request
                        .input_artifact_ids
                        .contains(&i.contract.requirement.artifact_id)
                })
                .map(|i| ResolvedInputFile {
                    artifact: i.contract.requirement.clone(),
                    sha256: i.sha256.clone(),
                })
                .collect(),
            validators: source
                .scope
                .bindings
                .child_planning
                .as_ref()
                .ok_or_else(denied)?
                .validator_definitions
                .values()
                .filter(|v| {
                    outputs
                        .iter()
                        .any(|o| o.validators.iter().any(|r| json!(r) == v["reference"]))
                })
                .map(|v| serde_json::from_value(v.clone()).map_err(|_| changed()))
                .collect::<Result<Vec<ResolvedValidator>>>()?,
            project_consumed: pc,
            project_active: pa,
            root_consumed: rc,
            root_active: ra,
            root_children_consumed: 0,
            project_children_consumed: 0,
        };
        let record = ScopeRecord {
            factory_origin: Some(origin.clone()),
            id: scope_id.clone(),
            revision: 1,
            revoked: false,
            approval_id: source.scope.approval_id.clone(),
            selection: GoalScopeSelection {
                task_id: source.task.id.clone(),
                expected_task_revision: source.task.revision,
                grant_id: source.scope.root_grant_id.clone(),
                expected_grant_revision: source.scope.root_grant_revision,
                max_nodes: execution.policy.max_nodes,
                new_max_children: source.scope.bindings.approved_grant.scope.max_children,
                new_max_depth: origin.policy.max_depth,
                root_budget: execution.policy.root_budget.clone(),
                project_budget: execution.policy.project_budget.clone(),
                inputs: refs.inputs.iter().map(|i| i.artifact.clone()).collect(),
                outputs,
                writable_prefixes: origin
                    .policy
                    .workspaces
                    .values()
                    .flat_map(|w| w.writable_prefixes.clone())
                    .collect(),
                owner_acceptance_criteria: source
                    .scope
                    .selection
                    .child_planning_policy
                    .as_ref()
                    .ok_or_else(denied)?
                    .owner_acceptance_criteria
                    .clone(),
            },
            refs,
            scope_digest: origin.planning_policy_digest.clone(),
            created_at: now()?,
        };
        let approved = approved_scope(&source.task, &record)?;
        let plan = ValidatedPlan::from_verified_factory_saved(&approved, selected.proposal.clone())
            .map_err(plan_error)?;
        if plan.digest() != selected.reference.plan_digest {
            return Err(changed());
        }
        require_output_mappings(&record.selection, plan.proposal())?;
        require_metered_capabilities(plan.proposal())?;
        let timestamp = now()?;
        let initial = GoalRun {
            id: run_id.clone(),
            scope_id: scope_id.clone(),
            project_id: source.task.project_id.clone(),
            task_id: source.task.id.clone(),
            root_grant_id: source.scope.root_grant_id.clone(),
            plan_revision: selected.reference.revision,
            plan_digest: selected.reference.plan_digest.clone(),
            revision: 1,
            state: "queued".into(),
            cancel_requested: false,
            task_accepted: false,
            created_at: timestamp.clone(),
            updated_at: timestamp,
        };
        let plan_view = GoalPlanView {
            scope_id: scope_id.clone(),
            proposal: selected.proposal.clone(),
            digest: plan.digest().into(),
            created_at: selected.created_at.clone(),
            state: "validated_proposal".into(),
            dispatched: false,
        };
        let snapshot = json!({"graph_contract_version":1,"factory_origin":origin,"scope":recorded_scope_view(&record)?,"root_grant":record.refs.grant_after,"existing_connection":connection_summary(&record.refs)?,"plan":plan_view,"definition_reuse":graph::definition_permissions(&record)?,"execution":"factory_child_goal_graph_v1_existing_workspace","allow_in_scope_plan_revisions":true,"new_workspace_authority":false,"enforcement":{"filesystem_sandbox":false,"network_sandbox":false,"output_quota":false},"subject_rebinding":{"mode":"this_run_current_plan_owned_producer_only","requires_owned_stop_and_capture":true,"allows_legacy_candidate_backfill":false},"budgets":{"root":execution.policy.root_budget,"project":execution.policy.project_budget},"io_limits":{"max_total_artifact_bytes":16777216,"max_artifact_refs":64,"cancel_requires_actual_join":true,"timeout_without_join_is_unknown":true},"native_factory_approval_id":source.scope.approval_id});
        let mut derived = DerivedRecord {
            run_id: run_id.clone(),
            scope_id: scope_id.clone(),
            origin,
            scope_record_digest: sha(&record)?,
            initial_run: initial.clone(),
            snapshot,
            caller_task_id: caller
                .as_ref()
                .map_or(source.task.id.clone(), |c| c.0.clone()),
            caller_session_id: caller.as_ref().map(|c| c.1.clone()),
            caller_attempt_id: caller.as_ref().map(|c| c.2.clone()),
            caller_fencing_token: caller.as_ref().map(|c| c.3),
            receipt_sequence: 0,
        };
        if encode(&derived)?.len() > MAX_STORED_BYTES as usize {
            return Err(limited());
        }
        let namespace = format!(
            "factory:{}:child-goal:{}:{}",
            source.scope.id, derived.caller_task_id, source.task.id
        );
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(old) =
            replay::<FactoryChildGoalSummary>(&tx, &namespace, key_id, action_payload)?
        {
            checked_record(&tx, &old.goal.run.id)?;
            return Ok(old);
        }
        crate::factory::factory_goal_source_is_current(&tx, &source)?;
        let active: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM goal_scopes WHERE task_id=?1 AND revoked=0)",
                [&source.task.id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if active {
            return Err(err("CONFIG_CONFLICT","This Factory target already has an execution scope; adopt a new saved plan in its existing run"));
        }
        tx.execute("INSERT INTO goal_scopes(id,task_id,root_grant_id,revision,revoked,document) VALUES(?1,?2,?3,1,0,?4)",params![scope_id,source.task.id,source.scope.root_grant_id,encode(&record)?]).map_err(db_error)?;
        for saved in &source.plans {
            let proposal = &saved.proposal;
            let previous = source
                .plans
                .iter()
                .find(|p| p.reference.revision + 1 == saved.reference.revision)
                .map(|p| p.reference.plan_digest.as_str());
            let submission = GoalPlanRawSubmission {
                scope_id: scope_id.clone(),
                plan_json: saved.plan_json.clone(),
            };
            tx.execute("INSERT INTO goal_plan_proposals(id,scope_id,plan_id,revision,document,digest) VALUES(?1,?2,?3,?4,?5,?6)",params![Uuid::new_v4().to_string(),scope_id,proposal.plan_id,sql_num(proposal.revision)?,encode(&submission)?,saved.reference.plan_digest]).map_err(db_error)?;
            tx.execute("INSERT INTO goal_plan_revisions(scope_id,plan_id,revision,previous_digest,digest,document,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![scope_id,proposal.plan_id,sql_num(proposal.revision)?,previous,saved.reference.plan_digest,encode(proposal)?,saved.created_at]).map_err(db_error)?;
            event(
                &tx,
                &proposal.plan_id,
                proposal.revision,
                "goal.plan_validated",
                json!({"scope_id":scope_id,"plan_id":proposal.plan_id,"plan_revision":proposal.revision,"digest":saved.reference.plan_digest,"previous_digest":previous,"dispatched":false,"reservation_made":false,"factory_saved_plan":saved.reference}),
            )?;
        }
        tx.execute(
            "INSERT INTO goal_plan_heads(scope_id,plan_id,revision,digest) VALUES(?1,?2,?3,?4)",
            params![
                scope_id,
                selected.proposal.plan_id,
                sql_num(selected.reference.revision)?,
                selected.reference.plan_digest
            ],
        )
        .map_err(db_error)?;
        tx.execute("INSERT INTO goal_runs(id,scope_id,project_id,task_id,revision,state,document) VALUES(?1,?2,?3,?4,1,'queued',?5)",params![run_id,scope_id,source.task.project_id,source.task.id,encode(&initial)?]).map_err(db_error)?;
        for (ordinal, node) in selected.proposal.nodes.iter().enumerate() {
            let n = RuntimeNode {
                node: node.clone(),
                plan_revision: initial.plan_revision,
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
                    run_id,
                    sql_num(initial.plan_revision)?,
                    node.id,
                    ordinal as i64,
                    encode(&n)?
                ],
            )
            .map_err(db_error)?;
        }
        let digest = commitment(&derived)?;
        let event = Event {
            sequence: 0,
            event_id: Uuid::new_v4().to_string(),
            aggregate_id: run_id.clone(),
            revision: 1,
            kind: "factory.child_goal_authorized".into(),
            occurred_at: now()?,
            source: "factory_goal_core".into(),
            payload: json!({"record_digest":digest,"factory_scope_id":source.scope.id,"factory_run_id":source.factory_run.id,"task_id":source.task.id,"plan":selected.reference,"execution_policy_digest":execution.definition_digest,"dispatched":false,"reservation_made":false,"new_grant":false,"task_accepted":false}),
        };
        tx.execute("INSERT INTO events(document) VALUES(?1)", [encode(&event)?])
            .map_err(db_error)?;
        derived.receipt_sequence = sequence(&tx)?;
        tx.execute(
            "INSERT INTO factory_child_goal_runs VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                run_id,
                scope_id,
                source.scope.id,
                source.factory_run.id,
                source.task.id,
                source.scope.root_task_id,
                source.parent_task_id,
                selected.reference.id,
                selected.proposal.nodes.len() as i64,
                sql_num(derived.receipt_sequence)?,
                digest,
                encode(&derived)?
            ],
        )
        .map_err(db_error)?;
        let view = FactoryChildGoalSummary {
            authority_kind: "factory_child_goal_execution_v1".into(),
            factory_scope_id: source.scope.id.clone(),
            factory_run_id: source.factory_run.id.clone(),
            factory_root_task_id: source.scope.root_task_id.clone(),
            parent_task_id: source.parent_task_id.clone(),
            task_id: source.task.id.clone(),
            plan: selected.reference.clone(),
            execution_policy_digest: execution.definition_digest.clone(),
            goal: GraphRunSummary {
                graph_contract_version: 1,
                run: initial,
                ledger_revision: sequence(&tx)?,
                task_accepted: false,
            },
        };
        if encode(
            &json!({"request_id":"\0".repeat(128),"result":view,"revision":u64::MAX,"warnings":[]}),
        )?
        .len()
            > MAX_STORED_BYTES as usize
        {
            return Err(limited());
        }
        checked_record(&tx, &run_id)?;
        remember(&tx, &namespace, key_id, action_payload, &view)?;
        tx.commit().map_err(db_error)?;
        Ok(view)
    }
}

pub(super) fn origin_view(
    db: &Connection,
    r: &GoalRun,
) -> Result<Option<crate::factory::FactoryChildGoalOriginView>> {
    if !marked_run(db, &r.id)? {
        return Ok(None);
    }
    let record = checked_record(db, &r.id)?;
    let scope = crate::factory::checked_child_goal_scope(db, &record.origin.factory_scope_id)?;
    Ok(Some(crate::factory::FactoryChildGoalOriginView {
        factory_scope_id: scope.id.clone(),
        factory_scope_digest: scope.contract_digest.clone(),
        factory_run_id: record.origin.factory_run_id,
        factory_root_task_id: scope.root_task_id.clone(),
        parent_task_id: record.origin.parent_task_id,
        ancestors: record.origin.ancestors,
        original_native_approval_id: scope.approval_id.clone(),
        root_grant_id: scope.root_grant_id.clone(),
        root_grant_revision: scope.root_grant_revision,
        initial_plan: record.origin.initial_plan,
        execution_binding: scope
            .bindings
            .child_goal_execution
            .clone()
            .ok_or_else(denied)?,
    }))
}

/// Read-only shared accounting projection of existing Graph reservations. It
/// neither creates a Provider session nor duplicates a root/project charge.
pub(crate) fn budget_records(
    db: &Connection,
    root: &str,
    project: &str,
) -> Result<Vec<crate::factory::FactoryReservation>> {
    let version: u32 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(db_error)?;
    if version < 15 {
        return Ok(vec![]);
    }
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(x.document)+octet_length(x.attempt_id)+octet_length(x.run_id)+octet_length(x.node_id)),0),COALESCE(max(octet_length(x.document)),0) FROM goal_run_reservations x JOIN factory_child_goal_runs f ON f.goal_run_id=x.run_id JOIN goal_runs r ON r.id=x.run_id WHERE r.project_id=?1 OR json_extract(r.document,'$.root_grant_id')=?2",params![project,root],100000,MAX_GOAL_PLAN_BYTES as i64)?;
    let mut q=db.prepare("SELECT x.attempt_id,x.run_id,x.node_id,x.document FROM goal_run_reservations x JOIN factory_child_goal_runs f ON f.goal_run_id=x.run_id JOIN goal_runs r ON r.id=x.run_id WHERE r.project_id=?1 OR json_extract(r.document,'$.root_grant_id')=?2").map_err(db_error)?;
    let rows = q
        .query_map(params![project, root], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(db_error)?;
    let mut result = Vec::new();
    for row in rows {
        let (attempt, run_id, node, doc) = row.map_err(db_error)?;
        let r: ReservationRecord = decode(&doc)?;
        let authority = checked_record(db, &run_id)?;
        let original =
            crate::factory::checked_child_goal_scope(db, &authority.origin.factory_scope_id)?;
        if r.run_id != run_id || r.node_id != node || r.shared_root_grant_id.is_some() {
            return Err(changed());
        }
        let reserved:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.kind')='goal.node_reserved' AND json_extract(document,'$.payload.attempt_id')=?2 AND json_extract(document,'$.payload.node_id')=?3 AND json_extract(document,'$.payload.plan_revision')=?4 AND json_extract(document,'$.payload.reservation')=json(?5)) AND NOT EXISTS(SELECT 1 FROM factory_budget_reservations WHERE attempt_id=?2)",params![run_id,attempt,node,sql_num(r.plan_revision)?,encode(&r.reservation)?],|r|r.get(0)).map_err(db_error)?;
        let current = run(db, &run_id)?;
        let history = bounded_plan_history(db, &current.scope_id)?;
        let plan = history
            .iter()
            .find(|p| p.proposal.revision == r.plan_revision)
            .ok_or_else(changed)?;
        let nodes = node_records_at(db, &current, r.plan_revision, &plan.digest, plan)?;
        let actual = nodes
            .iter()
            .find(|n| n.node.id == node && n.attempt_id.as_deref() == Some(attempt.as_str()))
            .ok_or_else(changed)?;
        if actual.node.reservation != r.reservation
            || actual.stop_confirmed != r.settled
            || actual.actual_runtime_ms != r.actual_runtime_ms
            || actual.actual_output_bytes != r.actual_output_bytes
            || actual.tokens != r.tokens
            || actual.cost_microunits != r.cost_microunits
        {
            return Err(err(
                "BUDGET_UNKNOWN",
                "Factory-derived accounting differs from its exact joined/unknown node evidence",
            ));
        }
        let session:Option<Option<String>>=db.query_row("SELECT session_id FROM goal_run_nodes WHERE run_id=?1 AND node_id=?2 AND plan_revision=?3 AND attempt_id=?4",params![run_id,node,sql_num(r.plan_revision)?,attempt],|r|r.get(0)).optional().map_err(db_error)?;
        if !reserved || session.is_none() {
            return Err(err(
                "BUDGET_UNKNOWN",
                "Factory-derived Goal reservation lacks its exact atomic node/attempt receipt",
            ));
        }
        result.push(crate::factory::FactoryReservation {
            authority_kind: "factory_child_goal".into(),
            scope_id: Some(original.id),
            run_id: Some(authority.origin.factory_run_id),
            root_task_id: original.root_task_id,
            root_grant_id: original.root_grant_id,
            project_id: original.project_id,
            task_id: authority.initial_run.task_id,
            session_id: session.flatten().unwrap_or_default(),
            attempt_id: attempt,
            ancestors: authority.origin.ancestors,
            queued_ms: 0,
            reservation: r.reservation,
            settled: r.settled,
            actual_runtime_ms: r.actual_runtime_ms,
            actual_output_bytes: r.actual_output_bytes,
            measurement_scope: "graph_selected_artifacts_plus_retained_validator_streams".into(),
            filesystem_total_bytes: None,
            captured_provider_message_bytes: None,
            provider_message_capture_complete: false,
        });
    }
    Ok(result)
}
pub(super) fn reserve_ancestors(
    db: &Connection,
    r: &GoalRun,
    reservation: &AttemptReservation,
) -> Result<()> {
    if !marked_run(db, &r.id)? {
        return Ok(());
    }
    let authority = checked_record(db, &r.id)?;
    let scope = crate::factory::checked_child_goal_scope(db, &authority.origin.factory_scope_id)?;
    let all = crate::factory::budget_records(db, &scope.root_grant_id, &scope.project_id)?;
    for (ancestor, budget) in crate::factory::child_goal_ancestor_budgets(db, &scope, &r.task_id)? {
        let relevant = all
            .iter()
            .filter(|x| {
                x.scope_id.as_deref() == Some(scope.id.as_str()) && x.ancestors.contains(&ancestor)
            })
            .collect::<Vec<_>>();
        if relevant.len() >= budget.attempts as usize
            || relevant.iter().filter(|x| !x.settled).count() >= budget.parallel as usize
        {
            return Err(limited());
        }
        let mut runtime = reservation.runtime_ms;
        let mut output = reservation.output_bytes;
        for previous in relevant {
            runtime = runtime
                .checked_add(if previous.settled {
                    previous.actual_runtime_ms.ok_or_else(|| {
                        err("BUDGET_UNKNOWN", "Factory ancestor runtime is unknown")
                    })?
                } else {
                    previous.reservation.runtime_ms
                })
                .ok_or_else(limited)?;
            output = output
                .checked_add(if previous.settled {
                    previous.actual_output_bytes.ok_or_else(|| {
                        err("BUDGET_UNKNOWN", "Factory ancestor output is unknown")
                    })?
                } else {
                    previous.reservation.output_bytes
                })
                .ok_or_else(limited)?;
        }
        if runtime > budget.runtime_ms || output > budget.output_bytes {
            return Err(limited());
        }
    }
    Ok(())
}
