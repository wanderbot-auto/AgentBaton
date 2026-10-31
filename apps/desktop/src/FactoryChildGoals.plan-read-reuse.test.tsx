// Mounted renderer/API mocks only. No Core, Native, Provider or CI claim.
import { afterEach,beforeEach,it,expect,vi } from 'vitest';
import { act,render,screen,waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { FactoryChildGoals } from './FactoryChildGoals';
import { api,DaemonError } from './api';
import { sessionApi } from './session-api';
import { factoryApi,factoryChildGoalApi,factoryChildPlanApi } from './factory-api';
import { goalGraphApi } from './goal-graph-api';
import * as childPlan from './factory-child-plan';
import { childGoalMessages } from './factory-child-goal-i18n';
import { childGoalFixture } from './factory-child-goal-test-fixtures';
import type { FactoryChildGoalView } from './factory-types';
vi.mock('./api',async()=>{const actual=await vi.importActual<typeof import('./api')>('./api');return {...actual,api:{...actual.api,snapshot:vi.fn()}};});
vi.mock('./session-api',()=>({sessionApi:{grantsList:vi.fn()}}));
vi.mock('./factory-api',()=>({factoryApi:{get:vi.fn(),run:vi.fn()},factoryChildPlanApi:{context:vi.fn(),get:vi.fn()},factoryChildGoalApi:{start:vi.fn(),list:vi.fn(),get:vi.fn(),cancel:vi.fn(),advancePlan:vi.fn()}}));
vi.mock('./goal-graph-api',()=>({goalGraphApi:{approveNode:vi.fn(),rejectNode:vi.fn()}}));
const t=childGoalMessages.en;
type Fixture=Awaited<ReturnType<typeof childGoalFixture>>;
beforeEach(()=>vi.resetAllMocks());
afterEach(()=>vi.restoreAllMocks());
function records(f:Fixture,initial:FactoryChildGoalView){
 const state={view:initial};
 vi.mocked(api.snapshot).mockImplementation(async()=>({tasks:[structuredClone(f.task)],sequence:1000,api_version:1}));
 vi.mocked(sessionApi.grantsList).mockImplementation(async()=>[structuredClone(f.grant)]);
 vi.mocked(factoryApi.get).mockImplementation(async()=>structuredClone(f.scope));
 vi.mocked(factoryApi.run).mockImplementation(async()=>structuredClone(f.detail));
 vi.mocked(factoryChildPlanApi.context).mockImplementation(async()=>structuredClone(f.context));
 vi.mocked(factoryChildPlanApi.get).mockImplementation(async(_scope,_task,revision)=>{
  const plan=f.plans.get(revision??f.context.current_plan_revision);
  if(!plan)throw new DaemonError('NOT_FOUND','Missing immutable test plan','not_started');
  return structuredClone(plan);
 });
 vi.mocked(factoryChildGoalApi.list).mockImplementation(async()=>[structuredClone(state.view.summary)]);
 vi.mocked(factoryChildGoalApi.get).mockImplementation(async()=>structuredClone(state.view));
 return state;
}
const props=(f:Fixture)=>({scope:f.scope,target:f.target,run:f.detail.run,selected:f.plans.get(f.context.current_plan_revision)!,language:'en' as const,connected:true,parentReady:true});
const panel=()=>screen.getByTestId('factory-child-goal-panel');
async function open(user:ReturnType<typeof userEvent.setup>){
 await user.click(screen.getByText(/^Factory child Goal execution ·/,{selector:'summary'}));
 await screen.findByTestId('factory-child-goal-panel');
 await waitFor(()=>expect(screen.getByRole('button',{name:t.refresh})).toBeEnabled());
}
function noWrites(){
 expect(factoryChildGoalApi.start).not.toHaveBeenCalled();expect(factoryChildGoalApi.cancel).not.toHaveBeenCalled();
 expect(factoryChildGoalApi.advancePlan).not.toHaveBeenCalled();expect(goalGraphApi.approveNode).not.toHaveBeenCalled();expect(goalGraphApi.rejectNode).not.toHaveBeenCalled();
}
function planCalls(){return vi.mocked(factoryChildPlanApi.get).mock.calls.map(call=>[...call]);}
async function thirdHead(f:Fixture){const value=await f.view(3);f.plans.set(3,value);f.context.current_plan_revision=3;return value;}

it('reads one head DTO for equal initial/adopted refs and still verifies raw bytes for all three roles',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();records(f,f.ownerReady());
 const verify=vi.spyOn(childPlan,'verifyChildRaw');
 render(<FactoryChildGoals {...props(f)}/>);await open(user);
 await waitFor(()=>expect(screen.getByRole('button',{name:t.owner})).toBeEnabled());
 expect(planCalls()).toEqual([[f.scope.id,f.task.id]]);
 expect(factoryChildGoalApi.get).toHaveBeenCalledOnce();expect(verify).toHaveBeenCalledTimes(3);
 expect(verify.mock.calls[0][0]).toBe(verify.mock.calls[1][0]);expect(verify.mock.calls[1][0]).toBe(verify.mock.calls[2][0]);
 expect(panel()).toHaveAttribute('data-cg-fresh','true');noWrites();
});

it('loads and verifies a new head in every manual refresh instead of caching a prior refresh DTO',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();records(f,f.ownerReady());const verify=vi.spyOn(childPlan,'verifyChildRaw');
 render(<FactoryChildGoals {...props(f)}/>);await open(user);await waitFor(()=>expect(screen.getByRole('button',{name:t.owner})).toBeEnabled());
 const firstEpoch=Number(panel().getAttribute('data-cg-epoch')),firstHead=verify.mock.calls[0][0];
 await user.click(screen.getByRole('button',{name:t.refresh}));await waitFor(()=>expect(verify).toHaveBeenCalledTimes(6));
 await waitFor(()=>expect(screen.getByRole('button',{name:t.owner})).toBeEnabled());
 expect(planCalls()).toEqual([[f.scope.id,f.task.id],[f.scope.id,f.task.id]]);
 expect(verify.mock.calls[3][0]).not.toBe(firstHead);expect(factoryChildGoalApi.get).toHaveBeenCalledTimes(2);
 expect(Number(panel().getAttribute('data-cg-epoch'))).toBeGreaterThan(firstEpoch);noWrites();
});

it('fetches a distinct initial reference while retaining the existing equal initial/adopted reuse',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();await f.nextPlan();records(f,f.ownerReady());
 render(<FactoryChildGoals {...props(f)}/>);await open(user);await waitFor(()=>expect(screen.getByRole('button',{name:t.owner})).toBeEnabled());
 expect(planCalls()).toEqual([[f.scope.id,f.task.id],[f.scope.id,f.task.id,1]]);
 expect(panel()).toHaveAttribute('data-cg-fresh','true');noWrites();
});

it('reuses head for the adopted ref while fetching a different immutable initial ref',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture(),adopted=await f.nextPlan();records(f,f.makeView(adopted,3,'blocked'));
 const verify=vi.spyOn(childPlan,'verifyChildRaw');
 render(<FactoryChildGoals {...props(f)}/>);await open(user);await screen.findByTestId('goal-graph-history');
 expect(planCalls()).toEqual([[f.scope.id,f.task.id],[f.scope.id,f.task.id,1]]);
 expect(verify).toHaveBeenCalledTimes(3);expect(verify.mock.calls[0][0]).toBe(verify.mock.calls[2][0]);
 expect(verify.mock.calls[0][0]).not.toBe(verify.mock.calls[1][0]);expect(panel()).toHaveAttribute('data-cg-fresh','true');noWrites();
});

it('fetches both historical refs when neither complete reference equals the current head',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture(),adopted=await f.nextPlan();await thirdHead(f);records(f,f.makeView(adopted,3,'blocked'));
 render(<FactoryChildGoals {...props(f)}/>);await open(user);await screen.findByTestId('goal-graph-history');
 expect(planCalls()).toEqual([[f.scope.id,f.task.id],[f.scope.id,f.task.id,1],[f.scope.id,f.task.id,2]]);
 expect(panel()).toHaveAttribute('data-cg-fresh','true');noWrites();
});

it.each(['id','plan_digest','raw_sha256','receipt_sequence']as const)('does not reuse a same-revision head when full reference field %s differs',async field=>{
 const user=userEvent.setup(),f=await childGoalFixture();records(f,f.ownerReady());
 const head=field==='raw_sha256'?await f.view(1,f.plan.plan_json+'\n'):structuredClone(f.plan);
 if(field==='id')head.reference.id+='-different';
 if(field==='plan_digest')head.reference.plan_digest='f'.repeat(64);
 if(field==='receipt_sequence')head.reference.receipt_sequence++;
 vi.mocked(factoryChildPlanApi.get).mockImplementation(async(_scope,_task,revision)=>structuredClone(revision===undefined?head:f.plan));
 render(<FactoryChildGoals {...props(f)}/>);await open(user);await waitFor(()=>expect(screen.getByRole('button',{name:t.owner})).toBeEnabled());
 expect(head.reference.revision).toBe(f.plan.reference.revision);
 expect(planCalls()).toEqual([[f.scope.id,f.task.id],[f.scope.id,f.task.id,1]]);noWrites();
});

it.each(['shape','raw']as const)('rejects %s corruption before a head DTO can be reused',async mode=>{
 const user=userEvent.setup(),f=await childGoalFixture();records(f,f.ownerReady());const head=structuredClone(f.plan);
 if(mode==='shape')Object.assign(head,{planning_only:false});else head.plan_json+='\n';
 vi.mocked(factoryChildPlanApi.get).mockResolvedValue(head);
 render(<FactoryChildGoals {...props(f)}/>);await open(user);await screen.findByRole('alert');
 expect(planCalls()).toEqual([[f.scope.id,f.task.id]]);expect(factoryChildGoalApi.get).not.toHaveBeenCalled();
 expect(panel()).toHaveAttribute('data-cg-fresh','false');expect(screen.queryByRole('button',{name:t.owner})).toBeNull();noWrites();
});

it.each(['shape','raw']as const)('revalidates reused head %s after the intervening exact Goal await',async mode=>{
 const user=userEvent.setup(),f=await childGoalFixture(),view=f.ownerReady(),head=structuredClone(f.plan);records(f,view);
 vi.mocked(factoryChildPlanApi.get).mockResolvedValue(head);
 vi.mocked(factoryChildGoalApi.get).mockImplementation(async()=>{
  if(mode==='shape')Object.assign(head,{planning_only:false});else head.plan_json+='\n';
  return structuredClone(view);
 });
 render(<FactoryChildGoals {...props(f)}/>);await open(user);await screen.findByRole('alert');
 expect(planCalls()).toEqual([[f.scope.id,f.task.id]]);expect(factoryChildGoalApi.get).toHaveBeenCalledOnce();
 expect(panel()).toHaveAttribute('data-cg-fresh','false');expect(screen.queryByRole('button',{name:t.owner})).toBeNull();noWrites();
});

it.each(['shape','raw']as const)('keeps all original %s checks on a freshly fetched different-reference initial DTO',async mode=>{
 const user=userEvent.setup(),f=await childGoalFixture();await f.nextPlan();records(f,f.ownerReady());const initial=structuredClone(f.plan);
 if(mode==='shape')Object.assign(initial,{planning_only:false});else initial.plan_json+='\n';
 vi.mocked(factoryChildPlanApi.get).mockImplementation(async(_scope,_task,revision)=>structuredClone(revision===undefined?f.plans.get(2)!:initial));
 render(<FactoryChildGoals {...props(f)}/>);await open(user);await screen.findByRole('alert');
 expect(planCalls()).toEqual([[f.scope.id,f.task.id],[f.scope.id,f.task.id,1]]);
 expect(panel()).toHaveAttribute('data-cg-fresh','false');expect(screen.queryByRole('button',{name:t.owner})).toBeNull();noWrites();
});

it.each(['scope','task','run']as const)('rejects a head outside the current %s binding before any reuse',async mode=>{
 const user=userEvent.setup(),f=await childGoalFixture();records(f,f.ownerReady());const head=structuredClone(f.plan);
 if(mode==='scope')head.reference.scope_id='foreign-scope';if(mode==='task')head.reference.task_id='foreign-task';if(mode==='run')head.reference.run_id='foreign-run';
 vi.mocked(factoryChildPlanApi.get).mockResolvedValue(head);
 render(<FactoryChildGoals {...props(f)}/>);await open(user);await screen.findByRole('alert');
 expect(factoryChildGoalApi.get).not.toHaveBeenCalled();expect(panel()).toHaveAttribute('data-cg-fresh','false');
 expect(screen.queryByRole('button',{name:t.owner})).toBeNull();noWrites();
});

it('discards an exact Goal result after its head was verified but disconnect superseded the refresh epoch',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture(),view=f.ownerReady();records(f,view);
 let release!:(value:FactoryChildGoalView)=>void;const held=new Promise<FactoryChildGoalView>(resolve=>{release=resolve;});
 vi.mocked(factoryChildGoalApi.get).mockReturnValueOnce(held);const current=props(f),mounted=render(<FactoryChildGoals {...current}/>);
 await user.click(screen.getByText(/^Factory child Goal execution ·/,{selector:'summary'}));
 await waitFor(()=>expect(factoryChildGoalApi.get).toHaveBeenCalledOnce());const epoch=Number(panel().getAttribute('data-cg-epoch'));
 mounted.rerender(<FactoryChildGoals {...current} connected={false}/>);
 await waitFor(()=>expect(panel()).toHaveAttribute('data-cg-connected','false'));await act(async()=>release(structuredClone(view)));
 expect(Number(panel().getAttribute('data-cg-epoch'))).toBeGreaterThan(epoch);
 expect(planCalls()).toEqual([[f.scope.id,f.task.id]]);expect(panel()).toHaveAttribute('data-cg-fresh','false');
 expect(panel()).toHaveAttribute('data-cg-has-view','false');expect(screen.queryByRole('button',{name:t.owner})).toBeNull();noWrites();
});
