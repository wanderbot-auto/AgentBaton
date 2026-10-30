//! Independent native-approved local branch integration. Serializable records
//! are views only; authority is reconstructed privately from Store receipts.
use crate::{
    goal_plan::{AttemptReservation, BudgetLimits},
    goal_store::{GoalOutputSelection, GoalValidatorRef},
    policy::{GrantScope, GrantedConnection},
    CoreError, TaskWorkspaceReservationReceipt,
};
use agentbaton_workspaces::validation::ArtifactRequirement;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[path = "integration_store.rs"]
mod store;
pub(crate) use store::migrate;
#[path = "integration_authority.rs"]
mod authority;
pub(crate) use authority::{
    IntegrationDelegatedMaterial, IntegrationScopeMaterial, IntegrationWorkspaceMaterial,
};
#[path = "integration_records.rs"]
mod records;

pub const MAX_INTEGRATION_BYTES: usize = 1_048_576;
pub const MAX_INTEGRATION_PAGE: u32 = 100;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationTargetAdvance {
    Block,
    BoundedDescendantReprepare,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationValidatorReservation {
    pub validator: GoalValidatorRef,
    pub reservation: AttemptReservation,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationStageReservations {
    pub preparation: AttemptReservation,
    pub validators: Vec<IntegrationValidatorReservation>,
    pub cas: AttemptReservation,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationPolicySelection {
    pub task_id: String,
    pub expected_task_revision: u64,
    pub expected_spec_revision: u64,
    pub goal_scope_id: String,
    pub expected_scope_revision: u64,
    pub host_id: String,
    pub target_ref: String,
    pub expected_target_head: String,
    pub max_rounds: u32,
    pub on_target_advance: IntegrationTargetAdvance,
    pub ttl_ms: u64,
    pub stage_reservations: IntegrationStageReservations,
}
/// Lossless native/renderer projection of host-observed directory identities.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationGitPin {
    pub canonical_root: PathBuf,
    pub device: String,
    pub inode: String,
    pub common_dir: PathBuf,
    pub common_device: String,
    pub common_inode: String,
    pub head: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationProgramPin {
    pub canonical_path: PathBuf,
    pub device: String,
    pub inode: String,
    pub sha256: String,
    pub bytes: u64,
    pub uid: u32,
    pub mode: u32,
    pub links: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationArtifactBinding {
    pub canonical_root: PathBuf,
    pub root_device: String,
    pub root_inode: String,
    pub requirement: ArtifactRequirement,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationValidatorBinding {
    pub reference: GoalValidatorRef,
    pub rule_id: String,
    pub definition_digest: String,
    /// Exact original immutable server definition, not executable renderer JSON.
    /// Text preserves filesystem integers before any JS number conversion.
    pub definition_json: String,
    pub program: Option<IntegrationArtifactBinding>,
    pub script: Option<IntegrationArtifactBinding>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationGrantBinding {
    pub id: String,
    pub root_grant_id: String,
    pub parent_id: Option<String>,
    pub revision: u64,
    pub approval_id: String,
    pub approved_at: String,
    pub expires_at_ms: i64,
    pub scope: GrantScope,
    pub connection_bindings: BTreeMap<String, GrantedConnection>,
    pub workspace_bindings: BTreeMap<String, IntegrationGitPin>,
    pub original_document_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntegrationWorkspaceBinding {
    TaskManaged {
        receipt: TaskWorkspaceReservationReceipt,
    },
    FactoryDelegated {
        factory_scope_id: String,
        factory_scope_digest: String,
        factory_run_id: String,
        child_spec_revision: u64,
        lineage: crate::factory::FactoryTaskLineage,
        delegation_event_sequence: u64,
        delegation_document_digest: String,
        original_request: crate::factory::FactoryDelegateRequest,
        original_handle: crate::factory::FactoryTaskHandle,
        workspace_id: String,
        registered_pin: IntegrationGitPin,
        source_base: String,
        managed_creation_ref: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationStaticBindings {
    pub task_id: String,
    pub task_goal: String,
    pub spec_revision: u64,
    pub project_id: String,
    pub project_revision: u64,
    pub project_source: IntegrationGitPin,
    pub workspace_id: String,
    pub workspace_revision: u64,
    pub source: IntegrationGitPin,
    pub workspace_binding: IntegrationWorkspaceBinding,
    pub goal_scope_id: String,
    pub goal_scope_revision: u64,
    pub goal_scope_digest: String,
    pub goal_scope_expires_at_ms: i64,
    pub root_task_id: String,
    pub root_grant_id: String,
    pub factory_scope_id: Option<String>,
    pub ancestors: Vec<String>,
    pub grant_lineage: Vec<IntegrationGrantBinding>,
    pub root_budget: BudgetLimits,
    pub project_budget: BudgetLimits,
    pub inputs: Vec<ArtifactRequirement>,
    pub outputs: Vec<GoalOutputSelection>,
    pub validators: Vec<IntegrationValidatorBinding>,
    pub system_git: IntegrationProgramPin,
    pub repository_digest: String,
    pub binding_digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationGuarantees {
    pub local_unchecked_out_ref_only: bool,
    pub merged_candidate_revalidation_required: bool,
    pub original_index_and_working_files_unchanged: bool,
    pub output_subject_rebinding_same_paths_only: bool,
    pub fixed_inputs_and_scripts_unchanged: bool,
    pub os_filesystem_sandbox: bool,
    pub os_network_sandbox: bool,
    pub whole_filesystem_output_quota: bool,
    pub remote_delivery_authorized: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationBudgetUsage {
    pub attempts: u32,
    pub active: u32,
    pub children: u32,
    pub runtime_ms: Option<u64>,
    pub output_bytes: Option<u64>,
    pub tokens: Option<u64>,
    pub cost_microunits: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationSharedUsage {
    pub root: IntegrationBudgetUsage,
    pub project: IntegrationBudgetUsage,
}
impl From<crate::goal_store::GoalBudgetUsage> for IntegrationSharedUsage {
    fn from(value: crate::goal_store::GoalBudgetUsage) -> Self {
        let project = IntegrationBudgetUsage {
            attempts: value.project.attempts,
            active: value.project.active,
            children: value.project.children,
            runtime_ms: value.project.runtime_ms,
            output_bytes: value.project.output_bytes,
            tokens: value.project.tokens,
            cost_microunits: value.project.cost_microunits,
        };
        let root = IntegrationBudgetUsage {
            attempts: value.root.attempts,
            active: value.root.active,
            children: value.root.children,
            runtime_ms: value.root.runtime_ms,
            output_bytes: value.root.output_bytes,
            tokens: value.root.tokens,
            cost_microunits: value.root.cost_microunits,
        };
        Self { root, project }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationPolicySnapshot {
    pub intent: IntegrationPolicySelection,
    pub bindings: IntegrationStaticBindings,
    pub as_of_sequence: u64,
    pub shared_usage: IntegrationSharedUsage,
    pub authority_kind: String,
    pub guarantees: IntegrationGuarantees,
    pub warning: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationPolicy {
    pub id: String,
    pub revision: u64,
    pub approval_id: String,
    pub selection: IntegrationPolicySelection,
    pub bindings: IntegrationStaticBindings,
    pub snapshot_digest: String,
    pub created_at: String,
    pub expires_at_ms: i64,
    pub revoked: bool,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationState {
    Queued,
    Capturing,
    Preparing,
    Validating,
    CasPrepared,
    ConfirmedLocalRef,
    Blocked,
    StaleTarget,
    Failed,
    CancelRequested,
    Cancelled,
    OutcomeUnknown,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationStageKind {
    Preparation,
    Validator,
    Cas,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationIoCompletion {
    WorkerJoined,
    NotScheduled,
    InterruptedNoJoinReceipt,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationValidationReceipt {
    pub attempt_id: String,
    pub round: u32,
    pub validator: GoalValidatorRef,
    pub definition_digest: String,
    pub integration_commit: String,
    pub integration_tree: String,
    pub result_digest: String,
    pub passed: bool,
    pub stop_confirmed: bool,
    pub worker_joined: bool,
    pub actual_runtime_ms: Option<u64>,
    pub actual_output_bytes: Option<u64>,
    pub details_ref: String,
    pub sequence: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationRun {
    pub id: String,
    pub policy_id: String,
    pub policy_revision: u64,
    pub revision: u64,
    pub task_id: String,
    pub spec_revision: u64,
    pub goal_scope_id: String,
    pub scope_digest: String,
    pub acceptance_ref: String,
    pub acceptance_digest: String,
    pub source_commit: Option<String>,
    pub source_tree: Option<String>,
    pub target_ref: String,
    pub expected_target_head: String,
    pub integration_commit: Option<String>,
    pub integration_tree: Option<String>,
    pub round: u32,
    pub state: IntegrationState,
    pub cancel_requested: bool,
    pub active_stage: Option<IntegrationStageKind>,
    pub io_completion: Option<IntegrationIoCompletion>,
    pub validation_receipts: Vec<IntegrationValidationReceipt>,
    pub last_error: Option<CoreError>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationAttemptView {
    pub attempt_id: String,
    pub round: u32,
    pub kind: IntegrationStageKind,
    pub validator: Option<GoalValidatorRef>,
    pub state: String,
    pub reservation: AttemptReservation,
    pub settled: bool,
    pub actual_runtime_ms: Option<u64>,
    pub actual_output_bytes: Option<u64>,
    pub io_completion: Option<IntegrationIoCompletion>,
    pub last_error: Option<CoreError>,
    pub receipt_sequence: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationFenceView {
    pub repository_digest: String,
    pub target_ref: String,
    pub integration_id: String,
    pub revision: u64,
    pub state: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationRunView {
    pub run: IntegrationRun,
    pub attempts: Vec<IntegrationAttemptView>,
    pub shared_usage: IntegrationSharedUsage,
    pub fence: IntegrationFenceView,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationListQuery {
    pub task_id: String,
    pub after_id: Option<String>,
    pub limit: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationPolicyId {
    pub policy_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationRunId {
    pub integration_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationReconciliation {
    pub integration: IntegrationRunView,
    pub observed_target_head: Option<String>,
    pub observation_scope: String,
    pub historical_cas_confirmed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationRecordCounts {
    pub policies: u64,
    pub runs: u64,
    pub attempts: u64,
    pub ref_fences: u64,
}

/// Pure canonical snapshot digest. This never verifies Owner permission.
pub fn integration_snapshot_digest(snapshot: &serde_json::Value) -> crate::Result<String> {
    authority::sha(snapshot)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationTargetQuery {
    pub task_id: String,
    pub after_ref: Option<String>,
    pub limit: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationTargetView {
    pub target_ref: String,
    pub head: String,
    pub checked_out: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationContextQuery {
    pub task_id: String,
    pub goal_scope_id: String,
}
/// A recorded preview, never executable authority or a replacement approval.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationContextView {
    pub task_revision: u64,
    pub bindings: IntegrationStaticBindings,
    pub as_of_sequence: u64,
    pub shared_usage: IntegrationSharedUsage,
    #[serde(deserialize_with = "preview_only")]
    pub preview_only: bool,
}
fn preview_only<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<bool, D::Error> {
    if bool::deserialize(d)? {
        Ok(true)
    } else {
        Err(serde::de::Error::custom(
            "integration context is preview only",
        ))
    }
}
