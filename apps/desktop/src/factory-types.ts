import type { Task, ManagedWorkspace, Project } from './api';
import type { ExecutionGrant, NamedConnection } from './session-api';
import type { GoalBudget } from './goal-api';
import type { ArtifactRequirement } from './validation-api';
export interface FactoryCapabilities { delegate_task:boolean; resume_parent_as_new_attempt:boolean; child_planning:boolean; child_goal_execution:boolean; create_workspace:boolean }
export interface FactorySelection {
  root_task_id:string; expected_task_revision:number; grant_id:string; expected_grant_revision:number;
  connection_ids:string[]; workspace_ids:string[]; inputs:ArtifactRequirement[]; outputs:ArtifactRequirement[];
  recipients:string[]; max_depth:number; max_children:number; max_total_nodes:number;
  root_budget:GoalBudget; project_budget:GoalBudget; ttl_ms:number; capabilities:FactoryCapabilities;
}
// Directory dev/inode metadata is opaque here: JS Number cannot faithfully
// represent every u64. Native Rust displays/revalidates the COMPLETE original
// snapshot; renderer never compares or round-trips those filesystem identities.
export interface FactoryContract { canonical_root:string; root_device:number; root_inode:number; requirement:ArtifactRequirement }
export interface FactoryInputBinding { contract:FactoryContract; sha256:string; bytes:number }
export interface FactoryCompanionBinding { executable:string; executable_sha256:string; argument_template:string[]; credential_transport:string; global_daemon_token_available:boolean }
export interface FactoryGuarantees { filesystem_sandbox:boolean; network_sandbox:boolean; account_isolation:boolean; output_quota:boolean; continuous_input_immutability:boolean; complete_process_tree_containment:boolean }
export interface FactoryStartContract { mode:string; root_task_request_digest:string; context_continuation:string; implicit_start_on_approval:boolean; new_workspace_authority:boolean }
export interface FactoryBindings { project:Project; root_task:Task; approved_grant:ExecutionGrant; connections:NamedConnection[]; workspaces:ManagedWorkspace[]; inputs:FactoryInputBinding[]; outputs:FactoryContract[]; companion:FactoryCompanionBinding; guarantees:FactoryGuarantees; start_contract:FactoryStartContract }
export interface FactoryScope { id:string; approval_id:string; project_id:string; root_task_id:string; root_grant_id:string; root_grant_revision:number; revision:number; revoked:boolean; expires_at_ms:number; selection:FactorySelection; bindings:FactoryBindings; contract_digest:string; created_at:string }
export interface FactoryTaskHandle { task_id:string; parent_task_id:string|null; root_task_id:string; delegation_id:string; factory_scope_id:string; revision:number; state:string; attempt_id:string|null; cancel_requested:boolean; confirmed_stop:boolean; result_available:boolean }
export interface FactoryWaitingRelation { parent_task_id:string; child_task_id:string; parent_session_id:string; parent_stop_confirmed:boolean; handoff_digest:string|null }
export interface FactoryRun { id:string; scope_id:string; project_id:string; root_task_id:string; revision:number; state:string; active_task_id:string|null; active_session_id:string|null; cancel_requested:boolean; context_continuation:string; last_error:{code:string;message:string;retryable:boolean;side_effect_state:string;details_ref?:string|null}|null; created_at:string; updated_at:string }
export interface FactoryRunDetails { run:FactoryRun; tasks:FactoryTaskHandle[]; waiting_relations:FactoryWaitingRelation[] }
