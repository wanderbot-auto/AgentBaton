// Mocked renderer read-state controls only; no daemon, Native approval or Provider.
import { act,render,screen,waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach,it,expect,vi } from 'vitest';
import { FactoryRuntime } from './FactoryRuntime';
import { childGoalFixture } from './factory-child-goal-test-fixtures';
import { factoryApi } from './factory-api';
import { sessionApi,type ExecutionGrant } from './session-api';
import { api } from './api';
import { factoryMessages } from './factory-i18n';
vi.mock('./FactoryChildPlans',()=>({FactoryChildPlans:(p:{parentReady:boolean;parentRefreshOnly?:boolean;parentReadIdentity?:string})=><output data-testid="ancestor-read-state" data-ready={String(p.parentReady)} data-refresh={String(!!p.parentRefreshOnly)} data-identity={p.parentReadIdentity??''}/> }));
vi.mock('./factory-api',()=>({factoryApi:{approve:vi.fn(),get:vi.fn(),list:vi.fn(),revoke:vi.fn(),start:vi.fn(),run:vi.fn(),cancel:vi.fn()}}));
vi.mock('./api',async()=>{const a=await vi.importActual<typeof import('./api')>('./api');return {...a,api:{workspacesList:vi.fn()}};});
vi.mock('./session-api',()=>({sessionApi:{grantsList:vi.fn(),connectionsList:vi.fn()}}));
const t=factoryMessages.en;
type F=Awaited<ReturnType<typeof childGoalFixture>>;
function setup(f:F){
 vi.mocked(sessionApi.grantsList).mockImplementation(async()=>[structuredClone(f.grant)]);
 vi.mocked(sessionApi.connectionsList).mockImplementation(async()=>[structuredClone(f.connection)]);
 vi.mocked(api.workspacesList).mockImplementation(async()=>[structuredClone(f.workspace)]);
 vi.mocked(factoryApi.list).mockImplementation(async()=>[structuredClone(f.scope)]);
 vi.mocked(factoryApi.run).mockImplementation(async()=>structuredClone(f.detail));
}
async function opened(user:ReturnType<typeof userEvent.setup>){await user.click(screen.getByRole('heading',{name:t.title}));await waitFor(()=>expect(screen.getByTestId('ancestor-read-state')).toHaveAttribute('data-ready','true'));}
beforeEach(()=>vi.resetAllMocks());
it('marks only a known unchanged ancestor snapshot as a read-only refresh',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();setup(f);render(<FactoryRuntime task={f.task} language="en" connected/>);await opened(user);
 const before=screen.getByTestId('ancestor-read-state').getAttribute('data-identity');let release!:(v:ExecutionGrant[])=>void;
 vi.mocked(sessionApi.grantsList).mockImplementationOnce(()=>new Promise(resolve=>{release=resolve;}));
 await user.click(screen.getByRole('button',{name:t.refresh}));expect(screen.getByTestId('ancestor-read-state')).toHaveAttribute('data-ready','false');expect(screen.getByTestId('ancestor-read-state')).toHaveAttribute('data-refresh','true');expect(screen.getByTestId('ancestor-read-state')).toHaveAttribute('data-identity',before);
 await act(async()=>release([structuredClone(f.grant)]));await waitFor(()=>expect(screen.getByTestId('ancestor-read-state')).toHaveAttribute('data-ready','true'));expect(screen.getByTestId('ancestor-read-state')).toHaveAttribute('data-refresh','false');expect(factoryApi.start).not.toHaveBeenCalled();
});
it('does not give the first unverified read a same-snapshot marker',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();setup(f);let release!:(v:ExecutionGrant[])=>void;
 vi.mocked(sessionApi.grantsList).mockImplementationOnce(()=>new Promise(resolve=>{release=resolve;}));
 render(<FactoryRuntime task={f.task} language="en" connected/>);await user.click(screen.getByRole('heading',{name:t.title}));expect(screen.queryByTestId('ancestor-read-state')).toBeNull();await act(async()=>release([structuredClone(f.grant)]));await waitFor(()=>expect(screen.getByTestId('ancestor-read-state')).toHaveAttribute('data-ready','true'));
});
it('withdraws the refresh marker when the root business snapshot changed',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();setup(f);const mounted=render(<FactoryRuntime task={f.task} language="en" connected/>);await opened(user);
 let release!:(v:ExecutionGrant[])=>void;const held=new Promise<ExecutionGrant[]>(resolve=>{release=resolve;});vi.mocked(sessionApi.grantsList).mockImplementation(()=>held);
 await user.click(screen.getByRole('button',{name:t.refresh}));expect(screen.getByTestId('ancestor-read-state')).toHaveAttribute('data-refresh','true');
 mounted.rerender(<FactoryRuntime task={{...f.task,revision:2,spec_revision:2,goal:'Changed root specification'}} language="en" connected/>);
 expect(screen.getByTestId('ancestor-read-state')).toHaveAttribute('data-refresh','false');await act(async()=>release([structuredClone(f.grant)]));
});
it('does not label an unknown ancestor write as read-only refresh',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();setup(f);vi.mocked(factoryApi.revoke).mockRejectedValue(new Error('lost private approval response'));
 render(<FactoryRuntime task={f.task} language="en" connected/>);await opened(user);await user.click(screen.getByRole('button',{name:t.revoke}));
 await screen.findByRole('alert');expect(screen.getByTestId('ancestor-read-state')).toHaveAttribute('data-ready','false');expect(screen.getByTestId('ancestor-read-state')).toHaveAttribute('data-refresh','false');expect(factoryApi.revoke).toHaveBeenCalledOnce();expect(factoryApi.start).not.toHaveBeenCalled();
});
it('does not reuse read provenance after a failed refresh',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();setup(f);render(<FactoryRuntime task={f.task} language="en" connected/>);await opened(user);
 vi.mocked(factoryApi.list).mockRejectedValueOnce(new Error('read disconnected'));
 await user.click(screen.getByRole('button',{name:t.refresh}));await screen.findByRole('alert');
 let release!:(v:ExecutionGrant[])=>void;vi.mocked(sessionApi.grantsList).mockImplementationOnce(()=>new Promise(resolve=>{release=resolve;}));
 await user.click(screen.getByRole('button',{name:t.refresh}));expect(screen.getByTestId('ancestor-read-state')).toHaveAttribute('data-refresh','false');await act(async()=>release([structuredClone(f.grant)]));await waitFor(()=>expect(screen.getByTestId('ancestor-read-state')).toHaveAttribute('data-ready','true'));
});
