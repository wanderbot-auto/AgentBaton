// Generated renderer test data from draft Core DTOs, never runtime proof.
import { goalExecutionFixture } from './goal-execution-test-fixtures';
import type { GraphRunSummary,GraphRunDetails,GraphCandidate,GraphArtifactReference } from './goal-graph-types';
export function goalGraphFixture(){
  const f=goalExecutionFixture();
  const summary:GraphRunSummary={graph_contract_version:1,run:{...f.run,id:'graph-'+f.run.id},ledger_revision:14,task_accepted:false};
  const view:GraphRunDetails={...summary,nodes:[{node:f.plan.proposal.nodes[0],plan_revision:1,state:'not_started',session_id:null,attempt_id:null,job_id:null,execution_kind:'connection',acceptance_state:'pending',stop_confirmed:false,actual_runtime_ms:null,actual_output_bytes:null,tokens:null,cost_microunits:null,candidate_ids:[],validation_ids:[],owner_decision_ids:[],blocked_reason:null}],candidates:[],validations:[],owner_decisions:[],task_acceptance:null};
  return {...f,summary,graphView:view};
}
export function graphCandidateView(f:ReturnType<typeof goalGraphFixture>):GraphRunDetails {
  const view=structuredClone(f.graphView),id='candidate-'+f.task.id;
  const artifact:GraphArtifactReference={candidate_id:id,artifact_id:'report',contract_id:'output:report',producer_node_id:'work',producer_attempt_id:'attempt',sha256:'a'.repeat(64),bytes:12,relative_path:'outputs/report.txt',root:{canonical_root:'/generated/workspace',device:1,inode:2,kind:{kind:'directory'}},file_pin:{device:1,inode:3,bytes:12,uid:1000,gid:1000,mode:33152,mtime:1,mtime_nsec:0,ctime:1,ctime_nsec:0},writer_epoch:10};
  const candidate:GraphCandidate={id,graph_contract_version:1,run_id:view.run.id,scope_id:f.scope.id,plan_revision:1,plan_digest:view.run.plan_digest,node_id:'work',attempt_id:'attempt',session_id:'session',task_id:f.task.id,spec_revision:1,input_revision:f.scope.input_revision,workspace_id:f.scope.workspace_id,capability_digest:'b'.repeat(64),consumed_inputs:[],artifacts:[artifact],stop_receipt_sequence:11,capture_job_id:'capture-job',capture_receipt_sequence:12,created_at:view.run.created_at,source:'baton_goal_evidence',integrity:'verified',task_acceptance:'not_established'};
  view.run={...view.run,revision:2,state:'awaiting_acceptance'};view.nodes=[{...view.nodes[0],state:'candidate',session_id:'session',attempt_id:'attempt',stop_confirmed:true,candidate_ids:[id]}];view.candidates=[candidate];return view;
}
export function graphAcceptedFixture(f:ReturnType<typeof goalGraphFixture>){
  const view=graphCandidateView(f),id='acceptance-'+f.task.id,decisionId='decision-'+f.task.id,criterion='d'.repeat(64);
  const node={...structuredClone(f.plan.proposal.nodes[0]),id:'owner',kind:'owner_approval' as const,goal:'Review exact report',dependencies:['work'],executor:{kind:'owner' as const,rule_id:'goal-owner-review'},outputs:[],write_set:[]};
  view.nodes[0].acceptance_state='accepted';view.nodes.push({...structuredClone(view.nodes[0]),node,state:'accepted',execution_kind:'owner',session_id:null,attempt_id:null,job_id:null,stop_confirmed:false,candidate_ids:[],owner_decision_ids:[decisionId]});
  view.owner_decisions=[{id:decisionId,graph_contract_version:1,approval_id:'approval-'+f.task.id,run_id:view.run.id,plan_revision:1,plan_digest:view.run.plan_digest,node_id:'owner',rule_id:'goal-owner-review',criterion_digest:criterion,candidate_refs:view.candidates[0].artifacts,validation_ids:[],decision:'accepted',created_at:view.run.created_at,source:'local_owner_confirmation'}];
  view.run={...view.run,revision:3,state:'accepted',task_accepted:true};view.task_accepted=true;view.ledger_revision=22;
  view.task_acceptance={id,graph_contract_version:1,task_id:f.task.id,task_revision:7,spec_revision:1,project_id:f.scope.project_id,run_id:view.run.id,scope_id:f.scope.id,root_grant_id:f.scope.root_grant_id,plan_revision:1,plan_digest:view.run.plan_digest,input_revision:f.scope.input_revision,workspace_id:f.scope.workspace_id,capability_digest:'b'.repeat(64),criterion_digest:criterion,artifact_refs:view.candidates[0].artifacts,validation_ids:[],owner_decision_ids:[decisionId],accepted_at:view.run.created_at,sequence:22,source:'baton_goal_acceptance'};
  return {view,task:{...f.task,state:'accepted',revision:7,acceptance_ref:id,acceptance_sequence:22}};
}
