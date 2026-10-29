//! Explicit Factory-derived Graph authority. Planning is never execution consent.
use super::*;

pub(super) const AUTHORITY: &str = "factory_child_goal_execution_v1";

pub(crate) fn policy_limits(
    selection: &FactoryScopeSelection,
    policy: &FactoryChildGoalExecutionPolicy,
    grant: &crate::policy::ExecutionGrant,
) -> Result<()> {
    let planning = selection
        .child_planning_policy
        .as_ref()
        .ok_or_else(denied)?;
    if !selection.capabilities.child_goal_execution
        || !selection.capabilities.child_planning
        || selection.capabilities.create_workspace
        || !policy.require_owner_acceptance
        || policy.max_nodes == 0
        || policy.max_nodes > planning.max_nodes
        || policy.max_nodes > 256
        || grant
            .scope
            .max_children
            .checked_add(1)
            .is_none_or(|v| policy.max_nodes > v)
        || policy.max_runs_per_task != 1
        || policy.max_total_runs == 0
        || policy.max_total_runs > selection.max_total_nodes
        || policy.max_runs_per_task > policy.max_total_runs
        || !policy.root_budget.narrower(&selection.root_budget)
        || !policy.project_budget.narrower(&selection.project_budget)
        || !policy.root_budget.narrower(&policy.project_budget)
        || policy.root_budget.attempts == 0
        || policy.root_budget.parallel == 0
        || policy.root_budget.runtime_ms == 0
        || policy.root_budget.output_bytes == 0
        || policy.root_budget.children > grant.scope.max_children
        || policy.root_budget.attempts > grant.scope.max_attempts
        || policy.root_budget.parallel > grant.scope.max_concurrency
        || policy.root_budget.runtime_ms > grant.scope.max_duration_ms
        || policy.root_budget.tokens.is_some()
        || policy.root_budget.cost_microunits.is_some()
        || policy.project_budget.tokens.is_some()
        || policy.project_budget.cost_microunits.is_some()
    {
        return Err(CoreError::new("AUTH_REQUIRED", "Explicit child Goal policy must fit the original Grant, reviewed planning contract and cumulative Factory budgets; planning consent never enlarges children/depth/attempt limits"));
    }
    Ok(())
}

pub(crate) fn resolve_binding(
    selection: &FactoryScopeSelection,
    grant: &crate::policy::ExecutionGrant,
    planning: Option<&FactoryChildPlanningBinding>,
) -> Result<Option<FactoryChildGoalExecutionBinding>> {
    let Some(policy) = &selection.child_goal_execution_policy else {
        if selection.capabilities.child_goal_execution {
            return Err(denied());
        }
        return Ok(None);
    };
    policy_limits(selection, policy, grant)?;
    let planning = planning.ok_or_else(denied)?;
    let mut binding = FactoryChildGoalExecutionBinding {
        schema_version: 1,
        authority_kind: AUTHORITY.into(),
        policy: policy.clone(),
        planning_policy_digest: planning.definition_digest.clone(),
        root_grant_id: grant.id.clone(),
        root_grant_revision: grant.revision,
        original_max_children: grant.scope.max_children,
        original_max_depth: grant.scope.max_depth,
        graph_contract_version: 1,
        definition_digest: String::new(),
        new_workspace_authorized: false,
    };
    let mut value = json!(binding);
    value
        .as_object_mut()
        .ok_or_else(changed)?
        .remove("definition_digest");
    binding.definition_digest = sha(&value)?;
    Ok(Some(binding))
}

pub(super) fn approved_binding(scope: &FactoryScope) -> Result<&FactoryChildGoalExecutionBinding> {
    let expected = resolve_binding(
        &scope.selection,
        &scope.bindings.approved_grant,
        scope.bindings.child_planning.as_ref(),
    )?
    .ok_or_else(denied)?;
    let actual = scope
        .bindings
        .child_goal_execution
        .as_ref()
        .ok_or_else(denied)?;
    if json!(expected) != json!(actual) {
        return Err(changed());
    }
    Ok(actual)
}

/// Only the misleading identity-as-proposal FK is removed. Existing native
/// rows retain every byte and continue through their original native verifier.
/// Incoming FKs keep their original table names; FK checks run before commit.
pub(crate) fn migrate(db: &mut Connection, version: u32) -> Result<()> {
    if version >= 15 {
        return Ok(());
    }
    let current: u32 = db
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(db_error)?;
    if current != 14 {
        return Err(changed());
    }
    crate::goal_store::verify_native_goal_history_before_factory_migration(db)?;
    let foreign_keys: bool = db
        .pragma_query_value(None, "foreign_keys", |row| row.get(0))
        .map_err(db_error)?;
    db.pragma_update(None, "foreign_keys", false)
        .map_err(db_error)?;
    let result = (|| {
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let invalid: bool = tx
            .prepare("PRAGMA foreign_key_check")
            .map_err(db_error)?
            .exists([])
            .map_err(db_error)?;
        if invalid {
            return Err(changed());
        }
        tx.execute_batch("CREATE TABLE goal_scopes_v15(id TEXT PRIMARY KEY,task_id TEXT NOT NULL REFERENCES tasks(id),root_grant_id TEXT NOT NULL REFERENCES execution_grants(id),revision INTEGER NOT NULL CHECK(revision>0),revoked INTEGER NOT NULL CHECK(revoked IN(0,1)),document TEXT NOT NULL);
INSERT INTO goal_scopes_v15 SELECT * FROM goal_scopes;
CREATE TABLE goal_runs_v15(id TEXT PRIMARY KEY,scope_id TEXT NOT NULL REFERENCES goal_scopes(id),project_id TEXT NOT NULL REFERENCES projects(id),task_id TEXT NOT NULL REFERENCES tasks(id),revision INTEGER NOT NULL CHECK(revision>0),state TEXT NOT NULL,document TEXT NOT NULL);
INSERT INTO goal_runs_v15 SELECT * FROM goal_runs;
DROP TABLE goal_runs;
DROP TABLE goal_scopes;
ALTER TABLE goal_scopes_v15 RENAME TO goal_scopes;
ALTER TABLE goal_runs_v15 RENAME TO goal_runs;
CREATE UNIQUE INDEX goal_one_active_scope_per_task ON goal_scopes(task_id) WHERE revoked=0;
CREATE UNIQUE INDEX goal_one_running_scope ON goal_runs(scope_id) WHERE state IN('queued','running','cancel_requested','outcome_unknown');
CREATE TABLE factory_child_goal_runs(goal_run_id TEXT PRIMARY KEY REFERENCES goal_runs(id),goal_scope_id TEXT NOT NULL UNIQUE REFERENCES goal_scopes(id),factory_scope_id TEXT NOT NULL REFERENCES factory_scopes(id),factory_run_id TEXT NOT NULL REFERENCES factory_runs(id),task_id TEXT NOT NULL REFERENCES tasks(id),root_task_id TEXT NOT NULL REFERENCES tasks(id),parent_task_id TEXT REFERENCES tasks(id),plan_revision_id TEXT NOT NULL REFERENCES factory_child_plan_revisions(id),node_count INTEGER NOT NULL CHECK(node_count>0 AND node_count<=256),receipt_sequence INTEGER NOT NULL UNIQUE REFERENCES events(sequence),record_digest TEXT NOT NULL CHECK(length(record_digest)=64),document TEXT NOT NULL CHECK(octet_length(document)<=1048576));
CREATE INDEX factory_child_goal_tasks ON factory_child_goal_runs(factory_scope_id,task_id);
PRAGMA user_version=15;").map_err(db_error)?;
        let invalid: bool = tx
            .prepare("PRAGMA foreign_key_check")
            .map_err(db_error)?
            .exists([])
            .map_err(db_error)?;
        if invalid {
            return Err(changed());
        }
        tx.commit().map_err(db_error)
    })();
    let restore = db
        .pragma_update(None, "foreign_keys", foreign_keys)
        .map_err(db_error);
    match (result, restore) {
        (Err(error), _) => Err(error),
        (Ok(()), result) => result,
    }
}

/// Private host proof: never Deserialize, never accepted from renderer/Agent JSON.
pub(crate) struct FactoryGoalSource {
    pub(crate) scope: FactoryScope,
    pub(crate) factory_run: FactoryRun,
    pub(crate) task: crate::storage::Task,
    pub(crate) parent_task_id: Option<String>,
    pub(crate) ancestors: Vec<String>,
    pub(crate) request: FactoryDelegateRequest,
    pub(crate) plans: Vec<FactoryChildPlanView>,
    pub(crate) host_policy: crate::goal_plan::HostGoalPolicy,
}

pub(crate) fn checked_scope(db: &Connection, id: &str) -> Result<FactoryScope> {
    let scope = scope(db, id)?;
    approved_binding(&scope)?;
    Ok(scope)
}
pub(crate) fn live_scope(
    db: &Connection,
    scope_id: &str,
    task_id: &str,
    input: Option<&crate::data_workspaces::DataInputWitness>,
) -> Result<()> {
    let scope = checked_scope(db, scope_id)?;
    live_with_input(db, &scope, input)?;
    let run = run(db, scope_id)?;
    let target = binding(db, &run.id, task_id)?;
    if run.cancel_requested
        || matches!(
            run.state.as_str(),
            "outcome_unknown" | "cancelled" | "blocked" | "completed"
        )
        || target.handle.cancel_requested
        || target.scope_id != scope_id
    {
        return Err(denied());
    }
    Ok(())
}
impl Store {
    pub(crate) fn prepare_factory_goal_source(
        &self,
        scope_id: &str,
        request: &FactoryChildGoalSubmission,
        scope_revision: u64,
    ) -> Result<FactoryGoalSource> {
        let scope = checked_scope(&self.conn, scope_id)?;
        let execution = approved_binding(&scope)?;
        let current_run = run(&self.conn, scope_id)?;
        if scope.revision != scope_revision
            || current_run.revision != request.expected_factory_run_revision
        {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Factory scope/run changed before child Goal admission",
            ));
        }
        let target = binding(&self.conn, &current_run.id, &request.task_id)?;
        child_plan::mutable_target(&self.conn, &scope, &target)?;
        self.require_factory_child_plan_observation(&scope)?;
        let query = FactoryChildPlanQuery {
            scope_id: scope_id.into(),
            task_id: request.task_id.clone(),
            plan_revision: None,
        };
        let head = self.get_factory_child_plan(&query)?;
        if head.reference != request.plan
            || request.plan.scope_id != scope_id
            || request.plan.run_id != current_run.id
            || request.plan.task_id != request.task_id
        {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Execute only the exact currently saved child plan reference",
            ));
        }
        if head.proposal.nodes.len() > execution.policy.max_nodes as usize {
            return Err(limited());
        }
        let lineage = lineage(&self.conn, &request.task_id)?;
        let ancestors = ancestry(&self.conn, &request.task_id)?;
        let mut host_policy = child_plan::host_policy(&scope, &target)?;
        host_policy.max_nodes = execution.policy.max_nodes;
        host_policy.max_depth = scope
            .bindings
            .approved_grant
            .scope
            .max_depth
            .checked_sub(lineage.depth)
            .ok_or_else(denied)?;
        // Retain the fixed target's own ceiling as well as shared root/project limits.
        host_policy.root_budget =
            intersect_budget(&target.request.budget, &execution.policy.root_budget);
        host_policy.project_budget = execution.policy.project_budget.clone();
        let refs = self.list_factory_child_plans(&query)?;
        let mut plans = Vec::with_capacity(refs.len());
        for reference in refs {
            if reference.revision > head.reference.revision {
                return Err(changed());
            }
            let plan = self.get_factory_child_plan(&FactoryChildPlanQuery {
                scope_id: scope_id.into(),
                task_id: request.task_id.clone(),
                plan_revision: Some(reference.revision),
            })?;
            if plan.reference != reference {
                return Err(changed());
            }
            plans.push(plan);
        }
        if plans.last().map(|p| &p.reference) != Some(&request.plan) {
            return Err(changed());
        }
        let (runs, own):(u32,u32)=self.conn.query_row("SELECT count(*),COALESCE(sum(task_id=?2),0) FROM factory_child_goal_runs WHERE factory_scope_id=?1",params![scope_id,request.task_id],|row|Ok((row.get(0)?,row.get(1)?))).map_err(db_error)?;
        if runs >= execution.policy.max_total_runs || own >= execution.policy.max_runs_per_task {
            return Err(limited());
        }
        check_child_capacity(
            &self.conn,
            &scope,
            &request.task_id,
            head.proposal.nodes.len().saturating_sub(1) as u32,
        )?;
        let task = Self::factory_child_read_task(&self.conn, &request.task_id)?;
        Ok(FactoryGoalSource {
            scope,
            factory_run: current_run,
            task,
            parent_task_id: lineage.parent_task_id,
            ancestors,
            request: target.request,
            plans,
            host_policy,
        })
    }
}

/// Count all already-created Factory children, original delegated Grants and
/// nested Graph nodes together; repeated nested scopes do not reset capacity.
fn check_child_capacity(
    db: &Connection,
    scope: &FactoryScope,
    task_id: &str,
    requested: u32,
) -> Result<()> {
    let root_factory:u32=db.query_row("SELECT count(*) FROM factory_task_lineage l JOIN factory_scopes s ON s.id=l.scope_id WHERE s.root_grant_id=?1 AND l.parent_task_id IS NOT NULL",[&scope.root_grant_id],|r|r.get(0)).map_err(db_error)?;
    let project_factory:u32=db.query_row("SELECT count(*) FROM factory_task_lineage l JOIN factory_scopes s ON s.id=l.scope_id WHERE s.project_id=?1 AND l.parent_task_id IS NOT NULL",[&scope.project_id],|r|r.get(0)).map_err(db_error)?;
    let root_grants:u32=db.query_row("SELECT count(*) FROM execution_grants WHERE parent_id IS NOT NULL AND json_extract(document,'$.root_grant_id')=?1",[&scope.root_grant_id],|r|r.get(0)).map_err(db_error)?;
    let project_grants: u32 = db
        .query_row(
            "SELECT count(*) FROM execution_grants WHERE parent_id IS NOT NULL AND project_id=?1",
            [&scope.project_id],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    let (root_graph,project_graph):(u32,u32)=db.query_row("SELECT COALESCE(sum(CASE WHEN g.root_grant_id=?1 THEN g.children ELSE 0 END),0),COALESCE(sum(CASE WHEN g.project_id=?2 THEN g.children ELSE 0 END),0) FROM (SELECT r.id,r.project_id,json_extract(r.document,'$.root_grant_id') root_grant_id,max(count(DISTINCT n.node_id)-1,0) children FROM goal_runs r JOIN goal_run_nodes n ON n.run_id=r.id GROUP BY r.id) g",params![scope.root_grant_id,scope.project_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
    let root = root_factory
        .checked_add(root_grants)
        .and_then(|n| n.checked_add(root_graph))
        .and_then(|n| n.checked_add(requested))
        .ok_or_else(limited)?;
    let project = project_factory
        .checked_add(project_grants)
        .and_then(|n| n.checked_add(project_graph))
        .and_then(|n| n.checked_add(requested))
        .ok_or_else(limited)?;
    let policy = approved_binding(scope)?;
    for (ancestor, budget) in ancestor_budgets(db, scope, task_id)? {
        let (created,nested):(u32,u32)=db.query_row("WITH RECURSIVE family(task_id,depth) AS (SELECT ?1,0 UNION ALL SELECT l.task_id,f.depth+1 FROM factory_task_lineage l JOIN family f ON l.parent_task_id=f.task_id WHERE l.scope_id=?2 AND f.depth<32) SELECT max((SELECT count(DISTINCT task_id)-1 FROM family),0),COALESCE((SELECT sum(children) FROM (SELECT r.id,max(count(DISTINCT n.node_id)-1,0) children FROM goal_runs r JOIN goal_run_nodes n ON n.run_id=r.id WHERE r.task_id IN (SELECT task_id FROM family) GROUP BY r.id)),0)",params![ancestor,scope.id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
        if created
            .checked_add(nested)
            .and_then(|n| n.checked_add(requested))
            .is_none_or(|n| n > budget.children)
        {
            return Err(limited());
        }
    }
    if root > scope.bindings.approved_grant.scope.max_children
        || root > policy.policy.root_budget.children
        || project > policy.policy.project_budget.children
        || root > scope.selection.root_budget.children
        || project > scope.selection.project_budget.children
    {
        return Err(CoreError::new("RESOURCE_LIMIT", "Original shared root/project child capacity is exhausted; a nested Graph cannot reset it"));
    }
    Ok(())
}

pub(crate) fn checked_plan(
    db: &Connection,
    reference: &FactoryChildPlanRef,
) -> Result<FactoryChildPlanView> {
    let plan = child_plan::row(
        db,
        &reference.scope_id,
        &reference.task_id,
        reference.revision,
    )?;
    if plan.reference != *reference {
        return Err(changed());
    }
    Ok(plan)
}
pub(crate) fn verify_lineage(
    db: &Connection,
    s: &FactoryScope,
    task_id: &str,
    ancestors: &[String],
    parent: Option<&str>,
) -> Result<()> {
    let l = lineage(db, task_id)?;
    let r = run(db, &s.id)?;
    let b = binding(db, &r.id, task_id)?;
    if l.factory_scope_id != s.id
        || l.root_task_id != s.root_task_id
        || l.parent_task_id.as_deref() != parent
        || ancestry(db, task_id)? != ancestors
        || b.scope_id != s.id
        || b.run_id != r.id
        || b.grant_id != s.root_grant_id
    {
        return Err(changed());
    }
    Ok(())
}
pub(crate) fn policy_live(db: &Connection, scope_id: &str, task_id: &str) -> Result<()> {
    let s = checked_scope(db, scope_id)?;
    base_live(db, &s)?;
    let r = run(db, scope_id)?;
    let b = binding(db, &r.id, task_id)?;
    if r.cancel_requested
        || matches!(
            r.state.as_str(),
            "cancelled" | "outcome_unknown" | "blocked" | "completed"
        )
        || b.handle.cancel_requested
    {
        return Err(denied());
    }
    let task = Store::factory_child_read_task(db, task_id)?;
    if task.project_id != s.project_id
        || b.planning_spec_revision != Some(task.spec_revision)
        || task.goal != b.request.goal
        || matches!(
            task.state,
            TaskState::CancelRequested | TaskState::Cancelled | TaskState::Accepted
        )
    {
        return Err(changed());
    }
    child_plan::require_planning_task_state(db, &s, &b, &task)
}

fn intersect_budget(
    a: &crate::goal_plan::BudgetLimits,
    b: &crate::goal_plan::BudgetLimits,
) -> crate::goal_plan::BudgetLimits {
    crate::goal_plan::BudgetLimits {
        attempts: a.attempts.min(b.attempts),
        parallel: a.parallel.min(b.parallel),
        runtime_ms: a.runtime_ms.min(b.runtime_ms),
        output_bytes: a.output_bytes.min(b.output_bytes),
        children: a.children.min(b.children),
        reworks: a.reworks.min(b.reworks),
        tokens: None,
        cost_microunits: None,
    }
}
pub(crate) fn source_is_current(db: &Connection, source: &FactoryGoalSource) -> Result<()> {
    let scope = checked_scope(db, &source.scope.id)?;
    let current_run = run(db, &scope.id)?;
    let target = binding(db, &current_run.id, &source.task.id)?;
    child_plan::mutable_target(db, &scope, &target)?;
    if scope.revision != source.scope.revision
        || scope.contract_digest != source.scope.contract_digest
        || current_run.revision != source.factory_run.revision
        || encode(&target.request)? != encode(&source.request)?
        || ancestry(db, &source.task.id)? != source.ancestors
        || lineage(db, &source.task.id)?.parent_task_id != source.parent_task_id
    {
        return Err(changed());
    }
    let latest =
        child_plan::latest_reference(db, &scope.id, &source.task.id)?.ok_or_else(changed)?;
    if source.plans.last().map(|p| &p.reference) != Some(&latest) {
        return Err(CoreError::new(
            "STALE_REVISION",
            "Saved child plan head changed before execution commit",
        ));
    }
    let binding = approved_binding(&scope)?;
    let (runs,own):(u32,u32)=db.query_row("SELECT count(*),COALESCE(sum(task_id=?2),0) FROM factory_child_goal_runs WHERE factory_scope_id=?1",params![scope.id,source.task.id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
    if runs >= binding.policy.max_total_runs || own >= binding.policy.max_runs_per_task {
        return Err(limited());
    }
    check_child_capacity(
        db,
        &scope,
        &source.task.id,
        source
            .plans
            .last()
            .ok_or_else(changed)?
            .proposal
            .nodes
            .len()
            .saturating_sub(1) as u32,
    )
}

pub(crate) fn ancestor_budgets(
    db: &Connection,
    scope: &FactoryScope,
    task_id: &str,
) -> Result<Vec<(String, crate::goal_plan::BudgetLimits)>> {
    let r = run(db, &scope.id)?;
    ancestry(db, task_id)?
        .into_iter()
        .map(|id| {
            let b = binding(db, &r.id, &id)?;
            Ok((id, b.request.budget))
        })
        .collect()
}

pub(crate) fn verify_target_request(
    db: &Connection,
    scope: &FactoryScope,
    task_id: &str,
    expected: &FactoryDelegateRequest,
) -> Result<()> {
    let r = run(db, &scope.id)?;
    let b = binding(db, &r.id, task_id)?;
    if encode(&b.request)? != encode(expected)?
        || b.scope_id != scope.id
        || b.grant_id != scope.root_grant_id
    {
        return Err(changed());
    }
    Ok(())
}

/// Called inside the same transaction as the Graph provenance. The current
/// Factory process retains its real lease/reservation until its normal owned
/// stop; this marker merely prevents the other scheduler launching this target.
pub(crate) fn mark_queued(
    db: &Connection,
    source: &FactoryGoalSource,
    goal_run_id: &str,
) -> Result<()> {
    let mut r = run(db, &source.scope.id)?;
    let mut b = binding(db, &r.id, &source.task.id)?;
    if b.child_goal_run_id.is_some() {
        return Err(CoreError::new(
            "CONFIG_CONFLICT",
            "The target already has a child Goal; adopt its saved plan on the same run",
        ));
    }
    b.child_goal_run_id = Some(goal_run_id.into());
    if b.session_id.is_none() {
        b.handle.state = "waiting_goal".into();
    }
    b.handle.revision = b.handle.revision.checked_add(1).ok_or_else(limited)?;
    save_binding(db, &mut b)?;
    if r.active_session_id.is_none() {
        r.state = "waiting_child".into();
    }
    save_run(
        db,
        &mut r,
        "factory.child_goal_queued",
        json!({"task_id":source.task.id,"goal_run_id":goal_run_id,"provider_started":false,"stop_confirmed":false,"task_accepted":false}),
    )
}

/// Reconstruct the host policy from real native Factory records and immutable
/// target request. Persisted JSON is compared to this value, never deserialized
/// into a host/approved policy type.
pub(crate) fn expected_policy(
    db: &Connection,
    scope_id: &str,
    task_id: &str,
) -> Result<crate::goal_plan::HostGoalPolicy> {
    let scope = checked_scope(db, scope_id)?;
    let execution = approved_binding(&scope)?;
    let r = run(db, scope_id)?;
    let target = binding(db, &r.id, task_id)?;
    let lineage = lineage(db, task_id)?;
    let mut policy = child_plan::host_policy(&scope, &target)?;
    policy.max_nodes = execution.policy.max_nodes;
    policy.max_depth = scope
        .bindings
        .approved_grant
        .scope
        .max_depth
        .checked_sub(lineage.depth)
        .ok_or_else(denied)?;
    policy.root_budget = intersect_budget(&target.request.budget, &execution.policy.root_budget);
    policy.project_budget = execution.policy.project_budget.clone();
    Ok(policy)
}
