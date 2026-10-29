import { request } from './api';
import { utf8Bytes } from './goal-api';
import { verifyGraphObservationText } from './goal-graph-guards';
import type { FactorySelection, FactoryScope, FactoryRun, FactoryRunDetails, FactoryChildPlanningContext, FactoryChildPlanView, FactoryChildPlanRef, FactoryChildGoalSubmission, FactoryChildGoalSummary, FactoryChildGoalView } from './factory-types';
export const factoryApi={
  approve:(selection:FactorySelection,idempotencyKey:string)=>request<FactoryScope>('factory_scope_approve',{selection,idempotencyKey}),
  get:(scopeId:string)=>request<FactoryScope>('factory_scopes_get',{scopeId}),
  list:(rootTaskId:string)=>request<FactoryScope[]>('factory_scopes_list',{rootTaskId}),
  revoke:(scopeId:string,expectedRevision:number,idempotencyKey:string)=>request<FactoryScope>('factory_scopes_revoke',{scopeId,expectedRevision,idempotencyKey}),
  start:(scopeId:string,expectedRevision:number,idempotencyKey:string)=>request<FactoryRun>('factory_runs_start',{scopeId,expectedRevision,idempotencyKey}),
  run:(scopeId:string)=>request<FactoryRunDetails>('factory_runs_get',{scopeId}),
  cancel:(scopeId:string,expectedRevision:number,idempotencyKey:string)=>request<FactoryRun>('factory_runs_cancel',{scopeId,expectedRevision,idempotencyKey}),
};

// Owner-only fixed Native bridges. No scoped-token/Agent generic dispatch.
export const factoryChildPlanApi={
 context:(scopeId:string,taskId:string)=>request<FactoryChildPlanningContext>('factory_child_plans_context',{scopeId,taskId}),
 get:(scopeId:string,taskId:string,planRevision?:number)=>request<FactoryChildPlanView>('factory_child_plans_get',{scopeId,taskId,...(planRevision===undefined?{}:{planRevision})}),
 list:(scopeId:string,taskId:string)=>request<FactoryChildPlanRef[]>('factory_child_plans_list',{scopeId,taskId}),
 submit:(scopeId:string,taskId:string,planJson:string,expectedPlanRevision:number,expectedRevision:number,idempotencyKey:string)=>request<FactoryChildPlanView>('factory_child_plans_submit',{scopeId,taskId,planJson,expectedPlanRevision,expectedRevision,idempotencyKey}),
};

// Fixed Owner bridges only. Original saved references, CAS and keys are sent;
// no renderer-supplied origin, resolved execution binding, private token or approval.
export const factoryChildGoalApi={
 start:(scopeId:string,request_:FactoryChildGoalSubmission,expectedRevision:number,idempotencyKey:string)=>request<FactoryChildGoalSummary>('factory_child_goals_start',{scopeId,request:request_,expectedRevision,idempotencyKey}),
 get:async(scopeId:string,taskId:string,goalRunId:string|null)=>{
  const view=await request<FactoryChildGoalView>('factory_child_goals_get',{scopeId,taskId,goalRunId});
  if(Array.isArray(view?.details?.validations)){
   if(view.details.validations.length>65536||utf8Bytes(JSON.stringify(view))>4194304)throw new Error('Child Goal details exceed the existing display limit');
   for(const validation of view.details.validations)await verifyGraphObservationText(validation);
  }
  return view;
 },
 list:(scopeId:string,taskId:string)=>request<FactoryChildGoalSummary[]>('factory_child_goals_list',{scopeId,taskId}),
 cancel:(scopeId:string,taskId:string,goalRunId:string,expectedGoalRunRevision:number,expectedRevision:number,idempotencyKey:string)=>request<FactoryChildGoalSummary>('factory_child_goals_cancel',{scopeId,taskId,goalRunId,expectedGoalRunRevision,expectedRevision,idempotencyKey}),
 advancePlan:(scopeId:string,goalRunId:string,request_:FactoryChildGoalSubmission,expectedGoalRunRevision:number,expectedRevision:number,idempotencyKey:string)=>request<FactoryChildGoalSummary>('factory_child_goals_advance_plan',{scopeId,goalRunId,request:request_,expectedGoalRunRevision,expectedRevision,idempotencyKey}),
};
