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
function editorProjection(c:FactoryChildPlanningContext,s:FactoryScope):GoalScope {
 const binding=c.execution_bindings[0];
 return {id:c.scope_id,revision:c.scope_revision,revoked:s.revoked,task_id:c.goal.task_id,project_id:c.goal.project_id,root_grant_id:s.root_grant_id,root_grant_revision:s.root_grant_revision,authority_revision:c.scope_revision,scope_digest:c.planning_policy_digest,goal:c.goal,root_budget:c.budget,project_budget:c.project_budget,outputs:c.outputs,required_acceptance_rules:c.required_acceptance_rules,contracts:c.contracts,inputs:c.inputs,binding,program_sha256:c.program_sha256[binding.connection_id],expires_at_ms:s.expires_at_ms,rules:c.rules,workspace_id:binding.workspace_id,input_revision:c.goal.input_revision,planning_only:true,edit_context:c.edit_context};
}
export function childEditorScope(c:FactoryChildPlanningContext,s:FactoryScope):GoalScope {
 if(!contextMatchesApproval(c,s))return fail();
 return editorProjection(c,s);
}
export function childEditorLatest(v:FactoryChildPlanView|null):GoalPlan|null{return v?{scope_id:v.reference.scope_id,proposal:v.proposal,digest:v.reference.plan_digest,created_at:v.created_at,state:'validated_proposal',dispatched:false}:null;}
function contextMatchesApproval(c:FactoryChildPlanningContext,s:FactoryScope):boolean {
 try{
  const approved=s.bindings.child_planning;
  if(!c||!bounded(c)||!approved||!s.selection.capabilities.child_planning||!factoryChildPolicyShape(c.policy)||!factorySame(c.policy,approved.policy)||c.scope_id!==s.id||c.scope_digest!==s.contract_digest||c.scope_revision!==1||c.planning_policy_digest!==approved.definition_digest||!factoryId(c.run_id)||!factoryId(c.goal?.task_id)||c.goal.root_task_id!==s.root_task_id||c.goal.project_id!==s.project_id||!safeInteger(c.goal.spec_revision,1)||typeof c.goal.input_revision!=='string'||!c.goal.input_revision||c.planning_only!==true||c.goal_execution_authorized!==false||c.new_workspace_authorized!==false||!safeInteger(c.current_plan_revision,0,c.policy.max_revisions)||!factoryBudget(c.budget)||!factoryBudget(c.project_budget)||!Array.isArray(c.execution_bindings)||c.execution_bindings.length!==1||!object(c.program_sha256)||!object(c.inputs)||!object(c.outputs)||!object(c.rules)||!object(c.contracts)||!ids(c.required_acceptance_rules))return false;
  // These are the independently approved ceilings and pinned input version,
  // not merely well-formed values returned by a context response.
  const fields=['attempts','parallel','runtime_ms','output_bytes','children','reworks'] as const;
  const optionalWithin=(v:number|null,ceiling:number|null)=>ceiling===null?v===null:v!==null&&v<=ceiling;
  if(fields.some(k=>c.budget[k]>s.selection.root_budget[k])||!optionalWithin(c.budget.tokens,s.selection.root_budget.tokens)||!optionalWithin(c.budget.cost_microunits,s.selection.root_budget.cost_microunits)||!factorySame(c.project_budget,s.selection.project_budget))return false;
  const b=c.execution_bindings[0],workspace=s.bindings.workspaces.find(w=>w.id===b?.workspace_id),connection=s.bindings.connections.find(v=>v.id===b?.connection_id);
  if(!connection||!workspace||c.goal.input_revision!==workspace.base_revision||!s.selection.workspace_ids.includes(b.workspace_id)||Object.keys(c.program_sha256).length!==1||c.program_sha256[b.connection_id]!==connection.executable_sha256||b.environment_id!==connection.environment_id||b.identity_ref!==connection.identity_ref||b.host_id!==connection.host_id||b.config_revision!==connection.config_revision||b.capability_snapshot_id!==connection.capability_snapshot_id||b.policy_revision!==`${s.root_grant_id}:${s.root_grant_revision}`||b.input_revision!==c.goal.input_revision)return false;
  if(Object.entries(c.inputs).some(([id,input])=>!factoryId(id)||!factoryId(input?.contract_id)||!factoryHash(input?.sha256)||!s.bindings.inputs.some(a=>a.contract.requirement.artifact_id===id&&a.sha256===input.sha256)))return false;
  if(c.goal.task_id===s.root_task_id&&(c.goal.spec_revision!==s.bindings.root_task.spec_revision||Object.keys(c.inputs).length!==s.bindings.inputs.length||s.bindings.inputs.some(a=>!Object.hasOwn(c.inputs,a.contract.requirement.artifact_id))))return false;
  if(Object.keys(c.outputs).length===0)return false;
  if(Object.entries(c.outputs).some(([id,ref])=>!factoryId(id)||!factoryId(ref)||!c.policy.outputs.some(a=>a.artifact.artifact_id===id)||!Array.isArray(c.contracts[ref])))return false;
  if(Object.values(c.rules).some(v=>!factoryHash(v))||Object.values(c.contracts).some(v=>!ids(v)||v.some(id=>!Object.hasOwn(c.rules,id)))||c.required_acceptance_rules.some(id=>!Object.hasOwn(c.rules,id))||!c.required_acceptance_rules.includes(approved.owner_rule_id))return false;
  // A context cannot erase a Native-selected validator by coherently changing
  // its own rules/contracts/editor maps. Resolve those maps from the approved
  // policy references and their complete, already reviewed definitions.
  if(!object(approved.validator_definitions)||Object.keys(approved.validator_definitions).length>254||!object(c.edit_context?.rule_executors))return false;
  const definitions=Object.entries(approved.validator_definitions);
  for(const [id,v] of definitions){
   if(!object(v)||v.rule_id!==id||!factoryId(id)||!factoryHash(v.digest)||!object(v.reference)||Object.keys(v.reference).length!==3||!['integrity','data','command'].includes(String(v.reference.kind))||!factoryId(v.reference.policy_id)||!safeInteger(v.reference.revision,1))return false;
  }
  const sameIds=(a:string[],b:string[])=>a.length===b.length&&new Set(a).size===a.length&&a.every(id=>b.includes(id));
  const expectedRules=new Set([approved.owner_rule_id,approved.input_rule_id]);
  const expectedContracts:string[]=[];
  if(!sameIds(c.required_acceptance_rules,[approved.owner_rule_id])||c.edit_context.rule_executors[approved.owner_rule_id]!=='owner'||c.edit_context.rule_executors[approved.input_rule_id]!=='validator')return false;
  for(const [id,input] of Object.entries(c.inputs)){
   if(input.contract_id!==`input:${id}`||!sameIds(c.contracts[input.contract_id]??[],[approved.input_rule_id]))return false;
   expectedContracts.push(input.contract_id);
  }
  for(const [id,contract] of Object.entries(c.outputs)){
   if(contract!==`output:${id}`)return false;
   const output=c.policy.outputs.find(o=>o.artifact.artifact_id===id)!;
   const required=new Set([approved.owner_rule_id]);
   for(const ref of output.validators){
    const matches=definitions.filter(([,v])=>object(v)&&factorySame(v.reference,ref));
    if(matches.length!==1)return false;
    const [rule,v]=matches[0];if(!object(v)||c.rules[rule]!==v.digest||c.edit_context.rule_executors[rule]!=='validator')return false;
    required.add(rule);expectedRules.add(rule);
   }
   if(!sameIds(c.contracts[contract],[...required]))return false;
   expectedContracts.push(contract);
  }
  if(!sameIds(Object.keys(c.rules),[...expectedRules])||!sameIds(Object.keys(c.contracts),expectedContracts))return false;
  const editor=editorProjection(c,s);
  return hasChildEditorContext(editor)&&c.edit_context.max_nodes===c.policy.max_nodes&&Object.values(c.edit_context.output_artifacts).every(a=>c.policy.outputs.some(v=>factorySame(a,v.artifact)))&&c.edit_context.writable_prefixes.every(p=>c.policy.writable_prefixes.includes(p));
 }catch{return false;}
}
export function childContextShape(c:FactoryChildPlanningContext,s:FactoryScope,target:FactoryTaskHandle):boolean {
 return !!s&&!!c&&!!target&&target.factory_scope_id===s.id&&target.root_task_id===s.root_task_id&&c?.goal?.task_id===target.task_id&&contextMatchesApproval(c,s);
}
export function parseFactoryChildPlan(raw:string,c:FactoryChildPlanningContext,s:FactoryScope,latest:FactoryChildPlanView|null):GoalPlanProposal {
 if(!contextMatchesApproval(c,s))return fail();
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
