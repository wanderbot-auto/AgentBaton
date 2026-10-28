import type { Task } from './api';
import type { ExecutionGrant } from './session-api';
import { safeInteger, type GoalBudget } from './goal-api';
import type { ArtifactRequirement } from './validation-api';
import type { FactorySelection } from './factory-types';
import { factoryBudget, factoryHash, factoryId, factoryRelative, factorySelectionShape } from './factory-guards';
export type FactoryBudgetForm={ [K in keyof GoalBudget]:string };
export interface FactoryForm { grantId:string; connections:string[]; workspaces:string[]; inputs:string; outputs:string; recipients:string; maxDepth:string; maxChildren:string; maxNodes:string; ttlMs:string; delegate:boolean; resume:boolean; root:FactoryBudgetForm; project:FactoryBudgetForm }
const budget=():FactoryBudgetForm=>({attempts:'3',parallel:'1',runtime_ms:'120000',output_bytes:'8388608',children:'1',reworks:'0',tokens:'',cost_microunits:''});
export const newFactoryForm=():FactoryForm=>({grantId:'',connections:[],workspaces:[],inputs:'',outputs:'',recipients:'',maxDepth:'1',maxChildren:'1',maxNodes:'2',ttlMs:'300000',delegate:true,resume:true,root:budget(),project:budget()});
const number=(raw:string,min=0,max=Number.MAX_SAFE_INTEGER)=>{if(!/^\d+$/.test(raw))throw new Error('Use bounded whole numbers');const value=Number(raw);if(!safeInteger(value,min,max))throw new Error('Use bounded safe whole numbers');return value;};
const parseBudget=(form:FactoryBudgetForm):GoalBudget=>{
 const value={attempts:number(form.attempts,1),parallel:number(form.parallel,1),runtime_ms:number(form.runtime_ms,1),output_bytes:number(form.output_bytes,1),children:number(form.children),reworks:number(form.reworks),tokens:form.tokens===''?null:number(form.tokens),cost_microunits:form.cost_microunits===''?null:number(form.cost_microunits)};
 if(!factoryBudget(value))throw new Error('Invalid bounded budget');return value;
};
export function parseFactoryArtifacts(raw:string):ArtifactRequirement[]{
 const lines=raw.split(/\r?\n/).map(s=>s.trim()).filter(Boolean);if(lines.length>64)throw new Error('Too many selected artifacts');
 const ids=new Set<string>(),paths=new Set<string>();
 return lines.map(line=>{const fields=line.split('|').map(s=>s.trim());if(fields.length<3||fields.length>4)throw new Error('Use artifact-id | relative/path | max-bytes | optional sha256');const [artifact_id,relative_path,bytes,sha='']=fields;
 if(!factoryId(artifact_id)||!factoryRelative(relative_path)||ids.has(artifact_id)||paths.has(relative_path)||(sha!==''&&!factoryHash(sha)))throw new Error('Invalid, duplicate or unsafe artifact reference');ids.add(artifact_id);paths.add(relative_path);return {artifact_id,relative_path,max_bytes:number(bytes,1),expected_sha256:sha||null};});
}
export function parseFactorySelection(form:FactoryForm,task:Task,grant:ExecutionGrant):FactorySelection{
 if(form.grantId!==grant.id||grant.project_id!==task.project_id||grant.root_grant_id!==grant.id||grant.parent_id!==null||grant.revoked||grant.expires_at_ms<=Date.now()||form.connections.some(id=>!grant.scope.connection_ids.includes(id))||form.workspaces.some(id=>!grant.scope.workspace_ids.includes(id)))throw new Error('Select a current root Grant and its existing recipients/workspaces');
 const recipients=form.recipients.split(/\r?\n/).map(s=>s.trim()).filter(Boolean);if(recipients.some(id=>!grant.scope.recipients.includes(id)))throw new Error('Recipient is outside the existing Grant');
 const result:FactorySelection={root_task_id:task.id,expected_task_revision:task.revision,grant_id:grant.id,expected_grant_revision:grant.revision,connection_ids:[...form.connections],workspace_ids:[...form.workspaces],inputs:parseFactoryArtifacts(form.inputs),outputs:parseFactoryArtifacts(form.outputs),recipients,max_depth:number(form.maxDepth),max_children:number(form.maxChildren),max_total_nodes:number(form.maxNodes,1),root_budget:parseBudget(form.root),project_budget:parseBudget(form.project),ttl_ms:number(form.ttlMs,1),capabilities:{delegate_task:form.delegate,resume_parent_as_new_attempt:form.resume,child_planning:false,child_goal_execution:false,create_workspace:false}};
 if(!factorySelectionShape(result)||result.connection_ids.length>4||result.workspace_ids.length!==1||result.inputs.length<1||result.inputs.length>16||result.outputs.length<1||result.outputs.length>16||result.recipients.length>64||result.max_depth<1||result.max_depth>32||result.max_total_nodes<2||result.max_total_nodes>1000||result.max_children<1||result.max_children>=result.max_total_nodes||result.ttl_ms>3600000||!result.capabilities.delegate_task||!result.capabilities.resume_parent_as_new_attempt)throw new Error('This Factory stage requires bounded existing-workspace delegation and new-attempt handoff');
 const all=[...result.inputs,...result.outputs];if(new Set(all.map(a=>a.artifact_id)).size!==all.length||all.some(a=>a.max_bytes>67108864))throw new Error('Artifact IDs must be unique and each selected file bounded to 64 MiB');
 const fields=['attempts','parallel','runtime_ms','output_bytes','children','reworks'] as const;
 if(fields.some(k=>result.root_budget[k]>result.project_budget[k])||result.root_budget.attempts>grant.scope.max_attempts||result.root_budget.parallel>grant.scope.max_concurrency||result.root_budget.runtime_ms>grant.scope.max_duration_ms*grant.scope.max_attempts||result.root_budget.children>result.max_total_nodes-1||[result.root_budget,result.project_budget].some(b=>b.reworks!==0||b.tokens!==null||b.cost_microunits!==null))throw new Error('Fit the existing shared Grant; automatic rework and unmeasured token/cost enforcement are unsupported');return result;
}
