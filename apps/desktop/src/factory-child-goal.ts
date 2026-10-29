import type { Task } from './api';
import { safeInteger, utf8Bytes } from './goal-api';
import { factoryHash, factoryId, factorySame } from './factory-guards';
import { childRefShape } from './factory-child-plan';
import { graphDetailsShape, graphSummaryShape, graphSummaryMeetsFloor } from './goal-graph-guards';
import type { GraphEvidenceContext } from './goal-graph-types';
import type { FactoryScope, FactoryRunDetails, FactoryTaskHandle, FactoryChildPlanningContext, FactoryChildGoalSummary, FactoryChildGoalOriginView, FactoryChildGoalView, FactoryChildPlanRef } from './factory-types';
export const childGoalAuthority='factory_child_goal_execution_v1';
const object=(v:unknown):v is Record<string,unknown>=>v!==null&&typeof v==='object'&&!Array.isArray(v);
const keys=(v:object,names:string[])=>Object.keys(v).sort().join('|')===[...names].sort().join('|');
const optionalId=(v:unknown)=>v===null||factoryId(v);
export function childGoalReferenceShape(r:FactoryChildPlanRef,s:FactoryScope,target:FactoryTaskHandle,f:FactoryRunDetails):boolean {
 return !!r&&keys(r,['id','scope_id','run_id','task_id','revision','plan_digest','raw_sha256','receipt_sequence'])&&factoryId(r.id)&&r.scope_id===s.id&&r.run_id===f.run.id&&r.task_id===target.task_id&&safeInteger(r.revision,1,s.bindings.child_planning?.policy.max_revisions??0)&&factoryHash(r.plan_digest)&&factoryHash(r.raw_sha256)&&safeInteger(r.receipt_sequence,1);
}
export function childGoalSummaryShape(x:FactoryChildGoalSummary,s:FactoryScope,target:FactoryTaskHandle,f:FactoryRunDetails,c:FactoryChildPlanningContext|null,task:Task):boolean {
 return !!x&&keys(x,['authority_kind','factory_scope_id','factory_run_id','factory_root_task_id','parent_task_id','task_id','plan','execution_policy_digest','goal'])&&x.authority_kind===childGoalAuthority&&x.factory_scope_id===s.id&&x.factory_run_id===f.run.id&&x.factory_root_task_id===s.root_task_id&&x.parent_task_id===target.parent_task_id&&x.task_id===target.task_id&&task.id===target.task_id&&task.project_id===s.project_id&&childGoalReferenceShape(x.plan,s,target,f)&&factoryHash(x.execution_policy_digest)&&x.execution_policy_digest===s.bindings.child_goal_execution?.definition_digest&&graphSummaryShape(x.goal,task)&&x.goal.run.root_grant_id===s.root_grant_id;
}
export function childGoalHistoryShape(rows:FactoryChildGoalSummary[],s:FactoryScope,target:FactoryTaskHandle,f:FactoryRunDetails,c:FactoryChildPlanningContext|null,task:Task):boolean {
 const max=s.bindings.child_goal_execution?.policy.max_runs_per_task;
 return Array.isArray(rows)&&safeInteger(max,1,1)&&rows.length<=max!&&utf8Bytes(JSON.stringify(rows))<=1048576&&rows.every(row=>childGoalSummaryShape(row,s,target,f,c,task))&&new Set(rows.map(row=>row.goal.run.id)).size===rows.length&&rows.every((row,i)=>i===0||Date.parse(rows[i-1].goal.run.created_at)<=Date.parse(row.goal.run.created_at));
}
/** Compare the complete server origin to recorded authority; never construct or
 * transmit an origin/approval from renderer-side selections. */
export function childGoalOriginShape(o:FactoryChildGoalOriginView,s:FactoryScope,target:FactoryTaskHandle,f:FactoryRunDetails,c:FactoryChildPlanningContext):boolean {
 if(!o||!keys(o,['factory_scope_id','factory_scope_digest','factory_run_id','factory_root_task_id','parent_task_id','ancestors','original_native_approval_id','root_grant_id','root_grant_revision','initial_plan','execution_binding'])||o.factory_scope_id!==s.id||o.factory_scope_digest!==s.contract_digest||o.factory_run_id!==f.run.id||o.factory_root_task_id!==s.root_task_id||o.parent_task_id!==target.parent_task_id||!optionalId(o.parent_task_id)||o.original_native_approval_id!==s.approval_id||o.root_grant_id!==s.root_grant_id||o.root_grant_revision!==s.root_grant_revision||!factorySame(o.execution_binding,s.bindings.child_goal_execution)||!childGoalReferenceShape(o.initial_plan,s,target,f)||o.initial_plan.task_id!==target.task_id||o.initial_plan.run_id!==f.run.id||!Array.isArray(o.ancestors)||o.ancestors.length<1||o.ancestors.length>33||!o.ancestors.every(factoryId)||new Set(o.ancestors).size!==o.ancestors.length)return false;
 const parents=new Map(f.tasks.map(row=>[row.task_id,row.parent_task_id]));const chain:string[]=[];let current:string|null=target.task_id;
 while(current!==null){if(chain.includes(current)||!parents.has(current)||chain.length>32)return false;chain.push(current);current=parents.get(current)!;}
 return chain.at(-1)===s.root_task_id&&factorySame(o.ancestors,chain);
}
/** Evidence-only structural context, not a Native GoalScope or capability. */
export function childGoalEvidenceContext(summary:FactoryChildGoalSummary,c:FactoryChildPlanningContext):GraphEvidenceContext {
 return {id:summary.goal.run.scope_id,root_grant_id:summary.goal.run.root_grant_id,workspace_id:c.execution_bindings[0].workspace_id,goal:c.goal,input_revision:c.goal.input_revision,outputs:c.outputs,required_acceptance_rules:c.required_acceptance_rules,edit_context:c.edit_context};
}
export function childGoalViewShape(v:FactoryChildGoalView,s:FactoryScope,target:FactoryTaskHandle,f:FactoryRunDetails,c:FactoryChildPlanningContext,task:Task):boolean {
 if(!v||!keys(v,['summary','origin','details'])||utf8Bytes(JSON.stringify(v))>4194304||!childGoalSummaryShape(v.summary,s,target,f,c,task)||!childGoalOriginShape(v.origin,s,target,f,c)||v.origin.initial_plan.revision>v.summary.plan.revision||!safeInteger(task.revision,1)||typeof task.goal!=='string')return false;
 return graphDetailsShape(v.details,task,childGoalEvidenceContext(v.summary,c))&&factorySame(v.summary.goal,{graph_contract_version:v.details.graph_contract_version,run:v.details.run,ledger_revision:v.details.ledger_revision,task_accepted:v.details.task_accepted})&&v.details.run.plan_revision===v.summary.plan.revision&&v.details.run.plan_digest===v.summary.plan.plan_digest;
}
export function childGoalSummaryMeetsFloor(next:FactoryChildGoalSummary,prior:FactoryChildGoalSummary):boolean {
 return next.authority_kind===prior.authority_kind&&next.factory_scope_id===prior.factory_scope_id&&next.factory_run_id===prior.factory_run_id&&next.factory_root_task_id===prior.factory_root_task_id&&next.parent_task_id===prior.parent_task_id&&next.task_id===prior.task_id&&next.execution_policy_digest===prior.execution_policy_digest&&next.plan.revision>=prior.plan.revision&&(next.plan.revision!==prior.plan.revision||factorySame(next.plan,prior.plan))&&graphSummaryMeetsFloor(next.goal,prior.goal);
}
export function childGoalRefSame(a:FactoryChildPlanRef,b:FactoryChildPlanRef):boolean{return factorySame(a,b);}
export const childGoalScopeFingerprint=(s:FactoryScope)=>JSON.stringify([s.id,s.approval_id,s.contract_digest,s.root_grant_id,s.root_grant_revision,s.selection,s.bindings.child_planning,s.bindings.child_goal_execution]);
/** The displayed current Graph nodes must be the exact adopted saved proposal,
 * including Owner criteria/executor/input/output/budget fields. */
export function childGoalPlanMatchesGraph(v:FactoryChildGoalView,plan:{reference:FactoryChildPlanRef;proposal:{nodes:unknown[]}}):boolean {
 if(!childGoalRefSame(v.summary.plan,plan.reference))return false;
 const planned=new Map(plan.proposal.nodes.map(node=>{const n=node as {id:string};return [n.id,node] as const;}));
 const current=v.details.nodes.filter(row=>row.plan_revision===v.details.run.plan_revision);
 return current.length>=planned.size&&new Set(current.map(row=>row.node.id)).size===planned.size&&current.every(row=>planned.has(row.node.id)&&factorySame(row.node,planned.get(row.node.id)));
}
export function childGoalSavedReferencesMatch(v:FactoryChildGoalView,initial:{reference:FactoryChildPlanRef},adopted:{reference:FactoryChildPlanRef;proposal:{nodes:unknown[]}}):boolean {
 return childGoalRefSame(v.origin.initial_plan,initial.reference)&&childGoalRefSame(v.summary.plan,adopted.reference)&&childGoalPlanMatchesGraph(v,adopted);
}
