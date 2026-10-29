// Synthetic renderer-only fixtures. No Core execution, Native consent or Provider proof.
import { childPlanningFixture } from './factory-child-plan-test-fixtures';
import type { FactoryChildGoalExecutionPolicy, FactoryChildGoalView, FactoryChildGoalSummary, FactoryChildPlanView } from './factory-types';
import type { GraphRunDetails, GraphArtifactReference } from './goal-graph-types';
import type { GoalPlanNode } from './goal-api';
export async function childGoalFixture(owner=true){
 const f=childPlanningFixture();f.grant.scope.max_children=3;f.grant.scope.max_depth=3;
 const policy:FactoryChildGoalExecutionPolicy={max_nodes:4,max_runs_per_task:1,max_total_runs:2,root_budget:{...f.selection.root_budget},project_budget:{...f.selection.project_budget},require_owner_acceptance:true};
 f.selection.capabilities.child_goal_execution=true;f.selection.child_goal_execution_policy=policy;
 f.scope.bindings.child_goal_execution={schema_version:1,authority_kind:'factory_child_goal_execution_v1',policy,planning_policy_digest:f.context.planning_policy_digest,root_grant_id:f.grant.id,root_grant_revision:f.grant.revision,original_max_children:3,original_max_depth:3,graph_contract_version:1,definition_digest:'6'.repeat(64),new_workspace_authorized:false};
 if(owner){const work=f.proposal.nodes[0];f.proposal.nodes.push({...structuredClone(work),id:'owner',kind:'owner_approval',goal:'Owner reviews the exact output',dependencies:['work'],executor:{kind:'owner',rule_id:'factory-child-owner-review'},required_inputs:[{goal_task_id:f.task.id,artifact_id:'output',contract_id:'output:output',source:{kind:'node',node_id:'work',candidate_for_validation:true}}],outputs:[],required_validators:['factory-child-owner-review'],side_effect:'advisory_read_only',write_set:[],idempotency_key:'owner-once'});}
 const plan=await f.view();f.context.current_plan_revision=1;
 const plans=new Map<number,FactoryChildPlanView>([[1,plan]]);
 const runId='derived-'+f.task.id,derivedScope='derived-scope-'+f.task.id;
 function makeView(selected=plan,revision=1,state='queued'):FactoryChildGoalView{
  const node=(value:GoalPlanNode)=>({node:structuredClone(value),plan_revision:selected.reference.revision,state:'not_started',session_id:null,attempt_id:null,job_id:null,execution_kind:value.executor.kind==='owner'?'owner':value.executor.kind==='validator'?'validator':'connection',acceptance_state:'pending',stop_confirmed:false,actual_runtime_ms:null,actual_output_bytes:null,tokens:null,cost_microunits:null,candidate_ids:[],validation_ids:[],owner_decision_ids:[],blocked_reason:null});
  const details:GraphRunDetails={graph_contract_version:1,run:{id:runId,scope_id:derivedScope,project_id:f.task.project_id!,task_id:f.task.id,root_grant_id:f.grant.id,plan_revision:selected.reference.revision,plan_digest:selected.reference.plan_digest,revision,state,cancel_requested:state==='cancel_requested'||state==='cancelled',task_accepted:false,created_at:'2026-10-29T00:00:00Z',updated_at:'2026-10-29T00:00:00Z'},ledger_revision:20+revision,task_accepted:false,nodes:selected.proposal.nodes.map(node),candidates:[],validations:[],owner_decisions:[],task_acceptance:null};
  const summary:FactoryChildGoalSummary={authority_kind:'factory_child_goal_execution_v1',factory_scope_id:f.scope.id,factory_run_id:f.detail.run.id,factory_root_task_id:f.task.id,parent_task_id:null,task_id:f.task.id,plan:structuredClone(selected.reference),execution_policy_digest:'6'.repeat(64),goal:{graph_contract_version:1,run:details.run,ledger_revision:details.ledger_revision,task_accepted:false}};
  return {summary,origin:{factory_scope_id:f.scope.id,factory_scope_digest:f.scope.contract_digest,factory_run_id:f.detail.run.id,factory_root_task_id:f.task.id,parent_task_id:null,ancestors:[f.task.id],original_native_approval_id:f.scope.approval_id,root_grant_id:f.grant.id,root_grant_revision:f.grant.revision,initial_plan:structuredClone(plan.reference),execution_binding:structuredClone(f.scope.bindings.child_goal_execution!)},details};
 }
 function sync(view:FactoryChildGoalView){view.summary.goal={graph_contract_version:1,run:view.details.run,ledger_revision:view.details.ledger_revision,task_accepted:view.details.task_accepted};return view;}
 function ownerReady(){
  const view=makeView(plan,2,'awaiting_acceptance'),id='candidate-'+f.task.id;
  const artifact:GraphArtifactReference={candidate_id:id,artifact_id:'output',contract_id:'output:output',producer_node_id:'work',producer_attempt_id:'owned-attempt',sha256:'a'.repeat(64),bytes:12,relative_path:'report.txt',root:{canonical_root:f.workspace.root,device:1,inode:2,kind:{kind:'directory'}},file_pin:{device:1,inode:3,bytes:12,uid:1000,gid:1000,mode:33152,mtime:1,mtime_nsec:0,ctime:1,ctime_nsec:0},writer_epoch:10};
  view.details.nodes[0]={...view.details.nodes[0],state:'candidate',session_id:'owned-session',attempt_id:'owned-attempt',stop_confirmed:true,candidate_ids:[id]};
  view.details.nodes[1]={...view.details.nodes[1],state:'awaiting_owner',execution_kind:'owner'};
  view.details.candidates=[{id,graph_contract_version:1,run_id:runId,scope_id:derivedScope,plan_revision:1,plan_digest:plan.reference.plan_digest,node_id:'work',attempt_id:'owned-attempt',session_id:'owned-session',task_id:f.task.id,spec_revision:1,input_revision:f.context.goal.input_revision,workspace_id:f.workspace.id,capability_digest:'b'.repeat(64),consumed_inputs:[],artifacts:[artifact],stop_receipt_sequence:11,capture_job_id:'owned-capture',capture_receipt_sequence:12,created_at:plan.created_at,source:'baton_goal_evidence',integrity:'verified',task_acceptance:'not_established'}];return sync(view);
 }
 async function nextPlan(){const value=await f.view(2);plans.set(2,value);f.context.current_plan_revision=2;return value;}
 return {...f,plan,plans,runId,derivedScope,makeView,sync,ownerReady,nextPlan};
}
