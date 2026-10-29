import { safeInteger, parseGoalJson, utf8Bytes, GoalFormError, type GoalScope, type GoalPlan, type GoalPlanProposal } from './goal-api';
import { hasChildEditorContext, validateChildEditorPlan } from './FactoryChildPlanEditor';
import { factoryBudget, factoryChildPolicyShape, factoryHash, factoryId, factorySame, factorySafeFingerprint } from './factory-guards';
import type { FactoryScope, FactoryTaskHandle, FactoryChildPlanningContext, FactoryChildPlanView, FactoryChildPlanRef } from './factory-types';
const object=(v:unknown):v is Record<string,unknown>=>v!==null&&typeof v==='object'&&!Array.isArray(v);
const ids=(v:unknown):v is string[]=>Array.isArray(v)&&v.length<=256&&v.every(factoryId)&&new Set(v).size===v.length;
const bounded=(v:unknown)=>utf8Bytes(JSON.stringify(v))<=1048576;
const fail=():never=>{throw new Error('Foreign, stale or inconsistent child-plan evidence');};
/** Editor-only projection of an already recorded context. No approval/run API
 * consumes it, and it cannot construct any Core private authority. */
export function childEditorScope(c:FactoryChildPlanningContext,s:FactoryScope):GoalScope {
 const binding=c.execution_bindings[0];
 return {id:c.scope_id,revision:c.scope_revision,revoked:s.revoked,task_id:c.goal.task_id,project_id:c.goal.project_id,root_grant_id:s.root_grant_id,root_grant_revision:s.root_grant_revision,authority_revision:c.scope_revision,scope_digest:c.planning_policy_digest,goal:c.goal,root_budget:c.budget,project_budget:c.project_budget,outputs:c.outputs,required_acceptance_rules:c.required_acceptance_rules,contracts:c.contracts,inputs:c.inputs,binding,program_sha256:c.program_sha256[binding.connection_id],expires_at_ms:s.expires_at_ms,rules:c.rules,workspace_id:binding.workspace_id,input_revision:c.goal.input_revision,planning_only:true,edit_context:c.edit_context};
}
export function childEditorLatest(v:FactoryChildPlanView|null):GoalPlan|null{return v?{scope_id:v.reference.scope_id,proposal:v.proposal,digest:v.reference.plan_digest,created_at:v.created_at,state:'validated_proposal',dispatched:false}:null;}
export function childContextShape(c:FactoryChildPlanningContext,s:FactoryScope,target:FactoryTaskHandle):boolean {
 try{
  const approved=s.bindings.child_planning;
  if(!c||!bounded(c)||!approved||!s.selection.capabilities.child_planning||!factoryChildPolicyShape(c.policy)||!factorySame(c.policy,approved.policy)||c.scope_id!==s.id||c.scope_digest!==s.contract_digest||c.scope_revision!==1||c.planning_policy_digest!==approved.definition_digest||!factoryId(c.run_id)||c.goal?.task_id!==target.task_id||c.goal.root_task_id!==s.root_task_id||c.goal.project_id!==s.project_id||!safeInteger(c.goal.spec_revision,1)||typeof c.goal.input_revision!=='string'||!c.goal.input_revision||c.planning_only!==true||c.goal_execution_authorized!==false||c.new_workspace_authorized!==false||!safeInteger(c.current_plan_revision,0,c.policy.max_revisions)||!factoryBudget(c.budget)||!factoryBudget(c.project_budget)||!Array.isArray(c.execution_bindings)||c.execution_bindings.length!==1||!object(c.program_sha256)||!object(c.inputs)||!object(c.outputs)||!object(c.rules)||!object(c.contracts)||!ids(c.required_acceptance_rules))return false;
  const b=c.execution_bindings[0],connection=s.bindings.connections.find(v=>v.id===b?.connection_id);
  if(!connection||!s.selection.workspace_ids.includes(b.workspace_id)||Object.keys(c.program_sha256).length!==1||c.program_sha256[b.connection_id]!==connection.executable_sha256||b.environment_id!==connection.environment_id||b.identity_ref!==connection.identity_ref||b.host_id!==connection.host_id||b.config_revision!==connection.config_revision||b.capability_snapshot_id!==connection.capability_snapshot_id||b.policy_revision!==`${s.root_grant_id}:${s.root_grant_revision}`||b.input_revision!==c.goal.input_revision)return false;
  if(Object.entries(c.inputs).some(([id,input])=>!factoryId(id)||!factoryId(input?.contract_id)||!factoryHash(input?.sha256)||!s.bindings.inputs.some(a=>a.contract.requirement.artifact_id===id&&a.sha256===input.sha256)))return false;
  if(Object.entries(c.outputs).some(([id,ref])=>!factoryId(id)||!factoryId(ref)||!c.policy.outputs.some(a=>a.artifact.artifact_id===id)||!Array.isArray(c.contracts[ref])))return false;
  if(Object.values(c.rules).some(v=>!factoryHash(v))||Object.values(c.contracts).some(v=>!ids(v)||v.some(id=>!Object.hasOwn(c.rules,id)))||c.required_acceptance_rules.some(id=>!Object.hasOwn(c.rules,id))||!c.required_acceptance_rules.includes(approved.owner_rule_id))return false;
  const editor=childEditorScope(c,s);
  return hasChildEditorContext(editor)&&c.edit_context.max_nodes===c.policy.max_nodes&&Object.values(c.edit_context.output_artifacts).every(a=>c.policy.outputs.some(v=>factorySame(a,v.artifact)))&&c.edit_context.writable_prefixes.every(p=>c.policy.writable_prefixes.includes(p));
 }catch{return false;}
}
export function parseFactoryChildPlan(raw:string,c:FactoryChildPlanningContext,s:FactoryScope,latest:FactoryChildPlanView|null):GoalPlanProposal {
 if(utf8Bytes(raw)>c.policy.max_plan_bytes||(latest?.reference.revision??0)>=c.policy.max_revisions)throw new GoalFormError('limit');
 const value=parseGoalJson(raw);
 if(!object(value)||!Array.isArray(value.nodes))return fail();
 return validateChildEditorPlan(value as unknown as GoalPlanProposal,childEditorScope(c,s),childEditorLatest(latest),s.selection.recipients);
}
export function childViewShape(v:FactoryChildPlanView,c:FactoryChildPlanningContext,s:FactoryScope):boolean {
 try{
  const r=v?.reference;
  if(!v||!bounded(v)||!r||!factoryId(r.id)||r.scope_id!==s.id||r.run_id!==c.run_id||r.task_id!==c.goal.task_id||!safeInteger(r.revision,1,c.policy.max_revisions)||!factoryHash(r.plan_digest)||!factoryHash(r.raw_sha256)||!safeInteger(r.receipt_sequence,1)||v.authority_kind!=='factory_child_planning_only'||v.scope_digest!==s.contract_digest||v.planning_policy_digest!==c.planning_policy_digest||v.root_task_id!==s.root_task_id||v.root_grant_id!==s.root_grant_id||v.root_grant_revision!==s.root_grant_revision||!factoryId(v.caller_task_id)||v.target_spec_revision!==c.goal.spec_revision||v.input_revision!==c.goal.input_revision||typeof v.created_at!=='string'||!Number.isFinite(Date.parse(v.created_at))||v.planning_only!==true||v.dispatched!==false||v.task_accepted!==false||typeof v.plan_json!=='string'||!factorySame(parseGoalJson(v.plan_json),v.proposal))return false;
  const owner=v.caller_session_id===null&&v.caller_attempt_id===null&&v.caller_fencing_token===null;
  if(owner?v.caller_task_id!==r.task_id:!factoryId(v.caller_session_id)||!factoryId(v.caller_attempt_id)||!safeInteger(v.caller_fencing_token,1))return false;
  if(v.proposal.revision!==r.revision||v.proposal.expected_revision!==r.revision-1)return false;
  // Validate historical revision as that immutable revision, never as a new
  // current head or execution permission.
  const previous=r.revision===1?null:{...v,reference:{...r,revision:r.revision-1},proposal:{...v.proposal,revision:r.revision-1}};
  parseFactoryChildPlan(v.plan_json,c,s,previous);
  return true;
 }catch{return false;}
}
export async function verifyChildRaw(v:FactoryChildPlanView):Promise<void>{
 if(!globalThis.crypto?.subtle)throw new Error('SHA-256 verification unavailable');
 const raw=v.plan_json,expected=v.reference.raw_sha256,projection=childViewFingerprint(v);
 const bytes=await crypto.subtle.digest('SHA-256',new TextEncoder().encode(raw));
 const hash=Array.from(new Uint8Array(bytes),b=>b.toString(16).padStart(2,'0')).join('');
 if(hash!==expected||v.plan_json!==raw||childViewFingerprint(v)!==projection)fail();
}
export function childRefShape(r:FactoryChildPlanRef,c:FactoryChildPlanningContext):boolean {
 return !!r&&factoryId(r.id)&&r.scope_id===c.scope_id&&r.run_id===c.run_id&&r.task_id===c.goal.task_id&&safeInteger(r.revision,1,c.policy.max_revisions)&&factoryHash(r.plan_digest)&&factoryHash(r.raw_sha256)&&safeInteger(r.receipt_sequence,1);
}
export function childHistoryShape(rows:FactoryChildPlanRef[],c:FactoryChildPlanningContext):boolean {
 return Array.isArray(rows)&&rows.length<=c.policy.max_revisions&&bounded(rows)&&rows.length===c.current_plan_revision&&rows.every((v,i)=>childRefShape(v,c)&&v.revision===i+1&&(i===0||v.receipt_sequence>rows[i-1].receipt_sequence))&&new Set(rows.map(v=>v.id)).size===rows.length;
}

export const childViewFingerprint=(v:FactoryChildPlanView)=>factorySafeFingerprint(v);
