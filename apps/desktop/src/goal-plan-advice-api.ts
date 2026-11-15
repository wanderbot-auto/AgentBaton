import {request,type Task} from './api';
import {parseGoalJson,safeInteger,utf8Bytes,type GoalScope,type GoalBudget} from './goal-api';
import {hasEditorContext} from './GoalPlanEditor';
import type {GoalPlanDraft} from './goal-plan-draft-api';
export interface GoalPlanAdviceQuery {scope_id:string;task_id:string;expected_task_revision:number;expected_spec_revision:number;expected_scope_revision:number;expected_scope_digest:string;expected_authority_revision:number}
export interface GoalAdviceStep {id:string;goal:string;dependencies:string[];input_artifact_ids:string[];output_artifact_ids:string[]}
type Source=GoalPlanDraft['source'];
export interface GoalAdviceContext {source:Source;approved_acceptance_criteria:string;inputs:GoalScope['inputs'];outputs:Array<{artifact_id:string;contract_id:string;relative_path:string;max_bytes:number;expected_sha256:string|null}>;writable_prefixes:string[];root_budget:GoalBudget;project_budget:GoalBudget;max_nodes:number;planning_only:true;dispatched:false}
export interface GoalAdviceImport {source:Source;steps:GoalAdviceStep[];planning_only:true;dispatched:false}
const fail=():never=>{throw new Error('Invalid planning advice');};
function row(value:unknown,keys:string[]):Record<string,unknown>{if(!value||typeof value!=='object'||Array.isArray(value))return fail();const r=value as Record<string,unknown>;if(Object.keys(r).length!==keys.length||keys.some(k=>!Object.hasOwn(r,k)))return fail();return r;}
function same(a:unknown,b:unknown):boolean{if(a===b)return true;if(!a||!b||typeof a!=='object'||typeof b!=='object'||Array.isArray(a)!==Array.isArray(b))return false;const x=a as Record<string,unknown>,y=b as Record<string,unknown>;return Object.keys(x).length===Object.keys(y).length&&Object.keys(x).every(k=>Object.hasOwn(y,k)&&same(x[k],y[k]));}
const id=(s:unknown):s is string=>typeof s==='string'&&/^[A-Za-z0-9._:-]{1,256}$/.test(s);
const ids=(v:unknown):v is string[]=>Array.isArray(v)&&v.length<=256&&v.every(id)&&new Set(v).size===v.length;
const sorted=(values:string[])=>[...new Set(values)].sort();
export function adviceQuery(task:Task,scope:GoalScope):GoalPlanAdviceQuery {
 if(!hasEditorContext(scope)||!safeInteger(task.revision,1)||!safeInteger(task.spec_revision,1)||task.id!==scope.task_id||task.project_id!==scope.project_id||task.spec_revision!==scope.goal.spec_revision||!safeInteger(scope.revision,1)||!safeInteger(scope.authority_revision,1)||!/^[a-f0-9]{64}$/.test(scope.scope_digest))return fail();
 return {scope_id:scope.id,task_id:task.id,expected_task_revision:task.revision,expected_spec_revision:task.spec_revision,expected_scope_revision:scope.revision,expected_scope_digest:scope.scope_digest,expected_authority_revision:scope.authority_revision};
}
function source(task:Task,scope:GoalScope):Source{return {task_id:task.id,task_revision:task.revision,spec_revision:task.spec_revision!,task_goal:task.goal,scope_id:scope.id,scope_revision:scope.revision,scope_digest:scope.scope_digest,authority_revision:scope.authority_revision};}
export function parseAdviceContext(value:unknown,task:Task,scope:GoalScope):GoalAdviceContext {
 adviceQuery(task,scope);const c=row(value,['source','approved_acceptance_criteria','inputs','outputs','writable_prefixes','root_budget','project_budget','max_nodes','planning_only','dispatched']);
 if(!same(c.source,source(task,scope))||c.planning_only!==true||c.dispatched!==false||typeof c.approved_acceptance_criteria!=='string'||!c.approved_acceptance_criteria.trim()||utf8Bytes(c.approved_acceptance_criteria)>2048||!same(c.inputs,scope.inputs)||!same(c.root_budget,scope.root_budget)||!same(c.project_budget,scope.project_budget)||!same(c.writable_prefixes,scope.edit_context!.writable_prefixes)||c.max_nodes!==scope.edit_context!.max_nodes||!Array.isArray(c.outputs)||c.outputs.length!==Object.keys(scope.outputs).length)return fail();
 const seen=new Set<string>();for(const item of c.outputs){const o=row(item,['artifact_id','contract_id','relative_path','max_bytes','expected_sha256']);if(!id(o.artifact_id)||seen.has(o.artifact_id))return fail();seen.add(o.artifact_id);const expected={...scope.edit_context!.output_artifacts[o.artifact_id],contract_id:scope.outputs[o.artifact_id]};if(!same(o,expected))return fail();}
 return c as unknown as GoalAdviceContext;
}
export function normalizeAdviceSteps(value:unknown,context:GoalAdviceContext):GoalAdviceStep[]{
 if(!Array.isArray(value)||value.length<1||value.length>Math.min(8,context.max_nodes,context.outputs.length))return fail();
 const steps:GoalAdviceStep[]=value.map(v=>{const s=row(v,['id','goal','dependencies','input_artifact_ids','output_artifact_ids']);if(!id(s.id)||s.id.startsWith('baton-review-')||typeof s.goal!=='string'||!s.goal.trim()||s.goal.includes('\0')||utf8Bytes(s.goal)>4096||!ids(s.dependencies)||!ids(s.input_artifact_ids)||!ids(s.output_artifact_ids)||!s.output_artifact_ids.length)return fail();return s as unknown as GoalAdviceStep;});
 if(new Set(steps.map(s=>s.id)).size!==steps.length)return fail();const producers=new Map<string,string>();for(const step of steps)for(const output of step.output_artifact_ids){if(!context.outputs.some(o=>o.artifact_id===output)||producers.has(output))return fail();producers.set(output,step.id);}if(producers.size!==context.outputs.length)return fail();
 const normalized=steps.map(s=>{const dependencies=[...s.dependencies];if(dependencies.some(d=>!steps.some(other=>other.id===d)))return fail();for(const input of s.input_artifact_ids){if(Object.hasOwn(context.inputs,input))continue;const producer=producers.get(input);if(!producer)return fail();dependencies.push(producer);}if(dependencies.includes(s.id))return fail();return {...s,dependencies:sorted(dependencies)};});
 const edges=new Map(normalized.map(s=>[s.id,new Set(s.dependencies)]));while(edges.size){const ready=[...edges].filter(([,deps])=>!deps.size).map(([id])=>id);if(!ready.length)return fail();for(const id of ready){edges.delete(id);for(const deps of edges.values())deps.delete(id);}}
 return normalized;
}
export function parseAdviceImport(value:unknown,raw:string,context:GoalAdviceContext):GoalAdviceImport {
 if(utf8Bytes(raw)>65536)return fail();const envelope=row(parseGoalJson(raw),['schema_version','source','steps']);if(envelope.schema_version!==1||!same(envelope.source,context.source))return fail();const expected=normalizeAdviceSteps(envelope.steps,context);
 const imported=row(value,['source','steps','planning_only','dispatched']);if(!same(imported.source,context.source)||imported.planning_only!==true||imported.dispatched!==false||!same(imported.steps,expected))return fail();
 return imported as unknown as GoalAdviceImport;
}
export function advicePrompt(context:GoalAdviceContext):string{return 'This local export does not send anything or restrict external Agent permissions. Review where you copy the task text and approved paths.\n\nDo not execute the work, run commands, or modify files; return suggestions only.\n\nSuggest 1–8 work steps for the recorded task within the supplied approved scope. Return only the JSON envelope shown below. Use only listed input/output artifact IDs; each output needs one producer. Do not include budgets, risks, prohibited actions, permissions, executors, review gates, or acceptance results. Your suggestions are untrusted and require user review.\n\nApproved context:\n'+JSON.stringify(context,null,2)+'\n\nRequired output shape (replace the placeholder step; preserve source exactly):\n'+JSON.stringify({schema_version:1,source:context.source,steps:[{id:'step-1',goal:'Describe the work',dependencies:[],input_artifact_ids:[],output_artifact_ids:[]}]},null,2);}
export const goalPlanAdviceApi={
 context:async(query:GoalPlanAdviceQuery,task:Task,scope:GoalScope)=>{if(!same(query,adviceQuery(task,scope)))return fail();return parseAdviceContext(await request<unknown>('goal_plan_advice_context',{request:query}),task,scope);},
 import:async(query:GoalPlanAdviceQuery,raw:string,context:GoalAdviceContext)=>{if(!same(query,{scope_id:context.source.scope_id,task_id:context.source.task_id,expected_task_revision:context.source.task_revision,expected_spec_revision:context.source.spec_revision,expected_scope_revision:context.source.scope_revision,expected_scope_digest:context.source.scope_digest,expected_authority_revision:context.source.authority_revision})||!raw.trim()||utf8Bytes(raw)>65536)return fail();return parseAdviceImport(await request<unknown>('goal_plan_advice_import',{request:{query,candidate_json:raw}}),raw,context);},
};
