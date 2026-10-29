import { request } from './api';
import type { FactorySelection, FactoryScope, FactoryRun, FactoryRunDetails, FactoryChildPlanningContext, FactoryChildPlanView, FactoryChildPlanRef } from './factory-types';
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
