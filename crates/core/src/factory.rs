//! Independently owner-approved bounded delegation. Wire declarations are not authority.
use crate::goal_plan::BudgetLimits;
use crate::{
    policy::ExecutionGrant, registry::ManagedWorkspace, sessions::ProviderConnection,
    storage::Task, CoreError,
};
use agentbaton_workspaces::validation::ArtifactContractView;
use agentbaton_workspaces::validation::ArtifactRequirement;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactoryCapabilities {
    pub delegate_task: bool,
    pub resume_parent_as_new_attempt: bool,
    pub child_planning: bool,
    pub child_goal_execution: bool,
    pub create_workspace: bool,
}
/// Selection only. Core resolves every task/root/grant/input and companion binding.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactoryScopeSelection {
    pub root_task_id: String,
    pub expected_task_revision: u64,
    pub grant_id: String,
    pub expected_grant_revision: u64,
    pub connection_ids: Vec<String>,
    pub workspace_ids: Vec<String>,
    pub inputs: Vec<ArtifactRequirement>,
    pub outputs: Vec<ArtifactRequirement>,
    pub recipients: Vec<String>,
    pub max_depth: u32,
    pub max_children: u32,
    pub max_total_nodes: u32,
    pub root_budget: BudgetLimits,
    pub project_budget: BudgetLimits,
    pub ttl_ms: u64,
    pub capabilities: FactoryCapabilities,
    /// Only a new complete Native Factory approval may grant plan-only policy.
    /// Missing in schema13/old receipts; omission keeps their exact bytes stable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_planning_policy: Option<FactoryChildPlanningPolicy>,
    /// Separate explicit execution consent. Old planning-only receipts omit it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_goal_execution_policy: Option<FactoryChildGoalExecutionPolicy>,
}
/// Execution permission remains within the original immutable Grant ceilings.
/// It does not amend that Grant, create a workspace, or accept any candidate.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildGoalExecutionPolicy {
    pub max_nodes: u32,
    pub max_runs_per_task: u32,
    pub max_total_runs: u32,
    pub root_budget: BudgetLimits,
    pub project_budget: BudgetLimits,
    pub require_owner_acceptance: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildGoalExecutionBinding {
    pub schema_version: u32,
    pub authority_kind: String,
    pub policy: FactoryChildGoalExecutionPolicy,
    pub planning_policy_digest: String,
    pub root_grant_id: String,
    pub root_grant_revision: u64,
    pub original_max_children: u32,
    pub original_max_depth: u32,
    pub graph_contract_version: u32,
    pub definition_digest: String,
    pub new_workspace_authorized: bool,
}
/// A complete saved reference is compared to its immutable server row; IDs and
/// plan text from the caller never become execution authority.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildGoalSubmission {
    pub task_id: String,
    pub plan: FactoryChildPlanRef,
    pub expected_factory_run_revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildGoalStart {
    pub scope_id: String,
    pub request: FactoryChildGoalSubmission,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildGoalQuery {
    pub scope_id: String,
    pub task_id: String,
    #[serde(default)]
    pub goal_run_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildGoalCancel {
    pub scope_id: String,
    pub task_id: String,
    pub goal_run_id: String,
    pub expected_goal_run_revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildGoalAdvance {
    pub scope_id: String,
    pub goal_run_id: String,
    pub request: FactoryChildGoalSubmission,
    pub expected_goal_run_revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildGoalSummary {
    pub authority_kind: String,
    pub factory_scope_id: String,
    pub factory_run_id: String,
    pub factory_root_task_id: String,
    pub parent_task_id: Option<String>,
    pub task_id: String,
    pub plan: FactoryChildPlanRef,
    pub execution_policy_digest: String,
    pub goal: crate::goal_store::GraphRunSummary,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildGoalView {
    pub summary: FactoryChildGoalSummary,
    pub origin: FactoryChildGoalOriginView,
    pub details: crate::goal_store::GraphRunDetails,
}
/// Fixed scoped adoption request: caller identity and Factory scope are supplied
/// by the private credential, never by Agent parameters.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildGoalAdvanceSubmission {
    pub goal_run_id: String,
    pub request: FactoryChildGoalSubmission,
    pub expected_goal_run_revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildGoalOriginView {
    pub factory_scope_id: String,
    pub factory_scope_digest: String,
    pub factory_run_id: String,
    pub factory_root_task_id: String,
    pub parent_task_id: Option<String>,
    pub ancestors: Vec<String>,
    pub original_native_approval_id: String,
    pub root_grant_id: String,
    pub root_grant_revision: u64,
    pub initial_plan: FactoryChildPlanRef,
    pub execution_binding: FactoryChildGoalExecutionBinding,
}
/// Narrow selection within existing Factory artifacts/connections/workspaces.
/// This uses Goal's exact output/validator references, never a second plan DSL.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildPlanningPolicy {
    pub max_nodes: u32,
    pub max_revisions: u32,
    pub max_plan_bytes: u32,
    pub outputs: Vec<crate::goal_store::GoalOutputSelection>,
    pub writable_prefixes: Vec<String>,
    pub owner_acceptance_criteria: String,
}
/// Complete server-resolved definitions are reviewed in the Native snapshot.
/// They permit references in stored plans, not validator/Goal execution.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildPlanningBinding {
    pub schema_version: u32,
    pub authority_kind: String,
    pub policy: FactoryChildPlanningPolicy,
    pub owner_rule_id: String,
    pub input_rule_id: String,
    pub validator_definitions: std::collections::BTreeMap<String, serde_json::Value>,
    pub definition_digest: String,
    pub goal_execution_authorized: bool,
    pub new_workspace_authorized: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildPlanSubmission {
    pub task_id: String,
    /// Full original GoalPlanProposal UTF-8 text, not a parsed/normalized copy.
    pub plan_json: String,
    pub expected_plan_revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildPlanUpdate {
    pub scope_id: String,
    pub task_id: String,
    pub plan_json: String,
    pub expected_plan_revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildPlanQuery {
    pub scope_id: String,
    pub task_id: String,
    /// None selects the current head; list returns all bounded revision refs.
    #[serde(default)]
    pub plan_revision: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildPlanRef {
    pub id: String,
    pub scope_id: String,
    pub run_id: String,
    pub task_id: String,
    pub revision: u64,
    pub plan_digest: String,
    pub raw_sha256: String,
    pub receipt_sequence: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildPlanView {
    pub reference: FactoryChildPlanRef,
    pub proposal: crate::goal_plan::GoalPlanProposal,
    pub plan_json: String,
    pub authority_kind: String,
    pub scope_digest: String,
    pub planning_policy_digest: String,
    pub root_task_id: String,
    pub root_grant_id: String,
    pub root_grant_revision: u64,
    pub caller_task_id: String,
    pub caller_session_id: Option<String>,
    pub caller_attempt_id: Option<String>,
    pub caller_fencing_token: Option<u64>,
    pub target_spec_revision: u64,
    pub input_revision: String,
    pub created_at: String,
    pub planning_only: bool,
    pub dispatched: bool,
    pub task_accepted: bool,
}
/// Read-only server-derived editor context; not an ApprovedGoalScope or run token.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryChildPlanningContext {
    pub scope_id: String,
    pub run_id: String,
    pub scope_revision: u64,
    pub scope_digest: String,
    pub planning_policy_digest: String,
    pub goal: crate::goal_plan::GoalBinding,
    pub policy: FactoryChildPlanningPolicy,
    pub execution_bindings: Vec<crate::execution::ExecutionBinding>,
    pub edit_context: crate::goal_store::GoalPlanEditingContext,
    pub program_sha256: std::collections::BTreeMap<String, String>,
    pub inputs: std::collections::BTreeMap<String, crate::goal_store::GoalInputView>,
    pub outputs: std::collections::BTreeMap<String, String>,
    pub contracts: std::collections::BTreeMap<String, Vec<String>>,
    pub rules: std::collections::BTreeMap<String, String>,
    pub required_acceptance_rules: Vec<String>,
    pub budget: BudgetLimits,
    pub project_budget: BudgetLimits,
    pub current_plan_revision: u64,
    pub planning_only: bool,
    pub goal_execution_authorized: bool,
    pub new_workspace_authorized: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryScope {
    pub id: String,
    pub approval_id: String,
    pub project_id: String,
    pub root_task_id: String,
    pub root_grant_id: String,
    pub root_grant_revision: u64,
    pub revision: u64,
    pub revoked: bool,
    pub expires_at_ms: i64,
    pub selection: FactoryScopeSelection,
    /// Server-resolved connections/workspaces, actual input hashes, companion,
    /// requested/effective/enforced guarantees and the original grant snapshot.
    pub bindings: FactoryScopeBindings,
    pub contract_digest: String,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryTaskLineage {
    pub task_id: String,
    pub root_task_id: String,
    pub parent_task_id: Option<String>,
    pub factory_scope_id: String,
    pub delegation_id: Option<String>,
    pub depth: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryDelegateRequest {
    pub goal: String,
    pub connection_id: String,
    pub workspace_id: String,
    pub input_artifact_ids: Vec<String>,
    pub output_artifact_ids: Vec<String>,
    pub budget: BudgetLimits,
    pub wait_for_child: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryTaskRequest {
    pub task_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryTaskHandle {
    pub task_id: String,
    pub parent_task_id: Option<String>,
    pub root_task_id: String,
    pub delegation_id: String,
    pub factory_scope_id: String,
    pub revision: u64,
    pub state: String,
    pub attempt_id: Option<String>,
    pub cancel_requested: bool,
    pub confirmed_stop: bool,
    pub result_available: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryArtifactReference {
    pub artifact_id: String,
    pub attempt_id: String,
    pub relative_path: std::path::PathBuf,
    pub sha256: String,
    pub bytes: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryTaskResult {
    pub task: FactoryTaskHandle,
    pub artifacts: Vec<FactoryArtifactReference>,
    pub validation_refs: Vec<String>,
    pub acceptance_state: String,
    pub context_continuation: String,
    pub handoff_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_plan: Option<FactoryChildPlanRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryInputBinding {
    pub contract: ArtifactContractView,
    pub sha256: String,
    pub bytes: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactoryCompanionBinding {
    pub executable: std::path::PathBuf,
    pub executable_sha256: String,
    pub argument_template: Vec<String>,
    pub credential_transport: String,
    pub global_daemon_token_available: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactoryGuarantees {
    pub filesystem_sandbox: bool,
    pub network_sandbox: bool,
    pub account_isolation: bool,
    pub output_quota: bool,
    pub continuous_input_immutability: bool,
    pub complete_process_tree_containment: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryScopeBindings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_planning: Option<FactoryChildPlanningBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_goal_execution: Option<FactoryChildGoalExecutionBinding>,
    pub project: crate::registry::Project,
    pub root_task: Task,
    pub approved_grant: ExecutionGrant,
    pub connections: Vec<ProviderConnection>,
    pub workspaces: Vec<ManagedWorkspace>,
    pub inputs: Vec<FactoryInputBinding>,
    pub outputs: Vec<ArtifactContractView>,
    pub companion: FactoryCompanionBinding,
    pub guarantees: FactoryGuarantees,
    pub start_contract: FactoryStartContract,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactoryStartContract {
    pub mode: String,
    pub root_task_request_digest: String,
    pub context_continuation: String,
    pub implicit_start_on_approval: bool,
    pub new_workspace_authority: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryRun {
    pub id: String,
    pub scope_id: String,
    pub project_id: String,
    pub root_task_id: String,
    pub revision: u64,
    pub state: String,
    pub active_task_id: Option<String>,
    pub active_session_id: Option<String>,
    pub cancel_requested: bool,
    pub context_continuation: String,
    pub last_error: Option<CoreError>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryRunDetails {
    pub run: FactoryRun,
    pub tasks: Vec<FactoryTaskHandle>,
    pub waiting_relations: Vec<FactoryWaitingRelation>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryWaitingRelation {
    pub parent_task_id: String,
    pub child_task_id: String,
    pub parent_session_id: String,
    pub parent_stop_confirmed: bool,
    pub handoff_digest: Option<String>,
}

#[path = "factory_store.rs"]
mod store;

pub(crate) use store::migrate;
pub(crate) fn companion_path() -> crate::Result<std::path::PathBuf> {
    let exe = std::env::current_exe().map_err(|_| {
        CoreError::new(
            "CAPABILITY_UNSUPPORTED",
            "Baton executable location is unavailable",
        )
    })?;
    let parent = exe.parent().ok_or_else(|| {
        CoreError::new(
            "CAPABILITY_UNSUPPORTED",
            "Baton installation root is unavailable",
        )
    })?;
    let base = if parent.file_name().is_some_and(|name| name == "deps") {
        parent.parent().unwrap_or(parent)
    } else {
        parent
    };
    base.join("baton").canonicalize().map_err(|_| {
        CoreError::new(
            "CAPABILITY_UNSUPPORTED",
            "Install the matching Baton MCP companion beside batond",
        )
    })
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactoryRecordCounts {
    pub child_goal_runs: u64,
    pub child_plan_heads: u64,
    pub child_plan_revisions: u64,
    pub scopes: u64,
    pub runs: u64,
    pub task_lineage: u64,
    pub task_bindings: u64,
    pub delegations: u64,
    pub attempts: u64,
    pub scoped_handles: u64,
    pub handoffs: u64,
    pub budget_reservations: u64,
}

#[path = "factory_runtime.rs"]
mod runtime;
pub(crate) use runtime::{
    is_factory_task, owns_session, permits_factory_dispatch, reserve_factory_session,
    task_budget_root, FactoryAction, FactoryDispatchContext, FactoryDispatchTicket,
    FactoryMcpLaunch,
};

#[path = "factory_budget.rs"]
mod budget;
pub(crate) use budget::{budget_records, FactoryReservation};

#[path = "factory_capture.rs"]
mod capture;
pub(crate) use capture::{FactoryCapture, FactoryCapturePlan};

#[path = "factory_reads.rs"]
mod reads;
pub(crate) use reads::{FactoryReadControl, FactoryReadPlan, FactoryReadWatch, FactoryReadWitness};

pub(crate) use runtime::{checked_child_goal_scope, live_child_goal_scope, FactoryGoalSource};

pub(crate) use runtime::{
    checked_child_goal_plan, child_goal_policy_live, verify_child_goal_lineage,
};

pub(crate) use runtime::factory_goal_source_is_current;

pub(crate) use runtime::child_goal_ancestor_budgets;

pub(crate) use runtime::verify_child_goal_target_request;

pub(crate) use runtime::mark_child_goal_queued;
