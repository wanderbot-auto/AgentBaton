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
