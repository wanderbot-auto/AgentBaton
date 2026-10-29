// Mocked component/transport controls only. No Core, Native dialog or Provider.
import { render,screen,waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach,it,expect,vi } from 'vitest';
import { FactoryRuntime } from './FactoryRuntime';
import { childGoalFixture } from './factory-child-goal-test-fixtures';
import { factoryApi } from './factory-api';
import { api } from './api';
import { sessionApi } from './session-api';
import { factoryMessages } from './factory-i18n';
import { factoryScopeShape,factoryDetailsShape } from './factory-guards';
vi.mock('./FactoryChildPlans',()=>({FactoryChildPlans:(p:{target:{task_id:string;attempt_id:string|null};parentReady:boolean})=><output data-testid="persisted-child-slot" data-task={p.target.task_id} data-attempt={p.target.attempt_id??'none'} data-ready={String(p.parentReady)}/> }));
vi.mock('./factory-api',()=>({factoryApi:{approve:vi.fn(),get:vi.fn(),list:vi.fn(),revoke:vi.fn(),start:vi.fn(),run:vi.fn(),cancel:vi.fn()}}));
vi.mock('./api',async()=>{const a=await vi.importActual<typeof import('./api')>('./api');return {...a,api:{workspacesList:vi.fn()}};});
vi.mock('./session-api',()=>({sessionApi:{grantsList:vi.fn(),connectionsList:vi.fn()}}));
const t=factoryMessages.en;
beforeEach(()=>vi.resetAllMocks());
async function completed(){
 const f=await childGoalFixture(),root=f.detail.tasks[0];
 Object.assign(root,{state:'result_available',revision:19,attempt_id:'joined-parent-attempt',confirmed_stop:true,result_available:true});
 const child={...structuredClone(root),task_id:'accepted-child-'+f.task.id,parent_task_id:f.task.id,delegation_id:'child-delegation',revision:4,attempt_id:null};
 f.detail.tasks=[root,child];Object.assign(f.detail.run,{state:'completed',revision:9,active_task_id:null,active_session_id:null});
 f.detail.waiting_relations=[{parent_task_id:f.task.id,child_task_id:child.task_id,parent_session_id:'old-owned-parent',parent_stop_confirmed:true,handoff_digest:'a'.repeat(64)}];
 const currentRoot={...f.task,state:'outcome_unknown',revision:5};
 vi.mocked(sessionApi.grantsList).mockImplementation(async()=>[structuredClone(f.grant)]);vi.mocked(sessionApi.connectionsList).mockImplementation(async()=>[structuredClone(f.connection)]);vi.mocked(api.workspacesList).mockImplementation(async()=>[structuredClone(f.workspace)]);
 vi.mocked(factoryApi.list).mockImplementation(async()=>[structuredClone(f.scope)]);vi.mocked(factoryApi.run).mockImplementation(async()=>structuredClone(f.detail));
 return {...f,child,currentRoot};
}
it.each(['live','expired','revoked'])('retains completed no-Factory-attempt child history under %s scope without enabling mutations',async mode=>{
 const f=await completed(),user=userEvent.setup(),clock=vi.spyOn(Date,'now');
 try{
  if(mode==='expired')clock.mockReturnValue(Math.max(f.scope.expires_at_ms,f.grant.expires_at_ms)+1);
  if(mode==='revoked'){f.scope.revoked=true;f.scope.revision++;}
  expect(factoryScopeShape(f.scope,f.currentRoot)).toBe(true);expect(factoryDetailsShape(f.detail,f.currentRoot,f.scope.id,f.scope)).toBe(true);
  render(<FactoryRuntime task={f.currentRoot} language="en" connected/>);await user.click(screen.getByRole('heading',{name:t.title}));
  await waitFor(()=>expect(screen.getAllByTestId('persisted-child-slot')).toHaveLength(2));
  const child=screen.getAllByTestId('persisted-child-slot').find(row=>row.dataset.task===f.child.task_id)!;expect(child).toHaveAttribute('data-attempt','none');
  if(mode!=='live')expect(child).toHaveAttribute('data-ready','false');
  expect(screen.queryByRole('alert')).toBeNull();expect(screen.getByRole('region',{name:t.run})).toHaveTextContent('completed');
  expect(screen.getByRole('button',{name:t.start})).toBeDisabled();expect(screen.getByRole('button',{name:t.approve})).toBeDisabled();expect(screen.getByRole('button',{name:t.cancel})).toBeDisabled();
  if(mode==='expired')expect(screen.getByLabelText(t.selectGrant).querySelectorAll('option')).toHaveLength(1); // Only the empty choice, not missing history.
  await user.click(screen.getByRole('button',{name:t.refresh}));await waitFor(()=>expect(screen.getAllByTestId('persisted-child-slot')).toHaveLength(2));
  expect(factoryApi.start).not.toHaveBeenCalled();expect(factoryApi.approve).not.toHaveBeenCalled();expect(factoryApi.revoke).not.toHaveBeenCalled();expect(factoryApi.cancel).not.toHaveBeenCalled();
 }finally{clock.mockRestore();}
});
it.each(['unjoined-parent','foreign-child'])('keeps %s evidence blocked instead of rendering a closed history',async mode=>{
 const f=await completed(),user=userEvent.setup();if(mode==='unjoined-parent')f.detail.tasks[0].confirmed_stop=false;else f.child.parent_task_id='foreign-parent';
 render(<FactoryRuntime task={f.currentRoot} language="en" connected/>);await user.click(screen.getByRole('heading',{name:t.title}));expect(await screen.findByRole('alert')).toHaveTextContent(t.invalid);
 expect(screen.queryByTestId('persisted-child-slot')).toBeNull();expect(factoryApi.start).not.toHaveBeenCalled();expect(factoryApi.approve).not.toHaveBeenCalled();
});
