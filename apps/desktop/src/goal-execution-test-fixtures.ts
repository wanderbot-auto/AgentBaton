import type { Task } from './api';
import type { GoalBudget, GoalPlan, GoalScope, GoalPreview } from './goal-api';
import type { GoalRun, GoalRunView } from './goal-execution-api';
let serial=0;
export function goalExecutionFixture(){
  const suffix=String(++serial),date='2026-10-28T07:00:00Z';
  const task:Task={id:'execution-task-'+suffix,project_id:'project',goal:'Deliver fixed report',state:'draft',revision:1,spec_revision:1,created_at:date,updated_at:date};
  const budget:GoalBudget={attempts:4,parallel:1,runtime_ms:10000,output_bytes:1048576,children:3,reworks:0,tokens:null,cost_microunits:null};
  const binding={connection_id:'connection',environment_id:'environment',identity_ref:'identity',host_id:'local',workspace_id:'workspace',config_revision:'config',input_revision:'input',capability_snapshot_id:'capabilities',policy_revision:'grant:2'};
  const scope:GoalScope={id:'execution-scope-'+suffix,revision:1,revoked:false,task_id:task.id,project_id:'project',root_grant_id:'grant-'+suffix,root_grant_revision:2,authority_revision:2,scope_digest:'c'.repeat(64),goal:{project_id:'project',root_task_id:task.id,task_id:task.id,spec_revision:1,input_revision:'input'},root_budget:budget,project_budget:budget,outputs:{report:'output:report'},required_acceptance_rules:['goal-owner-review'],contracts:{'output:report':['goal-owner-review']},inputs:{},binding,program_sha256:'b'.repeat(64),expires_at_ms:Date.now()+600000,rules:{'goal-owner-review':'d'.repeat(64)},workspace_id:'workspace',input_revision:'input',planning_only:true};
  const plan:GoalPlan={scope_id:scope.id,proposal:{schema_version:1,plan_id:'execution-plan-'+suffix,goal:scope.goal,scope_revision:2,revision:1,expected_revision:0,acceptance_rules:scope.required_acceptance_rules,nodes:[{id:'work',parent_id:null,kind:'work',goal:'Produce report',dependencies:[],workspace_id:'workspace',executor:{kind:'connection',binding,program_sha256:scope.program_sha256},recipients:[],required_inputs:[],outputs:[{artifact_id:'report',contract_id:'output:report'}],required_validators:scope.required_acceptance_rules,side_effect:'workspace_write',write_set:[{kind:'workspace',path:'outputs/report.txt'}],budget,reservation:{runtime_ms:1000,output_bytes:4096,tokens:null,cost_microunits:null},retry:{max_attempts:1,max_reworks:0,retry_classes:[]},idempotency_key:'work-key',external_action_id:null}]},digest:'e'.repeat(64),created_at:date,state:'validated_proposal',dispatched:false};
  const usage={attempts:0,active:0,children:0,runtime_ms:0,output_bytes:0,tokens:null,cost_microunits:null};
  const preview:GoalPreview={scope_id:scope.id,plan_id:plan.proposal.plan_id,plan_revision:1,journal_sequence:12,read_only:true,reservation_made:false,dispatched:false,budget_usage:{root:usage,project:usage},unknown_dimensions:[],blocked_reasons:[],decision:null};
  const run:GoalRun={id:'execution-run-'+suffix,scope_id:scope.id,project_id:'project',task_id:task.id,root_grant_id:scope.root_grant_id,plan_revision:1,plan_digest:plan.digest,revision:1,state:'queued',cancel_requested:false,task_accepted:false,created_at:date,updated_at:date};
  const view:GoalRunView={run,nodes:[{plan_revision:1,node:plan.proposal.nodes[0],state:'not_started',session_id:null,attempt_id:null,started_at_ms:null,stop_confirmed:false,actual_runtime_ms:null,actual_output_bytes:null,tokens:null,cost_microunits:null}],ledger_revision:14,usage_kind:'conservative_charged_reservations',task_accepted:false};
  return {task,scope,plan,preview,run,view};
}
export function executionView(f:ReturnType<typeof goalExecutionFixture>,run:GoalRun):GoalRunView{
  const active=['running','cancel_requested','outcome_unknown','awaiting_acceptance','cancelled','failed'].includes(run.state);
  const stopped=['awaiting_acceptance','cancelled','failed'].includes(run.state);
  return {...structuredClone(f.view),run,nodes:[{...structuredClone(f.view.nodes[0]),state:run.state==='awaiting_acceptance'?'candidate':run.state==='queued'?'not_started':run.state==='cancel_requested'?'stopping':run.state,session_id:active?'session-'+run.id:null,attempt_id:active?'attempt-'+run.id:null,started_at_ms:active?1:null,stop_confirmed:stopped,actual_runtime_ms:stopped?23:null,actual_output_bytes:stopped?31:null}]};
}
