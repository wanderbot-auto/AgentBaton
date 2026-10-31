// Synthetic read DTO/component transport only: no Core, Native approval or Provider.
import { act,render,screen,waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach,describe,it,expect,vi } from 'vitest';
import { FactoryRuntime } from './FactoryRuntime';
import { childGoalFixture } from './factory-child-goal-test-fixtures';
import { factoryApi } from './factory-api';
import { api } from './api';
import { sessionApi } from './session-api';
import { factoryMessages } from './factory-i18n';
import { factoryScopeShape,factoryDetailsShape,factoryCanCancel,factoryRunShape,factoryRunSupported } from './factory-guards';
import type { FactoryRunDetails } from './factory-types';
vi.mock('./FactoryChildPlans',()=>({FactoryChildPlans:(p:{target:{task_id:string;attempt_id:string|null}})=><output data-testid="cancelled-child-slot" data-task={p.target.task_id} data-attempt={p.target.attempt_id??'none'}/> }));
vi.mock('./factory-api',()=>({factoryApi:{approve:vi.fn(),get:vi.fn(),list:vi.fn(),revoke:vi.fn(),start:vi.fn(),run:vi.fn(),cancel:vi.fn()}}));
vi.mock('./api',async()=>{const a=await vi.importActual<typeof import('./api')>('./api');return {...a,api:{workspacesList:vi.fn()}};});
vi.mock('./session-api',()=>({sessionApi:{grantsList:vi.fn(),connectionsList:vi.fn()}}));
const t=factoryMessages.en;
beforeEach(()=>vi.resetAllMocks());
async function cancelledHistory(){
 const f=await childGoalFixture(),root=f.detail.tasks[0];
 // Exact finite projection seen in first-summary-cancel1 wire197: the root
 // keeps its real joined Factory attempt; only its derived child has none.
 Object.assign(root,{state:'waiting_child',revision:12,attempt_id:'joined-parent-attempt',confirmed_stop:true,result_available:false,cancel_requested:true});
 const child={...structuredClone(root),task_id:'cancelled-child-'+f.task.id,parent_task_id:f.task.id,delegation_id:'child-delegation',revision:5,attempt_id:null,state:'cancelled'};
 f.detail.tasks=[root,child];Object.assign(f.detail.run,{state:'cancelled',revision:7,cancel_requested:true,active_task_id:null,active_session_id:null});
 f.detail.waiting_relations=[{parent_task_id:f.task.id,child_task_id:child.task_id,parent_session_id:'old-owned-parent',parent_stop_confirmed:true,handoff_digest:'a'.repeat(64)}];
 const currentRoot={...f.task,state:'outcome_unknown',revision:5};
 vi.mocked(sessionApi.grantsList).mockImplementation(async()=>[structuredClone(f.grant)]);vi.mocked(sessionApi.connectionsList).mockImplementation(async()=>[structuredClone(f.connection)]);vi.mocked(api.workspacesList).mockImplementation(async()=>[structuredClone(f.workspace)]);
 vi.mocked(factoryApi.list).mockImplementation(async()=>[structuredClone(f.scope)]);vi.mocked(factoryApi.run).mockImplementation(async()=>structuredClone(f.detail));
 return {...f,child,currentRoot};
}
function noWrites(){expect(factoryApi.start).not.toHaveBeenCalled();expect(factoryApi.approve).not.toHaveBeenCalled();expect(factoryApi.revoke).not.toHaveBeenCalled();expect(factoryApi.cancel).not.toHaveBeenCalled();}
describe('cancelled derived-child history is structural, never execution authority',()=>{
 it('requires the full execution Scope, preserves the strict legacy caller and invents no Factory attempt',async()=>{
  const f=await cancelledHistory();expect(factoryScopeShape(f.scope,f.currentRoot)).toBe(true);
  expect(factoryDetailsShape(f.detail,f.currentRoot,f.scope.id)).toBe(false);
  expect(factoryDetailsShape(f.detail,f.currentRoot,f.scope.id,f.scope)).toBe(true);
  expect(f.child.attempt_id).toBeNull();expect(f.child.result_available).toBe(false);expect(factoryCanCancel(f.detail.run)).toBe(false);
 });
 it('rejects a future continuation contract even when the cancelled DTO has valid scalar shape',async()=>{
  const f=await cancelledHistory();f.detail.run.context_continuation='foreign_future_continuation';
  expect(factoryRunShape(f.detail.run,f.currentRoot,f.scope.id)).toBe(true);expect(factoryRunSupported(f.detail.run)).toBe(false);
  expect(factoryDetailsShape(f.detail,f.currentRoot,f.scope.id,f.scope)).toBe(false);noWrites();
 });
 it.each(['live','expired','revoked'])('retains %s cancelled history without start/approval/cancel writes',async mode=>{
  const f=await cancelledHistory(),user=userEvent.setup(),clock=vi.spyOn(Date,'now');
  try{
   if(mode==='expired')clock.mockReturnValue(Math.max(f.scope.expires_at_ms,f.grant.expires_at_ms)+1);
   if(mode==='revoked'){f.scope.revoked=true;f.scope.revision++;}
   render(<FactoryRuntime task={f.currentRoot} language="en" connected/>);await user.click(screen.getByRole('heading',{name:t.title}));
   await waitFor(()=>expect(screen.getAllByTestId('cancelled-child-slot')).toHaveLength(2));
   expect(screen.getByRole('region',{name:t.run})).toHaveTextContent('cancelled');expect(screen.queryByRole('alert')).toBeNull();
   const child=screen.getAllByTestId('cancelled-child-slot').find(row=>row.dataset.task===f.child.task_id)!;expect(child).toHaveAttribute('data-attempt','none');
   expect(screen.getByRole('button',{name:t.start})).toBeDisabled();expect(screen.getByRole('button',{name:t.approve})).toBeDisabled();expect(screen.getByRole('button',{name:t.cancel})).toBeDisabled();
   await user.click(screen.getByRole('button',{name:t.refresh}));await waitFor(()=>expect(screen.getByRole('button',{name:t.refresh})).toBeEnabled());
   expect(screen.getAllByTestId('cancelled-child-slot')).toHaveLength(2);noWrites();
  }finally{clock.mockRestore();}
 });
 it('keeps the exact child history attached across deferred parent cancelled refresh',async()=>{
  const f=await cancelledHistory(),user=userEvent.setup(),before=structuredClone(f.detail);
  Object.assign(before.run,{state:'waiting_child',revision:5,cancel_requested:false});
  Object.assign(before.tasks[0],{revision:10,cancel_requested:false});Object.assign(before.tasks[1],{state:'waiting_goal',revision:3,cancel_requested:false,confirmed_stop:false});
  vi.mocked(factoryApi.run).mockResolvedValueOnce(before);
  render(<FactoryRuntime task={f.currentRoot} language="en" connected/>);await user.click(screen.getByRole('heading',{name:t.title}));
  await waitFor(()=>expect(screen.getAllByTestId('cancelled-child-slot')).toHaveLength(2));
  let release!:(value:FactoryRunDetails)=>void;const pending=new Promise<FactoryRunDetails>(resolve=>{release=resolve;});
  vi.mocked(factoryApi.run).mockImplementationOnce(()=>pending);await user.click(screen.getByRole('button',{name:t.refresh}));
  await waitFor(()=>expect(factoryApi.run).toHaveBeenCalledTimes(2));expect(screen.getAllByTestId('cancelled-child-slot')).toHaveLength(2);expect(screen.getByRole('button',{name:t.refresh})).toBeDisabled();
  await act(async()=>{release(structuredClone(f.detail));});
  await waitFor(()=>expect(screen.getByRole('button',{name:t.refresh})).toBeEnabled());expect(screen.getByRole('region',{name:t.run})).toHaveTextContent('cancelled');expect(screen.getAllByTestId('cancelled-child-slot')).toHaveLength(2);expect(screen.queryByRole('alert')).toBeNull();noWrites();
 });
 it.each(['planning-only','foreign-scope','foreign-root','foreign-child-root','foreign-child-scope','orphan','root-no-attempt','unknown-run','future-run','request-only-run','running-run','missing-cancel-flag','active-session','active-task','unjoined-root','unjoined-child','unknown-child','result-child','missing-child-cancel','unconfirmed-handoff','foreign-handoff','missing-root','duplicate-child'])('rejects %s without converting unknown/requested evidence into terminal history',async mode=>{
  const f=await cancelledHistory();
  if(mode==='planning-only'){f.scope.selection.capabilities.child_goal_execution=false;delete f.scope.selection.child_goal_execution_policy;delete f.scope.bindings.child_goal_execution;}
  if(mode==='foreign-scope')f.scope.id='foreign';if(mode==='foreign-root')f.scope.root_task_id='foreign';
  if(mode==='foreign-child-root')f.child.root_task_id='foreign';if(mode==='foreign-child-scope')f.child.factory_scope_id='foreign';if(mode==='orphan')f.child.parent_task_id='missing';
  if(mode==='root-no-attempt')f.detail.tasks[0].attempt_id=null;if(mode==='unknown-run')f.detail.run.state='outcome_unknown';if(mode==='future-run')f.detail.run.state='future';
  if(mode==='request-only-run')f.detail.run.state='cancel_requested';if(mode==='running-run')f.detail.run.state='running';if(mode==='missing-cancel-flag')f.detail.run.cancel_requested=false;
  if(mode==='active-session'){f.detail.run.active_session_id='live';f.detail.run.active_task_id=f.task.id;}if(mode==='active-task')f.detail.run.active_task_id=f.task.id;
  if(mode==='unjoined-root')f.detail.tasks[0].confirmed_stop=false;
  // A claimed cancelled handle without stop evidence must not satisfy this branch.
  if(mode==='unjoined-child')f.child.confirmed_stop=false;
  if(mode==='unknown-child')f.child.state='outcome_unknown';if(mode==='result-child')f.child.result_available=true;if(mode==='missing-child-cancel')f.child.cancel_requested=false;
  if(mode==='unconfirmed-handoff')f.detail.waiting_relations[0].parent_stop_confirmed=false;if(mode==='foreign-handoff')f.detail.waiting_relations[0].parent_task_id=f.child.task_id;
  if(mode==='missing-root')f.detail.tasks=[f.child];if(mode==='duplicate-child')f.detail.tasks.push(structuredClone(f.child));
  expect(factoryDetailsShape(f.detail,f.currentRoot,f.scope.id,f.scope)).toBe(false);noWrites();
 });
});
