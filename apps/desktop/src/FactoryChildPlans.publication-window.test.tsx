// Controlled mocked React publication; not actual IPC, browser or CI52 reproduction.
import { act,fireEvent,render,screen,waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach,beforeEach,expect,it,vi } from 'vitest';
import { FactoryChildPlans } from './FactoryChildPlans';
import { factoryChildPlanApi } from './factory-api';
import { DaemonError } from './api';
import { childPlanningFixture } from './factory-child-plan-test-fixtures';
import { childPlanMessages } from './factory-child-plan-i18n';
import type { FactoryChildPlanningContext,FactoryChildPlanView } from './factory-types';
const publication=vi.hoisted(()=>({hold:false,queue:[] as Array<()=>void>,proofSeen:false,finishSeen:false,finished:null as (()=>void)|null}));
vi.mock('react',async original=>{
 const actual=await original<typeof import('react')>();
 return {...actual,useState:(initial:unknown)=>{
  const [value,setValue]=actual.useState(initial);
  return [value,(action:unknown)=>{
   if(!publication.hold){setValue(action);return;}
   publication.queue.push(()=>setValue(action));
   // After the complete new proof, the next false publication is the read's
   // finally setLoading(false), after synchronous reading=false in the source.
   if(publication.proofSeen&&action===false&&!publication.finishSeen){publication.finishSeen=true;publication.finished?.();}
  }];
 }};
});
vi.mock('./factory-guards',async original=>{
 const actual=await original<typeof import('./factory-guards')>();
 return {...actual,factorySafeFingerprint:(value:unknown)=>{
  const result=actual.factorySafeFingerprint(value);
  if(publication.hold&&Array.isArray(value)&&value.length===5){
   const context=value[3] as FactoryChildPlanningContext|null,head=value[4] as FactoryChildPlanView|null;
   if(context?.current_plan_revision===1&&head?.reference.revision===1)publication.proofSeen=true;
  }
  return result;
 }};
});
vi.mock('./factory-api',()=>({factoryChildPlanApi:{context:vi.fn(),list:vi.fn(),get:vi.fn(),submit:vi.fn()}}));
vi.mock('./FactoryChildGoals',()=>({FactoryChildGoals:()=>null}));
const t=childPlanMessages.en;
beforeEach(()=>{vi.resetAllMocks();publication.hold=false;publication.queue=[];publication.proofSeen=false;publication.finishSeen=false;publication.finished=null;});
afterEach(()=>{publication.hold=false;publication.queue=[];publication.finished=null;});
it('does not send an old CAS after new read proof completes but before React publishes context/head',async()=>{
 const f=childPlanningFixture(),u=userEvent.setup();let head:FactoryChildPlanView|null=null;
 vi.mocked(factoryChildPlanApi.context).mockImplementation(async()=>({...structuredClone(f.context),current_plan_revision:head?.reference.revision??0}));
 vi.mocked(factoryChildPlanApi.list).mockImplementation(async()=>head?[structuredClone(head.reference)]:[]);
 vi.mocked(factoryChildPlanApi.get).mockImplementation(async()=>{if(!head)throw new DaemonError('NOT_FOUND','No plan','not_started');return structuredClone(head);});
 render(<FactoryChildPlans scope={f.scope} target={f.target} run={f.detail.run} language='en' connected parentReady parentReadGeneration={1}/>);
 await u.click(screen.getByText(new RegExp(`^${t.title} ·`),{selector:'summary'}));const field=await screen.findByLabelText(t.raw);
 const original=JSON.stringify(f.proposal);fireEvent.change(field,{target:{value:original}});const button=screen.getByRole('button',{name:t.store});await waitFor(()=>expect(button).toBeEnabled());
 head=await f.view(1);const finished=new Promise<void>(resolve=>{publication.finished=resolve;});publication.hold=true;
 try{
  await act(async()=>{
   fireEvent.click(screen.getByRole('button',{name:t.refresh}));
   await finished;
   expect(publication.proofSeen).toBe(true);expect(publication.finishSeen).toBe(true);
   // Every React state setter is still deferred, while the full proof has
   // changed and finally has cleared the synchronous reading marker. The old
   // enabled DOM must not be sufficient to dispatch the old plan/head CAS.
   expect(screen.getByText(/Current plan head: r0/)).toBeVisible();expect(button).toBeEnabled();fireEvent.click(button);
  });
  expect(factoryChildPlanApi.submit).not.toHaveBeenCalled();
 }finally{
  await act(async()=>{publication.hold=false;for(const publish of publication.queue.splice(0))publish();});
 }
 expect(await screen.findByText(/Current plan head: r1/)).toBeVisible();expect(screen.getByLabelText(t.raw)).toHaveValue(original);
 expect(screen.getByText(/^No submission was sent\./)).toBeVisible();expect(screen.queryByRole('button',{name:t.recover})).toBeNull();expect(button).toBeDisabled();
 // A later explicit fresh draft and an equal-valued manual read can become
 // ready again; synchronous withdrawal is not a permanent write lock.
 await u.click(screen.getByRole('button',{name:t.clear}));fireEvent.change(screen.getByLabelText(t.raw),{target:{value:JSON.stringify({...f.proposal,revision:2,expected_revision:1})}});
 await u.click(screen.getByRole('button',{name:t.refresh}));await waitFor(()=>expect(button).toBeEnabled());expect(factoryChildPlanApi.submit).not.toHaveBeenCalled();
 // Reuse the exact already-published DTO objects and batch a whole manual read.
 // Fresh/loading/token values finish equal to their initial values, so the
 // completed proof publication itself must still make a new render possible.
 const cachedContext=await vi.mocked(factoryChildPlanApi.context).mock.results.at(-1)!.value;
 const cachedRefs=await vi.mocked(factoryChildPlanApi.list).mock.results.at(-1)!.value;
 const cachedHead=await vi.mocked(factoryChildPlanApi.get).mock.results.at(-1)!.value;
 vi.mocked(factoryChildPlanApi.context).mockResolvedValue(cachedContext);vi.mocked(factoryChildPlanApi.list).mockResolvedValue(cachedRefs);vi.mocked(factoryChildPlanApi.get).mockResolvedValue(cachedHead);
 publication.proofSeen=false;publication.finishSeen=false;const sameFinished=new Promise<void>(resolve=>{publication.finished=resolve;});publication.hold=true;
 try{await act(async()=>{fireEvent.click(screen.getByRole('button',{name:t.refresh}));await sameFinished;});}
 finally{await act(async()=>{publication.hold=false;for(const publish of publication.queue.splice(0))publish();});}
 await waitFor(()=>expect(button).toBeEnabled());
 vi.mocked(factoryChildPlanApi.submit).mockRejectedValue(new DaemonError('RESOURCE_LIMIT','Fixed test limit','not_started'));
 await u.click(button);await waitFor(()=>expect(factoryChildPlanApi.submit).toHaveBeenCalledOnce());await screen.findByText(/^Plan submission refused: RESOURCE_LIMIT/);
 expect(vi.mocked(factoryChildPlanApi.submit).mock.calls[0]).toEqual([f.scope.id,f.task.id,JSON.stringify({...f.proposal,revision:2,expected_revision:1}),1,f.scope.revision,expect.any(String)]);
});
