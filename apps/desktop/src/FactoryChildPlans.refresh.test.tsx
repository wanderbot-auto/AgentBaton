// Mocked plan-panel read-state provenance. No backend or Owner approval runs.
import { act,render,screen,waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach,it,expect,vi } from 'vitest';
import { FactoryChildPlans } from './FactoryChildPlans';
import { factoryChildPlanApi } from './factory-api';
import { childGoalFixture } from './factory-child-goal-test-fixtures';
import type { FactoryChildPlanningContext } from './factory-types';
import { childPlanMessages } from './factory-child-plan-i18n';
vi.mock('./FactoryChildGoals',()=>({FactoryChildGoals:(p:{parentReady:boolean;parentRefreshOnly?:boolean;parentReadIdentity?:string})=><output data-testid="plan-read-state" data-ready={String(p.parentReady)} data-refresh={String(!!p.parentRefreshOnly)} data-identity={p.parentReadIdentity??''}/> }));
vi.mock('./factory-api',()=>({factoryChildPlanApi:{context:vi.fn(),get:vi.fn(),list:vi.fn(),submit:vi.fn()}}));
const t=childPlanMessages.en;type F=Awaited<ReturnType<typeof childGoalFixture>>;
function setup(f:F){
 vi.mocked(factoryChildPlanApi.context).mockImplementation(async()=>structuredClone(f.context));
 vi.mocked(factoryChildPlanApi.list).mockImplementation(async()=>[structuredClone(f.plan.reference)]);
 vi.mocked(factoryChildPlanApi.get).mockImplementation(async()=>structuredClone(f.plan));
}
const props=(f:F)=>({scope:f.scope,target:f.target,run:f.detail.run,language:'en' as const,connected:true,parentReady:true,parentReadIdentity:'known-root'});
async function opened(user:ReturnType<typeof userEvent.setup>){await user.click(screen.getByText(new RegExp(`^${t.title} ·`),{selector:'summary'}));await waitFor(()=>expect(screen.getByTestId('plan-read-state')).toHaveAttribute('data-ready','true'));}
beforeEach(()=>vi.resetAllMocks());
it('marks an identical validated local plan reload without inventing readiness on first read',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();setup(f);let release!:(v:FactoryChildPlanningContext)=>void;
 vi.mocked(factoryChildPlanApi.context).mockImplementationOnce(()=>new Promise(resolve=>{release=resolve;}));
 render(<FactoryChildPlans {...props(f)}/>);await user.click(screen.getByText(new RegExp(`^${t.title} ·`),{selector:'summary'}));expect(screen.getByTestId('plan-read-state')).toHaveAttribute('data-refresh','false');
 await act(async()=>release(structuredClone(f.context)));await waitFor(()=>expect(screen.getByTestId('plan-read-state')).toHaveAttribute('data-ready','true'));
 const identity=screen.getByTestId('plan-read-state').getAttribute('data-identity');vi.mocked(factoryChildPlanApi.context).mockImplementationOnce(()=>new Promise(resolve=>{release=resolve;}));
 await user.click(screen.getByRole('button',{name:t.refresh}));expect(screen.getByTestId('plan-read-state')).toHaveAttribute('data-ready','false');expect(screen.getByTestId('plan-read-state')).toHaveAttribute('data-refresh','true');expect(screen.getByTestId('plan-read-state')).toHaveAttribute('data-identity',identity);
 await act(async()=>release(structuredClone(f.context)));await waitFor(()=>expect(screen.getByTestId('plan-read-state')).toHaveAttribute('data-ready','true'));expect(factoryChildPlanApi.submit).not.toHaveBeenCalled();
});
it('passes through an ancestor read-only refresh only after local verification',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();setup(f);const mounted=render(<FactoryChildPlans {...props(f)}/>);await opened(user);
 mounted.rerender(<FactoryChildPlans {...props(f)} parentReady={false} parentRefreshOnly/>);expect(screen.getByTestId('plan-read-state')).toHaveAttribute('data-ready','false');expect(screen.getByTestId('plan-read-state')).toHaveAttribute('data-refresh','true');
 mounted.rerender(<FactoryChildPlans {...props(f)} parentReady={false} parentRefreshOnly={false}/>);expect(screen.getByTestId('plan-read-state')).toHaveAttribute('data-refresh','false');
});
it('rejects a different target while a local reload is pending',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();setup(f);const mounted=render(<FactoryChildPlans {...props(f)}/>);await opened(user);
 let release!:(v:FactoryChildPlanningContext)=>void;const held=new Promise<FactoryChildPlanningContext>(resolve=>{release=resolve;});vi.mocked(factoryChildPlanApi.context).mockImplementation(()=>held);
 await user.click(screen.getByRole('button',{name:t.refresh}));expect(screen.getByTestId('plan-read-state')).toHaveAttribute('data-refresh','true');mounted.rerender(<FactoryChildPlans {...props(f)} target={{...f.target,revision:2}}/>);expect(screen.getByTestId('plan-read-state')).toHaveAttribute('data-refresh','false');await act(async()=>release(structuredClone(f.context)));
});
it('withdraws local read provenance after an error until a complete read succeeds',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();setup(f);render(<FactoryChildPlans {...props(f)}/>);await opened(user);
 vi.mocked(factoryChildPlanApi.context).mockRejectedValueOnce(new Error('readonly transport failed'));await user.click(screen.getByRole('button',{name:t.refresh}));await screen.findByRole('alert');
 let release!:(v:FactoryChildPlanningContext)=>void;vi.mocked(factoryChildPlanApi.context).mockImplementationOnce(()=>new Promise(resolve=>{release=resolve;}));await user.click(screen.getByRole('button',{name:t.refresh}));expect(screen.getByTestId('plan-read-state')).toHaveAttribute('data-refresh','false');await act(async()=>release(structuredClone(f.context)));await waitFor(()=>expect(screen.getByTestId('plan-read-state')).toHaveAttribute('data-ready','true'));
});
