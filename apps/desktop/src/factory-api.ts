import { request } from './api';
import type { FactorySelection, FactoryScope, FactoryRun, FactoryRunDetails } from './factory-types';
export const factoryApi={
  approve:(selection:FactorySelection,idempotencyKey:string)=>request<FactoryScope>('factory_scope_approve',{selection,idempotencyKey}),
  get:(scopeId:string)=>request<FactoryScope>('factory_scopes_get',{scopeId}),
  list:(rootTaskId:string)=>request<FactoryScope[]>('factory_scopes_list',{rootTaskId}),
  revoke:(scopeId:string,expectedRevision:number,idempotencyKey:string)=>request<FactoryScope>('factory_scopes_revoke',{scopeId,expectedRevision,idempotencyKey}),
  start:(scopeId:string,expectedRevision:number,idempotencyKey:string)=>request<FactoryRun>('factory_runs_start',{scopeId,expectedRevision,idempotencyKey}),
  run:(scopeId:string)=>request<FactoryRunDetails>('factory_runs_get',{scopeId}),
  cancel:(scopeId:string,expectedRevision:number,idempotencyKey:string)=>request<FactoryRun>('factory_runs_cancel',{scopeId,expectedRevision,idempotencyKey}),
};
