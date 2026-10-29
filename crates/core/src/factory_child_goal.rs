//! Explicit Factory-derived Graph authority. Planning is never execution consent.
use super::*;

pub(super) const AUTHORITY: &str = "factory_child_goal_execution_v1";

pub(super) fn policy_limits(
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
        || policy.max_runs_per_task == 0
        || policy.max_runs_per_task > 256
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

pub(super) fn resolve_binding(
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
