// Existing real SQLite/Native Factory fixture; these tests invoke the Core policy
// resolver only. No child Graph/Provider/MCP worker or OS dialog is claimed.
use super::*;
fn selected(scope: &FactoryScope) -> FactoryScopeSelection {
    let mut s = scope.selection.clone();
    let mut budget = s.root_budget.clone();
    budget.runtime_ms = budget
        .runtime_ms
        .min(scope.bindings.approved_grant.scope.max_duration_ms);
    budget.children = 0;
    s.capabilities.child_goal_execution = true;
    s.child_goal_execution_policy = Some(FactoryChildGoalExecutionPolicy {
        max_nodes: 1,
        max_runs_per_task: 1,
        max_total_runs: 1,
        root_budget: budget,
        project_budget: s.project_budget.clone(),
        require_owner_acceptance: true,
    });
    s
}
fn resolve(
    s: &FactoryScopeSelection,
    scope: &FactoryScope,
) -> crate::Result<Option<FactoryChildGoalExecutionBinding>> {
    crate::factory::runtime::resolve_child_goal_binding(
        s,
        &scope.bindings.approved_grant,
        scope.bindings.child_planning.as_ref(),
    )
}
#[test]
fn old_planning_native_scope_never_upgrades_and_keeps_omitted_execution_fields() {
    let (f, scope) = fixture();
    let before = json!(f.store.get_factory_scope(&scope.id).unwrap());
    assert!(!scope.selection.capabilities.child_goal_execution);
    assert!(before["selection"]
        .get("child_goal_execution_policy")
        .is_none());
    assert!(before["bindings"].get("child_goal_execution").is_none());
    assert!(resolve(&scope.selection, &scope).unwrap().is_none());
    let mut old = scope.selection.clone();
    old.capabilities.child_goal_execution = true;
    assert!(resolve(&old, &scope).is_err());
    assert_eq!(json!(f.store.get_factory_scope(&scope.id).unwrap()), before);
    assert_eq!(
        f.store
            .get_grant(&scope.root_grant_id)
            .unwrap()
            .scope
            .max_children,
        0
    );
}
#[test]
fn derived_execution_policy_cannot_expand_children_zero_or_invent_more_runs() {
    let (f, scope) = fixture();
    let before = json!(f.store.get_grant(&scope.root_grant_id).unwrap());
    let mut s = selected(&scope);
    let b = resolve(&s, &scope).unwrap().unwrap();
    assert_eq!(b.root_grant_id, scope.root_grant_id);
    assert_eq!(b.root_grant_revision, scope.root_grant_revision);
    assert_eq!(b.original_max_children, 0);
    assert!(!b.new_workspace_authorized);
    s.child_goal_execution_policy.as_mut().unwrap().max_nodes = 2;
    assert_eq!(resolve(&s, &scope).unwrap_err().code, "AUTH_REQUIRED");
    s = selected(&scope);
    s.child_goal_execution_policy
        .as_mut()
        .unwrap()
        .max_runs_per_task = 2;
    assert_eq!(resolve(&s, &scope).unwrap_err().code, "AUTH_REQUIRED");
    assert_eq!(
        json!(f.store.get_grant(&scope.root_grant_id).unwrap()),
        before
    );
}
#[test]
fn derived_execution_policy_requires_exact_planning_and_separate_execution_consent() {
    let (_, scope) = fixture();
    let mut s = selected(&scope);
    s.capabilities.child_planning = false;
    assert!(resolve(&s, &scope).is_err());
    s = selected(&scope);
    s.capabilities.child_goal_execution = false;
    assert!(resolve(&s, &scope).is_err());
    s = selected(&scope);
    s.capabilities.create_workspace = true;
    assert!(resolve(&s, &scope).is_err());
    s = selected(&scope);
    s.child_goal_execution_policy
        .as_mut()
        .unwrap()
        .require_owner_acceptance = false;
    assert!(resolve(&s, &scope).is_err());
    s = selected(&scope);
    s.child_goal_execution_policy
        .as_mut()
        .unwrap()
        .max_total_runs = s.max_total_nodes + 1;
    assert!(resolve(&s, &scope).is_err());
}
#[test]
fn derived_execution_policy_cannot_reset_shared_budget_or_claim_unknown_meter() {
    let (_, scope) = fixture();
    let mut s = selected(&scope);
    s.child_goal_execution_policy
        .as_mut()
        .unwrap()
        .root_budget
        .attempts = s.root_budget.attempts + 1;
    assert!(resolve(&s, &scope).is_err());
    s = selected(&scope);
    s.child_goal_execution_policy
        .as_mut()
        .unwrap()
        .project_budget
        .output_bytes = s.project_budget.output_bytes + 1;
    assert!(resolve(&s, &scope).is_err());
    s = selected(&scope);
    s.child_goal_execution_policy
        .as_mut()
        .unwrap()
        .root_budget
        .tokens = Some(1);
    assert!(resolve(&s, &scope).is_err());
    s = selected(&scope);
    s.child_goal_execution_policy
        .as_mut()
        .unwrap()
        .project_budget
        .cost_microunits = Some(1);
    assert!(resolve(&s, &scope).is_err());
}

#[test]
fn recorded_schema14_factory_scope_roundtrips_without_new_authority_or_fresh_counts() {
    let original: serde_json::Value = serde_json::from_str(include_str!(
        "../tests/fixtures/factory_actual_parent_yield.json"
    ))
    .unwrap();
    let original_scope = original["scope"].clone();
    let scope: FactoryScope = serde_json::from_value(original_scope.clone()).unwrap();
    assert_eq!(json!(scope), original_scope);
    assert!(!scope.selection.capabilities.child_goal_execution);
    assert!(scope.selection.child_goal_execution_policy.is_none());
    assert!(scope.bindings.child_goal_execution.is_none());
    assert_eq!(
        crate::factory::store::sha(
            &json!({"schema_version":1,"selection":scope.selection,"bindings":scope.bindings})
        )
        .unwrap(),
        scope.contract_digest
    );
    // Counts are a separate fresh schema15 readonly response, never inserted
    // into the original FactoryScope approval/result serialization.
    assert!(!serde_json::to_string(&scope)
        .unwrap()
        .contains("child_goal_runs"));
}
