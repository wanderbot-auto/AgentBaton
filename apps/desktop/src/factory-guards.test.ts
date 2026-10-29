import { describe,it,expect,vi } from 'vitest';
import { factoryFixture } from './factory-test-fixtures';
import { childGoalFixture } from './factory-child-goal-test-fixtures';
import { factoryScopeShape,factoryDetailsShape,factoryRunSupported,factoryCanCancel,factorySame } from './factory-guards';
import { parseFactoryArtifacts,parseFactorySelection } from './factory-form';
describe('Factory typed guards, synthetic DTOs only',()=>{
 it('preserves independent splitting limits without altering the existing Grant',()=>{const f=factoryFixture();expect(f.scope.selection.max_children).toBe(1);expect(f.grant.scope.max_children).toBe(0);expect(factoryScopeShape(f.scope,f.task)).toBe(true);expect(factoryDetailsShape(f.detail,f.task,f.scope.id)).toBe(true);expect(f.scope.bindings.workspaces[0].identity!.inode).toBeGreaterThan(Number.MAX_SAFE_INTEGER);});
 it.each(['foreign-task','grant-revision','implicit-start','new-workspace','daemon-token','sandbox','companion','input-hash','recipient'])('rejects %s authority/binding contradictions',kind=>{const f=factoryFixture(),s=structuredClone(f.scope);if(kind==='foreign-task')s.bindings.root_task.id='foreign';if(kind==='grant-revision')s.bindings.approved_grant.revision++;if(kind==='implicit-start')s.bindings.start_contract.implicit_start_on_approval=true;if(kind==='new-workspace')s.bindings.start_contract.new_workspace_authority=true;if(kind==='daemon-token')s.bindings.companion.global_daemon_token_available=true;if(kind==='sandbox')s.bindings.guarantees.filesystem_sandbox=true;if(kind==='companion')s.bindings.companion.argument_template=['--secret'];if(kind==='input-hash')s.bindings.inputs[0].sha256='model-says-safe';if(kind==='recipient')s.bindings.connections[0].id='foreign';expect(factoryScopeShape(s,f.task)).toBe(false);});
 it('compares safe requirement fields without depending on JSON property insertion order',()=>{const f=factoryFixture();const r=f.scope.bindings.outputs[0].requirement;f.scope.bindings.outputs[0].requirement={expected_sha256:r.expected_sha256,max_bytes:r.max_bytes,relative_path:r.relative_path,artifact_id:r.artifact_id};expect(factoryScopeShape(f.scope,f.task)).toBe(true);expect(factorySame(f.selection,JSON.parse(JSON.stringify(f.selection)))).toBe(true);});
 it('never treats cancellation request or future state as confirmed subtree stop',()=>{const f=factoryFixture();f.detail.run.state='cancel_requested';f.detail.run.cancel_requested=true;expect(factoryDetailsShape(f.detail,f.task,f.scope.id)).toBe(true);expect(f.detail.tasks[0].confirmed_stop).toBe(false);expect(factoryCanCancel(f.detail.run)).toBe(false);f.detail.run.state='future-release';expect(factoryRunSupported(f.detail.run)).toBe(false);expect(factoryCanCancel(f.detail.run)).toBe(false);});
 it('rejects terminal run with an unjoined attempt and foreign task/handoff references',()=>{const f=factoryFixture();f.detail.run.state='completed';f.detail.run.active_task_id=null;f.detail.tasks[0].attempt_id='attempt-live';expect(factoryDetailsShape(f.detail,f.task,f.scope.id)).toBe(false);f.detail.tasks[0].confirmed_stop=true;expect(factoryDetailsShape(f.detail,f.task,f.scope.id)).toBe(true);f.detail.waiting_relations=[{parent_task_id:f.task.id,child_task_id:'foreign',parent_session_id:'session',parent_stop_confirmed:true,handoff_digest:null}];expect(factoryDetailsShape(f.detail,f.task,f.scope.id)).toBe(false);});
 it.each(['../escape','/absolute','a/../b','C:/outside','a\\b'])('rejects unsafe artifact path %s',path=>{expect(()=>parseFactoryArtifacts(`id | ${path} | 1024`)).toThrow();});
 it.each(['unsafe-number','cost','rework','new-recipient','duplicate-artifact','empty-input','too-many-workspaces'])('fails unsupported or outside-Grant form %s before sending',kind=>{const f=factoryFixture(),form=structuredClone(f.form);if(kind==='unsafe-number')form.root.attempts='9007199254740992';if(kind==='cost')form.project.cost_microunits='1';if(kind==='rework')form.root.reworks='1';if(kind==='new-recipient')form.recipients='outside';if(kind==='duplicate-artifact')form.outputs='input | other.txt | 1024';if(kind==='empty-input')form.inputs='';if(kind==='too-many-workspaces')form.workspaces.push('outside');expect(()=>parseFactorySelection(form,f.task,f.grant)).toThrow();});
});

// Independent review's old 37c controls failed; preserve these assertions.
describe('Factory exact scope and lineage regression controls',()=>{
 it('rejects connection hosts outside the existing Grant host allowlist',()=>{const f=factoryFixture();f.scope.bindings.connections[0].host_id='outside-approved-host';expect(factoryScopeShape(f.scope,f.task)).toBe(false);});
 it('rejects recipients outside the existing Grant, independently of connection membership',()=>{const f=factoryFixture();f.scope.selection.recipients=['outside'];expect(factoryScopeShape(f.scope,f.task)).toBe(false);});
 it('rejects a completed details projection missing the required root handle',()=>{const f=factoryFixture();f.detail.run.state='completed';f.detail.run.active_task_id=null;f.detail.tasks=[];expect(factoryDetailsShape(f.detail,f.task,f.scope.id)).toBe(false);});
 it('binds waiting parent to the actual child lineage, preserving valid handoff relations',()=>{const f=factoryFixture(),root=f.detail.tasks[0];f.detail.tasks.push({...root,task_id:'child',parent_task_id:f.task.id,delegation_id:'delegation'});f.detail.waiting_relations=[{parent_task_id:f.task.id,child_task_id:'child',parent_session_id:'old-session',parent_stop_confirmed:true,handoff_digest:null}];expect(factoryDetailsShape(f.detail,f.task,f.scope.id)).toBe(true);f.detail.waiting_relations[0]={...f.detail.waiting_relations[0],parent_task_id:'child',child_task_id:f.task.id};expect(factoryDetailsShape(f.detail,f.task,f.scope.id)).toBe(false);});
 it('rejects disconnected or cyclic task chains instead of treating them as a closed subtree',()=>{const f=factoryFixture(),root=f.detail.tasks[0];f.detail.tasks.push({...root,task_id:'child',parent_task_id:'missing',delegation_id:'delegation'});expect(factoryDetailsShape(f.detail,f.task,f.scope.id)).toBe(false);f.detail.tasks.push({...root,task_id:'missing',parent_task_id:'child',delegation_id:'delegation-2'});expect(factoryDetailsShape(f.detail,f.task,f.scope.id)).toBe(false);});
 it.each(['missing','foreign','archived','unsafe-revision'])('rejects %s server Project binding',kind=>{const f=factoryFixture();if(kind==='missing')delete (f.scope.bindings as Partial<typeof f.scope.bindings>).project;if(kind==='foreign')f.scope.bindings.project.id='outside';if(kind==='archived')f.scope.bindings.project.archived=true;if(kind==='unsafe-revision')f.scope.bindings.project.revision=Number.MAX_SAFE_INTEGER+1;expect(factoryScopeShape(f.scope,f.task)).toBe(false);});
});

// Synthetic read DTOs matching Core's derived-child accepted-stop projection.
// They do not establish a new permission, Task acceptance or real stop proof.
async function completedDerivedHistory(){
 const f=await childGoalFixture(),root=f.detail.tasks[0];
 Object.assign(root,{state:'result_available',revision:19,attempt_id:'owned-root-attempt',confirmed_stop:true,result_available:true});
 const child={...structuredClone(root),task_id:'accepted-child-'+f.task.id,parent_task_id:f.task.id,delegation_id:'exact-child-delegation',revision:4,attempt_id:null};
 f.detail.tasks=[root,child];Object.assign(f.detail.run,{state:'completed',revision:9,active_task_id:null,active_session_id:null});
 f.detail.waiting_relations=[{parent_task_id:f.task.id,child_task_id:child.task_id,parent_session_id:'old-owned-parent',parent_stop_confirmed:true,handoff_digest:'a'.repeat(64)}];
 const currentRoot={...f.task,state:'outcome_unknown',revision:5};
 return {...f,child,currentRoot};
}
describe('completed derived-child history is not a Factory provider attempt',()=>{
 it('requires the validated execution Scope and exact result projection without inventing an attempt',async()=>{
  const f=await completedDerivedHistory();expect(factoryScopeShape(f.scope,f.currentRoot)).toBe(true);
  expect(factoryDetailsShape(f.detail,f.currentRoot,f.scope.id)).toBe(false); // Original strict3-argument contract.
  expect(factoryDetailsShape(f.detail,f.currentRoot,f.scope.id,f.scope)).toBe(true);
  expect(f.child.attempt_id).toBeNull();expect(f.child.confirmed_stop).toBe(true);expect(factoryCanCancel(f.detail.run)).toBe(false);
 });
 it.each(['expired','revoked'])('keeps %s history structural while live permissions remain closed',async mode=>{
  const f=await completedDerivedHistory(),clock=vi.spyOn(Date,'now');
  try{
   if(mode==='expired')clock.mockReturnValue(Math.max(f.scope.expires_at_ms,f.grant.expires_at_ms)+1);
   else{f.scope.revoked=true;f.scope.revision++;}
   expect(factoryScopeShape(f.scope,f.currentRoot)).toBe(true);expect(factoryDetailsShape(f.detail,f.currentRoot,f.scope.id,f.scope)).toBe(true);
   expect(f.scope.revoked||f.scope.expires_at_ms<=Date.now()).toBe(true);expect(factoryCanCancel(f.detail.run)).toBe(false);
  }finally{clock.mockRestore();}
 });
 it('does not reinterpret planning-only authority as derived execution',async()=>{
  const f=await completedDerivedHistory();f.scope.selection.capabilities.child_goal_execution=false;delete f.scope.selection.child_goal_execution_policy;delete f.scope.bindings.child_goal_execution;
  expect(factoryScopeShape(f.scope,f.currentRoot)).toBe(true);expect(factoryDetailsShape(f.detail,f.currentRoot,f.scope.id,f.scope)).toBe(false);
 });
 it.each(['scope-id','scope-root','child-root','child-scope','orphan','missing-result','wrong-result-state','cancelled-child','root-no-attempt','unknown-run','future-run','cancel-requested-run','unjoined-root','missing-root','duplicate-child','foreign-handoff'])('rejects %s instead of treating missing/contradictory data as a stopped history',async mode=>{
  const f=await completedDerivedHistory();
  if(mode==='scope-id')f.scope.id='foreign-scope';if(mode==='scope-root')f.scope.root_task_id='foreign-root';
  if(mode==='child-root')f.child.root_task_id='foreign-root';if(mode==='child-scope')f.child.factory_scope_id='foreign-scope';
  if(mode==='orphan')f.child.parent_task_id='missing-parent';if(mode==='missing-result')f.child.result_available=false;
  if(mode==='wrong-result-state')f.child.state='waiting_goal';if(mode==='cancelled-child')f.child.cancel_requested=true;
  if(mode==='root-no-attempt')f.detail.tasks[0].attempt_id=null;if(mode==='unknown-run')f.detail.run.state='outcome_unknown';if(mode==='future-run')f.detail.run.state='future-state';
  if(mode==='cancel-requested-run')f.detail.run.cancel_requested=true;if(mode==='unjoined-root')f.detail.tasks[0].confirmed_stop=false;
  if(mode==='missing-root')f.detail.tasks=[f.child];if(mode==='duplicate-child')f.detail.tasks.push(structuredClone(f.child));
  if(mode==='foreign-handoff')f.detail.waiting_relations[0].parent_task_id=f.child.task_id;
  expect(factoryDetailsShape(f.detail,f.currentRoot,f.scope.id,f.scope)).toBe(false);
 });
});
