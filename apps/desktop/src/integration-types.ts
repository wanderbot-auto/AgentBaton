import type { GoalBudget, GoalScope, GoalUsage, GoalValidatorRef } from './goal-api';
import type { TaskWorkspaceReservationReceipt } from './task-workspace-binding-wire';
import type { ArtifactRequirement } from './validation-api';
import type { ExecutionGrant } from './session-api';

/** Mirrors the independent Core integration contract. These records are views,
 * never renderer-created authority. Filesystem identities remain decimal text. */
export interface IntegrationReservation { runtime_ms:number; output_bytes:number; tokens:number|null; cost_microunits:number|null }
export interface IntegrationStageReservations { preparation:IntegrationReservation; validators:{validator:GoalValidatorRef;reservation:IntegrationReservation}[]; cas:IntegrationReservation }
export interface IntegrationPolicySelection {
 task_id:string;expected_task_revision:number;expected_spec_revision:number;goal_scope_id:string;expected_scope_revision:number;
 host_id:string;target_ref:string;expected_target_head:string;max_rounds:number;on_target_advance:'block'|'bounded_descendant_reprepare';ttl_ms:number;stage_reservations:IntegrationStageReservations;
}
export interface IntegrationGitPin { canonical_root:string;device:string;inode:string;common_dir:string;common_device:string;common_inode:string;head:string }
export interface IntegrationProgramPin { canonical_path:string;device:string;inode:string;sha256:string;bytes:number;uid:number;mode:number;links:string }
export interface IntegrationArtifactBinding {canonical_root:string;root_device:string;root_inode:string;requirement:ArtifactRequirement}
export interface IntegrationValidatorBinding {reference:GoalValidatorRef;rule_id:string;definition_digest:string;definition_json:string;program:IntegrationArtifactBinding|null;script:IntegrationArtifactBinding|null}
export interface IntegrationGrantBinding {id:string;root_grant_id:string;parent_id:string|null;revision:number;approval_id:string;approved_at:string;expires_at_ms:number;scope:ExecutionGrant['scope'];connection_bindings:Record<string,unknown>;workspace_bindings:Record<string,IntegrationGitPin>;original_document_sha256:string}
export interface IntegrationStaticBindings {
 task_id:string;task_goal:string;spec_revision:number;project_id:string;project_revision:number;project_source:IntegrationGitPin;
 workspace_id:string;workspace_revision:number;source:IntegrationGitPin;task_workspace_receipt:TaskWorkspaceReservationReceipt;
 goal_scope_id:string;goal_scope_revision:number;goal_scope_digest:string;root_task_id:string;root_grant_id:string;factory_scope_id:string|null;ancestors:string[];
 grant_lineage:IntegrationGrantBinding[];root_budget:GoalBudget;project_budget:GoalBudget;inputs:ArtifactRequirement[];
 outputs:{artifact:ArtifactRequirement;validators:GoalValidatorRef[]}[];validators:IntegrationValidatorBinding[];system_git:IntegrationProgramPin;repository_digest:string;binding_digest:string;
}
export interface IntegrationGuarantees {local_unchecked_out_ref_only:boolean;merged_candidate_revalidation_required:boolean;original_index_and_working_files_unchanged:boolean;output_subject_rebinding_same_paths_only:boolean;fixed_inputs_and_scripts_unchanged:boolean;os_filesystem_sandbox:boolean;os_network_sandbox:boolean;whole_filesystem_output_quota:boolean;remote_delivery_authorized:boolean}
export interface IntegrationSharedUsage {root:GoalUsage;project:GoalUsage}
export interface IntegrationPolicySnapshot {intent:IntegrationPolicySelection;bindings:IntegrationStaticBindings;as_of_sequence:number;shared_usage:IntegrationSharedUsage;authority_kind:string;guarantees:IntegrationGuarantees;warning:string}
export interface IntegrationPolicy {id:string;revision:number;approval_id:string;selection:IntegrationPolicySelection;bindings:IntegrationStaticBindings;snapshot_digest:string;created_at:string;expires_at_ms:number;revoked:boolean}
export type IntegrationState='queued'|'capturing'|'preparing'|'validating'|'cas_prepared'|'confirmed_local_ref'|'blocked'|'stale_target'|'failed'|'cancel_requested'|'cancelled'|'outcome_unknown';
export type IntegrationStageKind='preparation'|'validator'|'cas';
export type IntegrationIoCompletion='worker_joined'|'not_scheduled'|'interrupted_no_join_receipt';
export interface IntegrationError {code:string;message:string;retryable:boolean;side_effect_state:string}
export interface IntegrationValidationReceipt {attempt_id:string;round:number;validator:GoalValidatorRef;definition_digest:string;integration_commit:string;integration_tree:string;result_digest:string;passed:boolean;stop_confirmed:boolean;worker_joined:boolean;actual_runtime_ms:number|null;actual_output_bytes:number|null;details_ref:string;sequence:number}
export interface IntegrationRun {id:string;policy_id:string;policy_revision:number;revision:number;task_id:string;spec_revision:number;goal_scope_id:string;scope_digest:string;acceptance_ref:string;acceptance_digest:string;source_commit:string|null;source_tree:string|null;target_ref:string;expected_target_head:string;integration_commit:string|null;integration_tree:string|null;round:number;state:IntegrationState;cancel_requested:boolean;active_stage:IntegrationStageKind|null;io_completion:IntegrationIoCompletion|null;validation_receipts:IntegrationValidationReceipt[];last_error:IntegrationError|null;created_at:string;updated_at:string}
export interface IntegrationAttemptView {attempt_id:string;round:number;kind:IntegrationStageKind;validator:GoalValidatorRef|null;state:string;reservation:IntegrationReservation;settled:boolean;actual_runtime_ms:number|null;actual_output_bytes:number|null;io_completion:IntegrationIoCompletion|null;last_error:IntegrationError|null;receipt_sequence:number|null}
export interface IntegrationFenceView {repository_digest:string;target_ref:string;integration_id:string;revision:number;state:string}
export interface IntegrationRunView {run:IntegrationRun;attempts:IntegrationAttemptView[];shared_usage:IntegrationSharedUsage;fence:IntegrationFenceView}
export interface IntegrationReconciliation {integration:IntegrationRunView;observed_target_head:string|null;observation_scope:string;historical_cas_confirmed:boolean}
export interface IntegrationTarget {target_ref:string;head:string;checked_out:boolean}
export interface IntegrationContext {scope:GoalScope;validators:GoalValidatorRef[]}
