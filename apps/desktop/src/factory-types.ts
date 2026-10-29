import type { Task, ManagedWorkspace, Project, WorkspaceIdentity } from './api';
import type { ExecutionGrant, NamedConnection } from './session-api';
import type { GoalBudget, GoalValidatorRef, GoalBinding, GoalExecutionBinding, GoalEditContext, GoalPlanProposal } from './goal-api';
import type { ArtifactRequirement } from './validation-api';
import type { GraphRunSummary, GraphRunDetails } from './goal-graph-types';
export interface FactoryCapabilities { delegate_task:boolean; resume_parent_as_new_attempt:boolean; child_planning:boolean; child_goal_execution:boolean; create_workspace:boolean }
export interface FactorySelection {
  root_task_id:string; expected_task_revision:number; grant_id:string; expected_grant_revision:number;
  connection_ids:string[]; workspace_ids:string[]; inputs:ArtifactRequirement[]; outputs:ArtifactRequirement[];
  recipients:string[]; max_depth:number; max_children:number; max_total_nodes:number;
  root_budget:GoalBudget; project_budget:GoalBudget; ttl_ms:number; capabilities:FactoryCapabilities;
  child_planning_policy?:FactoryChildPlanningPolicy;
  child_goal_execution_policy?:FactoryChildGoalExecutionPolicy;
}
// Directory dev/inode metadata is opaque here: JS Number cannot faithfully
// represent every u64. Native Rust displays/revalidates the COMPLETE original
// snapshot; renderer never compares or round-trips those filesystem identities.
export interface FactoryContract { canonical_root:string; root_device:number; root_inode:number; requirement:ArtifactRequirement }
export interface FactoryInputBinding { contract:FactoryContract; sha256:string; bytes:number }
export interface FactoryCompanionBinding { executable:string; executable_sha256:string; argument_template:string[]; credential_transport:string; global_daemon_token_available:boolean }
export interface FactoryGuarantees { filesystem_sandbox:boolean; network_sandbox:boolean; account_isolation:boolean; output_quota:boolean; continuous_input_immutability:boolean; complete_process_tree_containment:boolean }
export interface FactoryStartContract { mode:string; root_task_request_digest:string; context_continuation:string; implicit_start_on_approval:boolean; new_workspace_authority:boolean }
export interface FactoryGrantedConnection { protocol_route:string; revision:number; environment_id:string; identity_ref:string; host_id:string; config_revision:string; executable_sha256:string; capability_snapshot_id:string }
export interface FactoryGrant extends ExecutionGrant { connection_bindings:Record<string,FactoryGrantedConnection>; workspace_bindings:Record<string,WorkspaceIdentity> }
export interface FactoryBindings { child_planning?:FactoryChildPlanningBinding; child_goal_execution?:FactoryChildGoalExecutionBinding; project:Project; root_task:Task; approved_grant:FactoryGrant; connections:NamedConnection[]; workspaces:ManagedWorkspace[]; inputs:FactoryInputBinding[]; outputs:FactoryContract[]; companion:FactoryCompanionBinding; guarantees:FactoryGuarantees; start_contract:FactoryStartContract }
export interface FactoryScope { id:string; approval_id:string; project_id:string; root_task_id:string; root_grant_id:string; root_grant_revision:number; revision:number; revoked:boolean; expires_at_ms:number; selection:FactorySelection; bindings:FactoryBindings; contract_digest:string; created_at:string }
export interface FactoryTaskHandle { task_id:string; parent_task_id:string|null; root_task_id:string; delegation_id:string; factory_scope_id:string; revision:number; state:string; attempt_id:string|null; cancel_requested:boolean; confirmed_stop:boolean; result_available:boolean }
export interface FactoryWaitingRelation { parent_task_id:string; child_task_id:string; parent_session_id:string; parent_stop_confirmed:boolean; handoff_digest:string|null }
export interface FactoryRun { id:string; scope_id:string; project_id:string; root_task_id:string; revision:number; state:string; active_task_id:string|null; active_session_id:string|null; cancel_requested:boolean; context_continuation:string; last_error:{code:string;message:string;retryable:boolean;side_effect_state:string;details_ref?:string|null}|null; created_at:string; updated_at:string }
export interface FactoryRunDetails { run:FactoryRun; tasks:FactoryTaskHandle[]; waiting_relations:FactoryWaitingRelation[] }

// Recorded plan-only views. None is an execution capability or approval token.
export interface FactoryChildPlanningPolicy { max_nodes:number; max_revisions:number; max_plan_bytes:number; outputs:{artifact:ArtifactRequirement;validators:GoalValidatorRef[]}[]; writable_prefixes:string[]; owner_acceptance_criteria:string }
export interface FactoryChildPlanningBinding { schema_version:number; authority_kind:string; policy:FactoryChildPlanningPolicy; owner_rule_id:string; input_rule_id:string; validator_definitions:Record<string,unknown>; definition_digest:string; goal_execution_authorized:boolean; new_workspace_authorized:boolean }
export interface FactoryChildPlanRef { id:string; scope_id:string; run_id:string; task_id:string; revision:number; plan_digest:string; raw_sha256:string; receipt_sequence:number }
export interface FactoryChildPlanView { reference:FactoryChildPlanRef; proposal:GoalPlanProposal; plan_json:string; authority_kind:string; scope_digest:string; planning_policy_digest:string; root_task_id:string; root_grant_id:string; root_grant_revision:number; caller_task_id:string; caller_session_id:string|null; caller_attempt_id:string|null; caller_fencing_token:number|null; target_spec_revision:number; input_revision:string; created_at:string; planning_only:boolean; dispatched:boolean; task_accepted:boolean }
export interface FactoryChildPlanningContext { scope_id:string; run_id:string; scope_revision:number; scope_digest:string; planning_policy_digest:string; goal:GoalBinding; policy:FactoryChildPlanningPolicy; execution_bindings:GoalExecutionBinding[]; edit_context:GoalEditContext; program_sha256:Record<string,string>; inputs:Record<string,{contract_id:string;sha256:string}>; outputs:Record<string,string>; contracts:Record<string,string[]>; rules:Record<string,string>; required_acceptance_rules:string[]; budget:GoalBudget; project_budget:GoalBudget; current_plan_revision:number; planning_only:boolean; goal_execution_authorized:boolean; new_workspace_authorized:boolean }

// Schema15 contract frozen at 2a3d6ae. These are server evidence, never renderer authority.
export interface FactoryChildGoalExecutionPolicy { max_nodes:number; max_runs_per_task:number; max_total_runs:number; root_budget:GoalBudget; project_budget:GoalBudget; require_owner_acceptance:boolean }
export interface FactoryChildGoalExecutionBinding { schema_version:number; authority_kind:string; policy:FactoryChildGoalExecutionPolicy; planning_policy_digest:string; root_grant_id:string; root_grant_revision:number; original_max_children:number; original_max_depth:number; graph_contract_version:number; definition_digest:string; new_workspace_authorized:boolean }
export interface FactoryChildGoalSubmission { task_id:string; plan:FactoryChildPlanRef; expected_factory_run_revision:number }
export interface FactoryChildGoalSummary { authority_kind:string; factory_scope_id:string; factory_run_id:string; factory_root_task_id:string; parent_task_id:string|null; task_id:string; plan:FactoryChildPlanRef; execution_policy_digest:string; goal:GraphRunSummary }
export interface FactoryChildGoalOriginView { factory_scope_id:string; factory_scope_digest:string; factory_run_id:string; factory_root_task_id:string; parent_task_id:string|null; ancestors:string[]; original_native_approval_id:string; root_grant_id:string; root_grant_revision:number; initial_plan:FactoryChildPlanRef; execution_binding:FactoryChildGoalExecutionBinding }
export interface FactoryChildGoalView { summary:FactoryChildGoalSummary; origin:FactoryChildGoalOriginView; details:GraphRunDetails }
