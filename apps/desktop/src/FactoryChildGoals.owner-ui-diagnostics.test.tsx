// Renderer-only v4 observation controls; no Core, Native, Provider or CI claim.
import { beforeEach,it,expect,vi } from 'vitest';
import { act,render,screen,waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { FactoryChildGoals } from './FactoryChildGoals';
import { api,DaemonError } from './api';
import { sessionApi } from './session-api';
import { factoryApi,factoryChildGoalApi,factoryChildPlanApi } from './factory-api';
import { goalGraphApi } from './goal-graph-api';
import { childGoalMessages } from './factory-child-goal-i18n';
import { childGoalFixture } from './factory-child-goal-test-fixtures';
import type { FactoryChildGoalView,FactoryChildPlanView } from './factory-types';
vi.mock('./api',async()=>{const actual=await vi.importActual<typeof import('./api')>('./api');return {...actual,api:{...actual.api,snapshot:vi.fn()}};});
vi.mock('./session-api',()=>({sessionApi:{grantsList:vi.fn()}}));
vi.mock('./factory-api',()=>({factoryApi:{get:vi.fn(),run:vi.fn()},factoryChildPlanApi:{context:vi.fn(),get:vi.fn()},factoryChildGoalApi:{start:vi.fn(),list:vi.fn(),get:vi.fn(),cancel:vi.fn(),advancePlan:vi.fn()}}));
vi.mock('./goal-graph-api',()=>({goalGraphApi:{approveNode:vi.fn(),rejectNode:vi.fn()}}));
const t=childGoalMessages.en;
type Fixture=Awaited<ReturnType<typeof childGoalFixture>>;
beforeEach(()=>vi.resetAllMocks());
function records(f:Fixture,initial:FactoryChildGoalView|null=null){
 const state={view:initial,task:structuredClone(f.task),grant:structuredClone(f.grant)};
 vi.mocked(api.snapshot).mockImplementation(async()=>({tasks:[structuredClone(state.task)],sequence:1000,api_version:1,database_schema_version:17}));
 vi.mocked(sessionApi.grantsList).mockImplementation(async()=>[structuredClone(state.grant)]);
 vi.mocked(factoryApi.get).mockImplementation(async()=>structuredClone(f.scope));vi.mocked(factoryApi.run).mockImplementation(async()=>structuredClone(f.detail));
 vi.mocked(factoryChildPlanApi.context).mockImplementation(async()=>structuredClone(f.context));vi.mocked(factoryChildPlanApi.get).mockImplementation(async(_s,_t,revision)=>{const p=f.plans.get(revision??f.context.current_plan_revision);if(!p)throw new DaemonError('NOT_FOUND','missing original saved plan','not_started');return structuredClone(p);});
 vi.mocked(factoryChildGoalApi.list).mockImplementation(async()=>state.view?[structuredClone(state.view.summary)]:[]);
 vi.mocked(factoryChildGoalApi.get).mockImplementation(async()=>{if(!state.view)throw new DaemonError('NOT_FOUND','not started','not_started');return structuredClone(state.view);});
 vi.mocked(factoryChildGoalApi.start).mockImplementation(async()=>{state.view=f.makeView();return structuredClone(state.view.summary);});
 vi.mocked(factoryChildGoalApi.cancel).mockImplementation(async()=>{const v=structuredClone(state.view!);v.details.run={...v.details.run,revision:v.details.run.revision+1,state:'cancel_requested',cancel_requested:true};v.details.ledger_revision++;state.view=f.sync(v);return structuredClone(state.view.summary);});
 return state;
}
const props=(f:Fixture,selected:FactoryChildPlanView|null=f.plan)=>({scope:f.scope,target:f.target,run:f.detail.run,selected,language:'en' as const,connected:true,parentReady:true});
async function open(user:ReturnType<typeof userEvent.setup>){await user.click(screen.getByText(/^Factory child Goal execution ·/,{selector:'summary'}));await screen.findByTestId('factory-child-goal-panel');await waitFor(()=>expect(screen.getByRole('button',{name:t.refresh})).toBeEnabled());}

function deferInitial(f:Fixture){
 let release!:(value:FactoryChildPlanView)=>void;
 const held=new Promise<FactoryChildPlanView>(resolve=>{release=resolve;});
 vi.mocked(factoryChildPlanApi.get).mockImplementation(async(_scope,_task,revision)=>revision===1?held:structuredClone(f.plan));
 return ()=>release(structuredClone(f.plan));
}
const panel=()=>screen.getByTestId('factory-child-goal-panel');
const noWrites=()=>{expect(goalGraphApi.approveNode).not.toHaveBeenCalled();expect(goalGraphApi.rejectNode).not.toHaveBeenCalled();expect(factoryChildGoalApi.start).not.toHaveBeenCalled();expect(factoryChildGoalApi.cancel).not.toHaveBeenCalled();expect(factoryChildGoalApi.advancePlan).not.toHaveBeenCalled();};

it('observes a completed Goal get while initial-plan tail still prevents full freshness',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture(true);records(f,f.ownerReady());const release=deferInitial(f);
 render(<FactoryChildGoals {...props(f)}/>);
 await user.click(screen.getByText(/^Factory child Goal execution ·/,{selector:'summary'}));
 await waitFor(()=>expect(factoryChildGoalApi.get).toHaveBeenCalledOnce());
 await waitFor(()=>expect(factoryChildPlanApi.get).toHaveBeenCalledWith(f.scope.id,f.task.id,1));
 expect(panel()).toHaveAttribute('data-cg-diagnostic','owner-ui-v4');
 expect(panel()).toHaveAttribute('data-cg-fresh','false');expect(panel()).toHaveAttribute('data-cg-loading','true');
 expect(panel()).toHaveAttribute('data-cg-has-view','false');expect(panel()).toHaveAttribute('data-cg-pending-kind','none');
 expect(screen.queryByRole('button',{name:t.owner})).toBeNull();noWrites();
 await act(async()=>release());await waitFor(()=>expect(screen.getByRole('button',{name:t.owner})).toBeEnabled());
 expect(panel()).toHaveAttribute('data-cg-fresh','true');expect(panel()).toHaveAttribute('data-cg-loading','false');
 expect(screen.getByRole('button',{name:t.owner})).toHaveAttribute('data-cg-owner-control','review');noWrites();
});

it('retains the prior view but disables Owner during a deferred manual-refresh tail',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture(true);records(f,f.ownerReady());render(<FactoryChildGoals {...props(f)}/>);await open(user);
 await waitFor(()=>expect(screen.getByRole('button',{name:t.owner})).toBeEnabled());
 const epoch=Number(panel().getAttribute('data-cg-epoch')),release=deferInitial(f);
 await user.click(screen.getByRole('button',{name:t.refresh}));
 await waitFor(()=>expect(panel()).toHaveAttribute('data-cg-loading','true'));
 expect(Number(panel().getAttribute('data-cg-epoch'))).toBeGreaterThan(epoch);
 expect(panel()).toHaveAttribute('data-cg-fresh','false');expect(panel()).toHaveAttribute('data-cg-has-view','true');
 expect(screen.getByRole('button',{name:t.owner})).toBeDisabled();noWrites();
 await act(async()=>release());await waitFor(()=>expect(screen.getByRole('button',{name:t.owner})).toBeEnabled());noWrites();
});

it('cannot publish a deferred old epoch after disconnect supersedes it',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture(true);records(f,f.ownerReady());const release=deferInitial(f),current=props(f);
 const view=render(<FactoryChildGoals {...current}/>);
 await user.click(screen.getByText(/^Factory child Goal execution ·/,{selector:'summary'}));
 await waitFor(()=>expect(factoryChildPlanApi.get).toHaveBeenCalledWith(f.scope.id,f.task.id,1));
 const epoch=Number(panel().getAttribute('data-cg-epoch'));
 view.rerender(<FactoryChildGoals {...current} connected={false}/>);
 await waitFor(()=>expect(panel()).toHaveAttribute('data-cg-connected','false'));
 await act(async()=>release());
 expect(Number(panel().getAttribute('data-cg-epoch'))).toBeGreaterThan(epoch);
 expect(panel()).toHaveAttribute('data-cg-fresh','false');expect(panel()).toHaveAttribute('data-cg-has-view','false');
 expect(screen.queryByRole('button',{name:t.owner})).toBeNull();noWrites();
});

it('distinguishes a fresh base gate from the existing unready-parent Owner gate',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture(true);records(f,f.ownerReady());render(<FactoryChildGoals {...props(f)} parentReady={false}/>);await open(user);
 await screen.findByRole('button',{name:t.owner});
 expect(panel()).toHaveAttribute('data-cg-fresh','true');expect(panel()).toHaveAttribute('data-cg-disabled','false');
 expect(panel()).toHaveAttribute('data-cg-parent-ready','false');expect(panel()).toHaveAttribute('data-cg-parent-refresh-only','false');
 expect(screen.getByRole('button',{name:t.owner})).toBeDisabled();noWrites();
});

it('never consumes diagnostic DOM attributes as authorization',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture(true);records(f,f.ownerReady());render(<FactoryChildGoals {...props(f)} parentReady={false}/>);await open(user);
 const owner=await screen.findByRole('button',{name:t.owner});
 panel().setAttribute('data-cg-parent-ready','true');panel().setAttribute('data-cg-disabled','false');
 expect(owner).toBeDisabled();await user.click(owner);noWrites();
});
