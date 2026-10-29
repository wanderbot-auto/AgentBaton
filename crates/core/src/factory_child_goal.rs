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
