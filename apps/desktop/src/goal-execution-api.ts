import { request, type Task } from './api';
import { safeInteger, type GoalPlanNode, type GoalScope } from './goal-api';

export interface GoalRunSelection { scope_id:string; expected_plan_revision:number; expected_ledger_revision:number }
export interface GoalPlanAdvance { run_id:string; expected_plan_revision:number; expected_ledger_revision:number }
export interface GoalRun {
  readonly id:string; readonly scope_id:string; readonly project_id:string; readonly task_id:string; readonly root_grant_id:string;
  readonly plan_revision:number; readonly plan_digest:string; readonly revision:number; readonly state:string;
  readonly cancel_requested:boolean; readonly task_accepted:false; readonly created_at:string; readonly updated_at:string;
}
export interface GoalRunNode {
  readonly plan_revision:number; readonly node:GoalPlanNode; readonly state:string; readonly session_id:string|null; readonly attempt_id:string|null;
  readonly started_at_ms:number|null; readonly stop_confirmed:boolean;
  readonly actual_runtime_ms:number|null; readonly actual_output_bytes:number|null; readonly tokens:number|null; readonly cost_microunits:number|null;
}
export interface GoalRunView {
  readonly run:GoalRun; readonly nodes:GoalRunNode[]; readonly ledger_revision:number;
  readonly usage_kind:'conservative_charged_reservations'; readonly task_accepted:false;
}
export interface GoalRunFloor { revision:number; planRevision:number; digest:string; scopeId:string; rootGrantId:string; state:string; cancelRequested:boolean; createdAt:string; updatedAt:string }
const id=(value:unknown):value is string=>typeof value==='string'&&/^[A-Za-z0-9._:-]{1,128}$/.test(value);
const hash=(value:unknown):value is string=>typeof value==='string'&&/^[a-f0-9]{64}$/.test(value);
const timestamp=(value:unknown):value is string=>typeof value==='string'&&value.length>0&&value.length<=128;
const meter=(value:unknown)=>value===null||safeInteger(value);
export const goalRunFloor=(run:GoalRun):GoalRunFloor=>({revision:run.revision,planRevision:run.plan_revision,digest:run.plan_digest,scopeId:run.scope_id,rootGrantId:run.root_grant_id,state:run.state,cancelRequested:run.cancel_requested,createdAt:run.created_at,updatedAt:run.updated_at});
export const goalRunMeetsFloor=(run:GoalRun,floor:GoalRunFloor)=>run.scope_id===floor.scopeId&&run.root_grant_id===floor.rootGrantId&&run.revision>=floor.revision&&run.created_at===floor.createdAt&&run.plan_revision>=floor.planRevision&&(run.plan_revision!==floor.planRevision||run.plan_digest===floor.digest)&&(run.revision!==floor.revision||run.plan_revision===floor.planRevision&&run.state===floor.state&&run.cancel_requested===floor.cancelRequested&&run.updated_at===floor.updatedAt);
export const goalRunStates=['queued','running','cancel_requested','cancelled','awaiting_acceptance','blocked','failed','outcome_unknown'];
export const goalRunNodeStates=['not_started','reserved','running','stopping','candidate','cancelled','failed','outcome_unknown'];
// Owned stop is a lifecycle observation, not a proof that effects are known.
const nodeKnownClosed=(record:GoalRunNode)=>record.state==='not_started'
  ?record.session_id===null&&record.attempt_id===null&&!record.stop_confirmed
  :['candidate','cancelled','failed'].includes(record.state)&&record.session_id!==null&&record.attempt_id!==null&&record.stop_confirmed;
export const goalRunViewSupported=(view:GoalRunView)=>goalRunStates.includes(view.run.state)&&view.nodes.every(record=>goalRunNodeStates.includes(record.state));
export const goalRunViewClosed=(view:GoalRunView)=>goalRunViewSupported(view)&&view.nodes.every(nodeKnownClosed);
export const goalRunCanAdvanceView=(view:GoalRunView)=>!view.run.cancel_requested&&['queued','running','blocked','awaiting_acceptance'].includes(view.run.state)&&goalRunViewClosed(view);

export const goalRunNeedsReconciliation=(run:GoalRun)=>!['cancelled','failed'].includes(run.state);
export const goalRunCanCancel=(run:GoalRun)=>!run.cancel_requested&&['queued','running','blocked','outcome_unknown','awaiting_acceptance'].includes(run.state);
export const goalRunPolling=(run:GoalRun)=>['queued','running','cancel_requested'].includes(run.state);
export function goalRunShape(value:GoalRun,task:Task,scope?:GoalScope):boolean {
  return !!value&&id(value.id)&&id(value.scope_id)&&value.task_id===task.id&&value.project_id===task.project_id&&id(value.root_grant_id)
    &&safeInteger(value.plan_revision,1)&&hash(value.plan_digest)&&safeInteger(value.revision,1)&&typeof value.state==='string'&&value.state.length>0&&value.state.length<=128
    &&typeof value.cancel_requested==='boolean'&&value.task_accepted===false&&timestamp(value.created_at)&&timestamp(value.updated_at)
    &&(!scope||(value.scope_id===scope.id&&value.root_grant_id===scope.root_grant_id))
    &&(value.state!=='cancel_requested'||value.cancel_requested)&&(value.state!=='cancelled'||value.cancel_requested);
}
export function goalRunViewShape(value:GoalRunView,task:Task,scope:GoalScope):boolean {
  if(!value||!goalRunShape(value.run,task,scope)||value.task_accepted!==false||value.usage_kind!=='conservative_charged_reservations'||!safeInteger(value.ledger_revision)||!Array.isArray(value.nodes)||value.nodes.length<1||value.nodes.length>65_536||new TextEncoder().encode(JSON.stringify(value.nodes)).length>4_194_304)return false;
  const keys=new Set<string>();
  for(const record of value.nodes){
    const node=record?.node;
    if(!record||!node||!safeInteger(record.plan_revision,1)||record.plan_revision>value.run.plan_revision||!id(node.id)||typeof node.goal!=='string'||node.goal.length>16384||!['work','validate','owner_approval','summary'].includes(node.kind)||node.workspace_id!==scope.workspace_id||typeof record.state!=='string'||record.state.length<1||record.state.length>128||typeof record.stop_confirmed!=='boolean')return false;
    if(![record.started_at_ms,record.actual_runtime_ms,record.actual_output_bytes,record.tokens,record.cost_microunits].every(meter)||!(record.session_id===null||id(record.session_id))||!(record.attempt_id===null||id(record.attempt_id))||(record.session_id===null)!==(record.attempt_id===null))return false;
    if(goalRunNodeStates.includes(record.state)){
      if(record.state==='not_started'){if(record.session_id!==null||record.attempt_id!==null||record.stop_confirmed)return false;}
      else if(record.session_id===null||record.attempt_id===null)return false;
      if(['reserved','running','stopping'].includes(record.state)&&record.stop_confirmed)return false;
      if(['candidate','cancelled','failed'].includes(record.state)&&!record.stop_confirmed)return false;
    }
    const reserved=node.reservation;
    if(!reserved||!safeInteger(reserved.runtime_ms,1)||!safeInteger(reserved.output_bytes,1)||![reserved.tokens,reserved.cost_microunits].every(meter))return false;
    const key=JSON.stringify([record.plan_revision,node.id,record.attempt_id]);if(keys.has(key))return false;keys.add(key);
  }
  return !['cancelled','failed','awaiting_acceptance'].includes(value.run.state)||goalRunViewClosed(value);
}
export const goalExecutionApi={
  approve:(selection:GoalRunSelection,idempotencyKey:string)=>request<GoalRun>('goal_run_approve',{selection,idempotencyKey}),
  list:(taskId:string)=>request<GoalRun[]>('goal_runs_list',{taskId}),
  get:(runId:string)=>request<GoalRunView>('goal_runs_get',{runId}),
  cancel:(runId:string,expectedRevision:number,idempotencyKey:string)=>request<GoalRun>('goal_runs_cancel',{runId,expectedRevision,idempotencyKey}),
  advancePlan:(selection:GoalPlanAdvance,expectedRevision:number,idempotencyKey:string)=>request<GoalRun>('goal_run_advance_plan',{runId:selection.run_id,expectedPlanRevision:selection.expected_plan_revision,expectedLedgerRevision:selection.expected_ledger_revision,expectedRevision,idempotencyKey}),
};
