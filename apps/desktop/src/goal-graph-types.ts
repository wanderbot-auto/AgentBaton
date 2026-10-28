// Mirrors the schema12 Core author's goal_graph.rs / goal_evidence.rs DTOs.
// Workspace/FilePin integer identities are opaque in JavaScript. They are
// displayed, never roundtripped as approval authority or compared as u64s.
import type { WorkspaceIdentity } from './api';
import type { GoalRun } from './goal-execution-api';
import type { GoalPlanNode } from './goal-api';
export type GraphRun=Omit<GoalRun,'task_accepted'>&{readonly task_accepted:boolean};
export interface GraphArtifactReference {
  candidate_id:string;artifact_id:string;contract_id:string;producer_node_id:string;producer_attempt_id:string;
  sha256:string;bytes:number;relative_path:string;root:WorkspaceIdentity;file_pin:Record<string,unknown>;writer_epoch:number;
}
export interface GraphInputBinding {artifact_id:string;contract_id:string;sha256:string;candidate:GraphArtifactReference|null}
export interface GraphCandidate {
  id:string;graph_contract_version:number;run_id:string;scope_id:string;plan_revision:number;plan_digest:string;node_id:string;attempt_id:string;session_id:string;task_id:string;spec_revision:number;input_revision:string;workspace_id:string;capability_digest:string;consumed_inputs:GraphInputBinding[];artifacts:GraphArtifactReference[];stop_receipt_sequence:number;capture_job_id:string;capture_receipt_sequence:number;created_at:string;source:string;integrity:string;task_acceptance:string;
}
export interface GraphValidation {
  id:string;graph_contract_version:number;run_id:string;plan_revision:number;plan_digest:string;node_id:string;attempt_id:string;rule_id:string;definition_digest:string;candidate_refs:GraphArtifactReference[];input_bindings:GraphInputBinding[];state:string;passed:boolean;scope:string;correctness:string;task_acceptance:string;result_digest:string;details_ref:string;stop_confirmed:boolean;io_completion:string;actual_runtime_ms:number|null;created_at:string;source:string;
}
export interface GraphOwnerDecision {
  id:string;graph_contract_version:number;approval_id:string;run_id:string;plan_revision:number;plan_digest:string;node_id:string;rule_id:string;criterion_digest:string;candidate_refs:GraphArtifactReference[];validation_ids:string[];decision:string;created_at:string;source:string;
}
export interface GraphNode {
  node:GoalPlanNode;plan_revision:number;state:string;session_id:string|null;attempt_id:string|null;job_id:string|null;execution_kind:string;acceptance_state:string;stop_confirmed:boolean;actual_runtime_ms:number|null;actual_output_bytes:number|null;tokens:number|null;cost_microunits:number|null;candidate_ids:string[];validation_ids:string[];owner_decision_ids:string[];blocked_reason:string|null;
}
export interface GraphRunSummary {graph_contract_version:number;run:GraphRun;ledger_revision:number;task_accepted:boolean}
export interface GraphRunDetails extends GraphRunSummary {nodes:GraphNode[];candidates:GraphCandidate[];validations:GraphValidation[];owner_decisions:GraphOwnerDecision[];task_acceptance:GraphTaskAcceptance|null}
export interface GraphOwnerDecisionResult {graph_contract_version:number;approval_id:string;run_id:string;node_id:string;plan_revision:number;decision:string;owner_decision_id:string;run_revision:number;task_accepted:boolean}

export interface GraphTaskAcceptance {
  id:string;graph_contract_version:number;task_id:string;task_revision:number;spec_revision:number;project_id:string;run_id:string;scope_id:string;root_grant_id:string;plan_revision:number;plan_digest:string;input_revision:string;workspace_id:string;capability_digest:string;criterion_digest:string;artifact_refs:GraphArtifactReference[];validation_ids:string[];owner_decision_ids:string[];accepted_at:string;sequence:number;source:string;
}
