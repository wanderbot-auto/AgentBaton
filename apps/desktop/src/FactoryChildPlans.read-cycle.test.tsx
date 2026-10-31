// Mocked React/transport contracts only; no Core permission, IPC or browser proof.
import { act,fireEvent,render,screen,waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach,beforeEach,describe,expect,it,vi } from 'vitest';
import { FactoryChildPlans } from './FactoryChildPlans';
import { factoryChildPlanApi } from './factory-api';
import { DaemonError } from './api';
import { childPlanningFixture } from './factory-child-plan-test-fixtures';
import { childPlanMessages } from './factory-child-plan-i18n';
import type { FactoryChildPlanningContext,FactoryChildPlanView } from './factory-types';
vi.mock('./factory-api',()=>({factoryChildPlanApi:{context:vi.fn(),list:vi.fn(),get:vi.fn(),submit:vi.fn()}}));
vi.mock('./FactoryChildGoals',()=>({FactoryChildGoals:()=>null}));
const t=childPlanMessages.en;
type F=ReturnType<typeof childPlanningFixture>;
function deferred<T>(){let resolve!:(value:T)=>void;const promise=new Promise<T>(r=>{resolve=r;});return {promise,resolve};}
function setup(f:F,views:FactoryChildPlanView[]=[]){
 vi.mocked(factoryChildPlanApi.context).mockImplementation(async()=>({...structuredClone(f.context),current_plan_revision:views.length}));
 vi.mocked(factoryChildPlanApi.list).mockImplementation(async()=>views.map(v=>structuredClone(v.reference)));
 vi.mocked(factoryChildPlanApi.get).mockImplementation(async(_s,_t,revision)=>{const view=revision?views[revision-1]:views.at(-1);if(!view)throw new DaemonError('NOT_FOUND','No plan','not_started');return structuredClone(view);});
}
const props=(f:F,generation=1)=>({scope:f.scope,target:f.target,run:f.detail.run,language:'en' as const,connected:true,parentReady:true,parentReadIdentity:'fixed-parent-read',parentReadGeneration:generation});
async function opened(user:ReturnType<typeof userEvent.setup>){await user.click(screen.getByText(new RegExp(`^${t.title} ·`),{selector:'summary'}));await waitFor(()=>expect(screen.getByRole('button',{name:t.refresh})).toBeEnabled());}
const draft=(f:F,revision=1)=>JSON.stringify({...f.proposal,revision,expected_revision:revision-1},null,2)+'\n';
const store=()=>screen.getByRole('button',{name:t.store});
function enter(f:F,revision=1){fireEvent.change(screen.getByLabelText(t.raw),{target:{value:draft(f,revision)}});}
beforeEach(()=>vi.resetAllMocks());
afterEach(()=>vi.restoreAllMocks());

describe('child-plan explicit read cycles',()=>{
 it('does not reread equal parent clones in one cycle, but reads a new MCP head on the next cycle',async()=>{
  const f=childPlanningFixture(),u=userEvent.setup();setup(f);const mounted=render(<FactoryChildPlans {...props(f)}/>);await opened(u);enter(f);
  expect(store()).toBeEnabled();mounted.rerender(<FactoryChildPlans {...props(f)} scope={structuredClone(f.scope)} target={structuredClone(f.target)} run={structuredClone(f.detail.run)}/>);
  await act(async()=>{});expect(factoryChildPlanApi.context).toHaveBeenCalledOnce();expect(store()).toBeEnabled();
  const v=await f.view();setup(f,[v]);mounted.rerender(<FactoryChildPlans {...props(f,2)}/>);
  expect(store()).toBeDisabled();expect(await screen.findByText(/Current plan head: r1/)).toBeVisible();
  expect(factoryChildPlanApi.context).toHaveBeenCalledTimes(2);expect(factoryChildPlanApi.list).toHaveBeenCalledTimes(2);expect(factoryChildPlanApi.get).toHaveBeenCalledTimes(2);
  expect(screen.getByLabelText(t.raw)).toHaveValue(draft(f));expect(screen.getByText(t.staleDraft)).toBeVisible();expect(store()).toBeDisabled();expect(factoryChildPlanApi.submit).not.toHaveBeenCalled();
 });
 it.each(['child-first','parent-first'] as const)('keeps both parent and complete local proof required with %s completion',async order=>{
  const f=childPlanningFixture(),u=userEvent.setup();setup(f);const mounted=render(<FactoryChildPlans {...props(f)}/>);await opened(u);enter(f);
  const held=deferred<FactoryChildPlanningContext>();vi.mocked(factoryChildPlanApi.context).mockImplementationOnce(()=>held.promise);
  mounted.rerender(<FactoryChildPlans {...props(f,2)} parentReady={false} parentRefreshOnly/>);expect(store()).toBeDisabled();
  await waitFor(()=>expect(factoryChildPlanApi.context).toHaveBeenCalledTimes(2));
  if(order==='parent-first'){
   mounted.rerender(<FactoryChildPlans {...props(f,2)} scope={structuredClone(f.scope)} target={structuredClone(f.target)} run={structuredClone(f.detail.run)}/>);expect(store()).toBeDisabled();
   await act(async()=>held.resolve(structuredClone(f.context)));
  }else{
   await act(async()=>held.resolve(structuredClone(f.context)));await waitFor(()=>expect(screen.getByRole('button',{name:t.refresh})).toBeEnabled());expect(store()).toBeDisabled();
   mounted.rerender(<FactoryChildPlans {...props(f,2)} scope={structuredClone(f.scope)} target={structuredClone(f.target)} run={structuredClone(f.detail.run)}/>);
  }
  await waitFor(()=>expect(store()).toBeEnabled());expect(factoryChildPlanApi.context).toHaveBeenCalledTimes(2);expect(factoryChildPlanApi.submit).not.toHaveBeenCalled();
 });
 it('rejects a late prior-generation read without publishing its older head',async()=>{
  const f=childPlanningFixture(),u=userEvent.setup();setup(f);const mounted=render(<FactoryChildPlans {...props(f)}/>);await opened(u);enter(f);
  const old=deferred<FactoryChildPlanningContext>();vi.mocked(factoryChildPlanApi.context).mockImplementationOnce(()=>old.promise);
  mounted.rerender(<FactoryChildPlans {...props(f,2)}/>);await waitFor(()=>expect(factoryChildPlanApi.context).toHaveBeenCalledTimes(2));
  const v1=await f.view(),v2=await f.view(2);setup(f,[v1,v2]);mounted.rerender(<FactoryChildPlans {...props(f,3)}/>);expect(await screen.findByText(/Current plan head: r2/)).toBeVisible();
  await act(async()=>old.resolve(structuredClone(f.context)));expect(screen.getByText(/Current plan head: r2/)).toBeVisible();expect(factoryChildPlanApi.list).toHaveBeenCalledTimes(2);expect(factoryChildPlanApi.submit).not.toHaveBeenCalled();
 });
 it('invalidates complete parent read identity even with unchanged scope, target and run',async()=>{
  const f=childPlanningFixture(),u=userEvent.setup();setup(f);const mounted=render(<FactoryChildPlans {...props(f)}/>);await opened(u);enter(f);
  const held=deferred<FactoryChildPlanningContext>();vi.mocked(factoryChildPlanApi.context).mockImplementationOnce(()=>held.promise);
  mounted.rerender(<FactoryChildPlans {...props(f)} parentReadIdentity='changed-current-grant-or-connection'/>);expect(store()).toBeDisabled();
  await act(async()=>held.resolve(structuredClone(f.context)));await waitFor(()=>expect(store()).toBeEnabled());expect(factoryChildPlanApi.submit).not.toHaveBeenCalled();
 });
 it.each(['revoked','expired','cancel','future','disconnected','parent-unfresh'] as const)('never lets an older held proof restore new writes after %s',async kind=>{
  const f=childPlanningFixture(),u=userEvent.setup();setup(f);const mounted=render(<FactoryChildPlans {...props(f)}/>);await opened(u);enter(f);
  const old=deferred<FactoryChildPlanningContext>();vi.mocked(factoryChildPlanApi.context).mockImplementationOnce(()=>old.promise);
  mounted.rerender(<FactoryChildPlans {...props(f,2)}/>);await waitFor(()=>expect(factoryChildPlanApi.context).toHaveBeenCalledTimes(2));
  const scope=structuredClone(f.scope),run=structuredClone(f.detail.run);
  if(kind==='revoked'){scope.revoked=true;scope.revision=2;}if(kind==='expired')scope.expires_at_ms=1;if(kind==='cancel')run.cancel_requested=true;if(kind==='future')run.state='future';
  mounted.rerender(<FactoryChildPlans {...props(f,3)} scope={scope} run={run} connected={kind!=='disconnected'} parentReady={kind!=='parent-unfresh'}/>);expect(store()).toBeDisabled();
  await act(async()=>old.resolve(structuredClone(f.context)));if(kind!=='disconnected')await waitFor(()=>expect(screen.getByRole('button',{name:t.refresh})).toBeEnabled());expect(store()).toBeDisabled();expect(factoryChildPlanApi.submit).not.toHaveBeenCalled();
 });
 it('observes expiry at the actual intent without rerendering and retains not-sent notice through reads',async()=>{
  const f=childPlanningFixture(),u=userEvent.setup();setup(f);render(<FactoryChildPlans {...props(f)}/>);await opened(u);enter(f);expect(store()).toBeEnabled();
  const clock=vi.spyOn(Date,'now').mockReturnValue(f.scope.expires_at_ms+1);
  fireEvent.click(store());expect(factoryChildPlanApi.submit).not.toHaveBeenCalled();expect(screen.getByRole('alert')).toHaveTextContent('No submission was sent.');
  clock.mockRestore();await u.click(screen.getByRole('button',{name:t.refresh}));await waitFor(()=>expect(screen.getByRole('button',{name:t.refresh})).toBeEnabled());expect(screen.getByRole('alert')).toHaveTextContent('No submission was sent.');expect(factoryChildPlanApi.submit).not.toHaveBeenCalled();
 });
 it('keeps a local parse refusal visible through read cycles until explicit editing',async()=>{
  const f=childPlanningFixture(),u=userEvent.setup();setup(f);const mounted=render(<FactoryChildPlans {...props(f)}/>);await opened(u);
  fireEvent.change(screen.getByLabelText(t.raw),{target:{value:'{"schema_version":1,"schema_version":1}'}});await u.click(store());expect(screen.getByRole('alert')).toHaveTextContent(t.denied);
  mounted.rerender(<FactoryChildPlans {...props(f,2)}/>);await waitFor(()=>expect(screen.getByRole('button',{name:t.refresh})).toBeEnabled());expect(screen.getByRole('alert')).toHaveTextContent(t.denied);expect(factoryChildPlanApi.submit).not.toHaveBeenCalled();
  enter(f);expect(screen.queryByRole('alert')).toBeNull();
 });
 it('keeps unknown original bytes/key/CAS across read cycles and sends no duplicate intent automatically',async()=>{
  const f=childPlanningFixture(),u=userEvent.setup();setup(f);vi.mocked(factoryChildPlanApi.submit).mockRejectedValue(new Error('lost after commit'));
  const mounted=render(<FactoryChildPlans {...props(f)}/>);await opened(u);enter(f);fireEvent.click(store());fireEvent.click(store());
  await waitFor(()=>expect(screen.getByRole('alert')).toHaveTextContent(t.pending));expect(factoryChildPlanApi.submit).toHaveBeenCalledOnce();
  const original=vi.mocked(factoryChildPlanApi.submit).mock.calls[0],v=await f.view(1,original[2]);setup(f,[v]);mounted.rerender(<FactoryChildPlans {...props(f,2)}/>);
  await waitFor(()=>expect(screen.getByRole('button',{name:t.recover})).toBeEnabled());expect(screen.getByRole('alert')).toHaveTextContent(t.pending);expect(factoryChildPlanApi.submit).toHaveBeenCalledOnce();expect(store()).toBeDisabled();
  vi.mocked(factoryChildPlanApi.submit).mockResolvedValue(v);await u.click(screen.getByRole('button',{name:t.recover}));await waitFor(()=>expect(screen.queryByRole('button',{name:t.recover})).toBeNull());
  expect(vi.mocked(factoryChildPlanApi.submit).mock.calls[1]).toEqual(original);expect(screen.queryByRole('alert')).toBeNull();
 });
 it('records a committed receipt as history after revocation without restoring new-write authority',async()=>{
  const f=childPlanningFixture(),u=userEvent.setup();setup(f);const held=deferred<FactoryChildPlanView>();vi.mocked(factoryChildPlanApi.submit).mockImplementation(()=>held.promise);
  const mounted=render(<FactoryChildPlans {...props(f)}/>);await opened(u);enter(f);await u.click(store());expect(factoryChildPlanApi.submit).toHaveBeenCalledOnce();
  const original=vi.mocked(factoryChildPlanApi.submit).mock.calls[0],v=await f.view(1,original[2]);setup(f,[v]);const revoked={...structuredClone(f.scope),revision:2,revoked:true};
  mounted.rerender(<FactoryChildPlans {...props(f,2)} scope={revoked}/>);expect(store()).toBeDisabled();
  await act(async()=>held.resolve(v));await waitFor(()=>expect(screen.queryByRole('button',{name:t.recover})).toBeNull());expect(await screen.findByText(/Current plan head: r1/)).toBeVisible();
  expect(store()).toBeDisabled();expect(screen.getByText(t.readonly)).toBeVisible();expect(factoryChildPlanApi.submit).toHaveBeenCalledOnce();
 });
 it('retains definite not_started diagnosis across a later complete read without replay',async()=>{
  const f=childPlanningFixture(),u=userEvent.setup();setup(f);vi.mocked(factoryChildPlanApi.submit).mockRejectedValue(new DaemonError('RESOURCE_LIMIT','Fixed revision budget','not_started'));
  const mounted=render(<FactoryChildPlans {...props(f)}/>);await opened(u);enter(f);await u.click(store());await waitFor(()=>expect(screen.getByRole('alert')).toHaveTextContent('RESOURCE_LIMIT'));
  expect(screen.queryByRole('button',{name:t.recover})).toBeNull();expect(store()).toBeDisabled();mounted.rerender(<FactoryChildPlans {...props(f,2)}/>);
  await waitFor(()=>expect(screen.getByRole('button',{name:t.refresh})).toBeEnabled());expect(screen.getByRole('alert')).toHaveTextContent('RESOURCE_LIMIT');expect(factoryChildPlanApi.submit).toHaveBeenCalledOnce();
 });

});
