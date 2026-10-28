import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { goalApi, newGoalForm, parseGoalJson, parseGoalPlan, parseGoalSelection, type GoalBudget, type GoalPlanProposal, type GoalScope } from './goal-api';
import type { Task } from './api';
import type { ExecutionGrant } from './session-api';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
beforeEach(()=>{vi.resetAllMocks();vi.mocked(invoke).mockResolvedValue({result:{},warnings:[]});});
const date='2026-10-27T00:00:00Z';
function fixture(){
  const task:Task={id:'task',project_id:'project',goal:'Deliver report',state:'draft',revision:1,spec_revision:1,created_at:date,updated_at:date};
  const grant={id:'grant',project_id:'project',root_grant_id:'grant',parent_id:null,depth:0,revision:1,approval_id:'owner-grant',approved_at:date,expires_at_ms:Date.now()+60000,revoked:false,scope:{workspace_ids:['workspace'],connection_ids:['connection'],host_ids:['local'],max_attempts:8,max_concurrency:2,max_duration_ms:60000,recipients:[],network:{mode:'inherited'},max_depth:0,max_children:0},enforcement:{baton_mediated_scope:true,os_filesystem_sandbox:false,os_network_sandbox:false,native_identity_isolation:false}} satisfies ExecutionGrant;
  const budget:GoalBudget={attempts:8,parallel:2,runtime_ms:60000,output_bytes:8192,children:3,reworks:2,tokens:null,cost_microunits:null};
  const binding={connection_id:'connection',environment_id:'env',identity_ref:'identity',host_id:'local',workspace_id:'workspace',config_revision:'config',input_revision:'base',capability_snapshot_id:'caps',policy_revision:'grant:2'};
  const scope:GoalScope={id:'scope',revision:1,revoked:false,task_id:task.id,project_id:'project',root_grant_id:'grant',root_grant_revision:2,authority_revision:2,scope_digest:'s'.repeat(64),goal:{project_id:'project',root_task_id:task.id,task_id:task.id,spec_revision:1,input_revision:'base'},root_budget:budget,project_budget:budget,outputs:{report:'output:report'},required_acceptance_rules:['goal-owner-review'],contracts:{'output:report':['goal-owner-review'],'input:source':['goal-input-sha256']},inputs:{source:{contract_id:'input:source',sha256:'a'.repeat(64)}},binding,program_sha256:'b'.repeat(64),expires_at_ms:Date.now()+60000,rules:{'goal-owner-review':'c'.repeat(64),'goal-input-sha256':'d'.repeat(64)},workspace_id:'workspace',input_revision:'base',planning_only:true};
  const plan:GoalPlanProposal={schema_version:1,plan_id:'plan',goal:scope.goal,scope_revision:2,revision:1,expected_revision:0,acceptance_rules:scope.required_acceptance_rules,nodes:[{id:'work',parent_id:null,kind:'work',goal:'Write report',dependencies:[],workspace_id:'workspace',executor:{kind:'connection',binding,program_sha256:scope.program_sha256},recipients:[],required_inputs:[{goal_task_id:task.id,artifact_id:'source',contract_id:'input:source',source:{kind:'approved_external',sha256:'a'.repeat(64)}}],outputs:[{artifact_id:'report',contract_id:'output:report'}],required_validators:['goal-owner-review'],side_effect:'workspace_write',write_set:[{kind:'workspace',path:'outputs/report.txt'}],budget:{...budget,attempts:2,parallel:1,children:0},reservation:{runtime_ms:500,output_bytes:1024,tokens:null,cost_microunits:null},retry:{max_attempts:2,max_reworks:0,retry_classes:['before_dispatch_transient']},idempotency_key:'node-work',external_action_id:null}]};
  return {task,grant,scope,plan};
}
describe('Goal strict raw transport and proposal checks; mocked native transport',()=>{
  it('sends exact original plan text and outer CAS without normalization or authority flags',async()=>{
    const {scope,plan}=fixture(),raw=` \n${JSON.stringify(plan,null,2)}\n`;
    expect(parseGoalPlan(raw,scope,null)).toEqual(plan);await goalApi.propose(scope.id,raw,0,'same-key');
    expect(invoke).toHaveBeenCalledWith('goal_plans_propose',{scopeId:scope.id,planJson:raw,expectedRevision:0,idempotencyKey:'same-key'});expect(vi.mocked(invoke).mock.calls[0][1]).not.toHaveProperty('proposal');
    await goalApi.revise(scope.id,raw,1,'revise-key');expect(invoke).toHaveBeenLastCalledWith('goal_plans_revise',{scopeId:scope.id,planJson:raw,expectedRevision:1,idempotencyKey:'revise-key'});
  });
  it('separates native scope approval/read/revoke from plan history and readonly preview',async()=>{
    const {task,grant}=fixture(),form=newGoalForm();Object.assign(form,{grantId:grant.id,criteria:'Review output',cap:'1024',writable:'outputs'});form.outputs[0].path='report.txt';form.root.output_bytes='1024';form.project.output_bytes='1024';
    const selection=parseGoalSelection(form,task,grant);await goalApi.approve(selection,'approve-key');await goalApi.scopesList(task.id);await goalApi.scopeGet('scope');await goalApi.revoke('scope',1,'revoke-key');await goalApi.planGet('scope');await goalApi.plansList('scope');await goalApi.preview('scope',2);
    expect(invoke).toHaveBeenNthCalledWith(1,'goal_scope_approve',{selection,idempotencyKey:'approve-key'});expect(selection.expected_grant_revision).toBe(1);expect(selection.new_max_children).toBe(3);expect(selection.root_budget.tokens).toBeNull();
    expect(invoke).toHaveBeenLastCalledWith('goal_plans_ready_preview',{scopeId:'scope',expectedRevision:2});expect(JSON.stringify(vi.mocked(invoke).mock.calls[0])).not.toMatch(/verified|consumed|passed|finalize|approved_scope|identity_ref/);
    expect(Object.keys(goalApi)).not.toContain('dispatch');expect(Object.keys(goalApi)).not.toContain('reserve');expect(Object.keys(goalApi)).not.toContain('accept');
  });
  it.each(['{"a":1,"a":2}','{"n":{"x":1,"x":2}}','{"a":1,"\\u0061":2}'])('refuses duplicate JSON keys before they are lost: %s',text=>{expect(()=>parseGoalJson(text)).toThrow('duplicate');expect(invoke).not.toHaveBeenCalled();});
  it.each(['9007199254740993','9007199254740991.1','1e3','-1','NaN','Infinity'])('refuses unsafe/noninteger plan literal %s',number=>{expect(()=>parseGoalJson(`{"n":${number}}`)).toThrow();});
  it('refuses malformed/trailing JSON, excess depth and raw UTF8 byte bounds',()=>{for(const text of ['{"x":1,}','{}true','{"x":}','[1,]','\uFEFF{}'])expect(()=>parseGoalJson(text)).toThrow('json');expect(()=>parseGoalJson('['.repeat(50)+'0'+']'.repeat(50))).toThrow('limit');expect(()=>parseGoalJson('"'+'中'.repeat(90000)+'"')).toThrow('limit');});
  it('rejects nested unknown authority fields rather than dropping them',()=>{const {scope,plan}=fixture();for(const value of [{...plan,approved:true},{...plan,nodes:[{...plan.nodes[0],passed:true}]},{...plan,nodes:[{...plan.nodes[0],executor:{...plan.nodes[0].executor,verified:true}}]}])expect(()=>parseGoalPlan(JSON.stringify(value),scope,null)).toThrow('unknownField');});
  it('supports multiple versioned nodes, while external/production/remote proposals are never sent',()=>{
    const {scope,plan}=fixture(),work=plan.nodes[0];const review={...work,id:'review',kind:'owner_approval' as const,goal:'Review exact outcome',dependencies:['work'],executor:{kind:'owner' as const,rule_id:'goal-owner-review'},outputs:[],required_inputs:[{goal_task_id:scope.task_id,artifact_id:'report',contract_id:'output:report',source:{kind:'node' as const,node_id:'work',candidate_for_validation:true}}],side_effect:'advisory_read_only' as const,write_set:[],idempotency_key:'review-key'};
    const multiple={...plan,nodes:[work,review]};expect(parseGoalPlan(JSON.stringify(multiple),scope,null).nodes).toHaveLength(2);
    for(const side_effect of ['external_write','production'])expect(()=>parseGoalPlan(JSON.stringify({...plan,nodes:[{...work,side_effect}]}),scope,null)).toThrow('unsupported');
    const remote={...scope,binding:{...scope.binding,host_id:'remote'}};expect(()=>parseGoalPlan(JSON.stringify(plan),remote,null)).toThrow('unsupported');expect(invoke).not.toHaveBeenCalled();
  });
  it('pins plan/scope authority revision and CAS without changing the original plan identity',()=>{const {scope,plan}=fixture(),previous={scope_id:scope.id,proposal:plan,digest:'digest',created_at:date,state:'validated_proposal',dispatched:false};const revised={...plan,revision:2,expected_revision:1};expect(parseGoalPlan(JSON.stringify(revised),scope,previous)).toEqual(revised);for(const patch of [{plan_id:'different'},{scope_revision:1},{expected_revision:0},{goal:{...scope.goal,task_id:'other'}}])expect(()=>parseGoalPlan(JSON.stringify({...revised,...patch}),scope,previous)).toThrow('scope');});
  it('refuses scope grant expansion, traversal, unsafe integers and invalid validator references',()=>{
    const {task,grant}=fixture(),f=newGoalForm();f.grantId=grant.id;f.criteria='Review report';f.writable='outputs';f.outputs[0].path='report.txt';expect(parseGoalSelection(f,task,grant).expected_task_revision).toBe(1);
    expect(()=>parseGoalSelection({...f,maxNodes:'9007199254740993'},task,grant)).toThrow('integer');expect(()=>parseGoalSelection({...f,maxChildren:'0'},task,grant)).toThrow('limit');expect(()=>parseGoalSelection({...f,inputs:`../input|${'a'.repeat(64)}`},task,grant)).toThrow('path');
    expect(()=>parseGoalSelection(f,task,{...grant,project_id:'foreign'})).toThrow('scope');expect(()=>parseGoalSelection({...f,root:{...f.root,attempts:'9'}},task,grant)).toThrow('limit');
  });
});
