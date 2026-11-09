import {goalExecutionFixture} from './goal-execution-test-fixtures';
import {draftRequest,draftRiskCodes,type GoalPlanDraft} from './goal-plan-draft-api';
export function goalDraftFixture(){
 const f=goalExecutionFixture(),artifact={artifact_id:'report',relative_path:'outputs/report.txt',max_bytes:4096,expected_sha256:null};
 f.scope.edit_context={max_nodes:4,writable_prefixes:['outputs'],output_artifacts:{report:artifact},rule_executors:{'goal-owner-review':'owner'}};
 const selection=draftRequest(f.task,f.scope,'Review overwritten files','Do not publish','1000','4096');
 const plan=structuredClone(f.plan.proposal),node=plan.nodes[0];node.id='step-1';node.goal=`Task goal:\n${f.task.goal}\n\nUser-reviewed risks:\n${selection.risk_notes}\n\nProhibited actions:\n${selection.prohibited_actions}`;node.budget={attempts:1,parallel:1,runtime_ms:1000,output_bytes:4096,children:0,reworks:0,tokens:null,cost_microunits:null};node.reservation={runtime_ms:1000,output_bytes:4096,tokens:null,cost_microunits:null};
 const draft:GoalPlanDraft={source:{task_id:f.task.id,task_revision:1,spec_revision:1,task_goal:f.task.goal,scope_id:f.scope.id,scope_revision:1,scope_digest:f.scope.scope_digest,authority_revision:2},approved_acceptance_criteria:'Report must explain all verified results.',approved_outputs:[{artifact,validators:[]}],writable_prefixes:['outputs'],root_budget:f.scope.root_budget,project_budget:f.scope.project_budget,risk_facts:[...draftRiskCodes],plan,plan_digest:'a'.repeat(64),planning_only:true,dispatched:false};
 return {...f,selection,draft};
}
