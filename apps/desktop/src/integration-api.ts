import { request } from './api';
import { parseIntegrationPolicy, parseIntegrationReconciliation, parseIntegrationRun, parseIntegrationRunView, parseIntegrationTargets, validateIntegrationSelection } from './integration-wire';
import type { IntegrationPolicySelection } from './integration-types';

const page={limit:100};
export const integrationApi={
 approve:async(selection:IntegrationPolicySelection,idempotencyKey:string)=>parseIntegrationPolicy(await request<unknown>('integration_policy_approve',{selection:validateIntegrationSelection(selection),idempotencyKey})),
 targets:async(taskId:string,afterRef?:string)=>parseIntegrationTargets(await request<unknown>('integration_targets_list',{taskId,...page,...(afterRef?{afterRef}:{})})),
 policy:async(policyId:string)=>parseIntegrationPolicy(await request<unknown>('integration_policies_get',{policyId})),
 policies:async(taskId:string,afterId?:string)=>{const rows=await request<unknown[]>('integration_policies_list',{taskId,...page,...(afterId?{afterId}:{})});if(!Array.isArray(rows)||rows.length>100)throw new Error('Invalid integration policy page');return rows.map(parseIntegrationPolicy);},
 revoke:async(policyId:string,expectedRevision:number,idempotencyKey:string)=>parseIntegrationPolicy(await request<unknown>('integration_policies_revoke',{policyId,expectedRevision,idempotencyKey})),
 start:async(policyId:string,expectedRevision:number,idempotencyKey:string)=>parseIntegrationRunView(await request<unknown>('integrations_start',{policyId,expectedRevision,idempotencyKey})),
 get:async(integrationId:string)=>parseIntegrationRunView(await request<unknown>('integrations_get',{integrationId})),
 list:async(taskId:string,afterId?:string)=>{const rows=await request<unknown[]>('integrations_list',{taskId,...page,...(afterId?{afterId}:{})});if(!Array.isArray(rows)||rows.length>100)throw new Error('Invalid integration run page');return rows.map(parseIntegrationRun);},
 cancel:async(integrationId:string,expectedRevision:number,idempotencyKey:string)=>parseIntegrationRunView(await request<unknown>('integrations_cancel',{integrationId,expectedRevision,idempotencyKey})),
 reconcile:async(integrationId:string,expectedRevision:number,idempotencyKey:string)=>parseIntegrationReconciliation(await request<unknown>('integrations_reconcile',{integrationId,expectedRevision,idempotencyKey})),
};
