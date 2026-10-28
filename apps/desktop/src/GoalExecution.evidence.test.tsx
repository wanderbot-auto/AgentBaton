import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { GoalExecution } from './GoalExecution';
import { goalApi } from './goal-api';
import { goalExecutionApi as api, goalRunViewShape, goalRunViewSupported, goalRunViewClosed, goalRunCanAdvanceView, type GoalRun, type GoalRunView } from './goal-execution-api';
import { goalExecutionFixture, executionView } from './goal-execution-test-fixtures';
vi.mock('./goal-execution-api',async original=>({...await original<typeof import('./goal-execution-api')>(),goalExecutionApi:{list:vi.fn(),get:vi.fn(),approve:vi.fn(),cancel:vi.fn(),advancePlan:vi.fn()}}));
vi.mock('./goal-api',async original=>({...await original<typeof import('./goal-api')>(),goalApi:{preview:vi.fn()}}));
type Fixture=ReturnType<typeof goalExecutionFixture>;
const approveName='Review Goal execution in native dialog',advanceName='Adopt the validated new plan',refreshName='Refresh execution records',recoverName='Recover the same execution request',cancelName='Request Goal run cancellation';
const unresolved='Node effects are unknown or their states are unsupported. Owned stop does not resolve effects; new approval and plan adoption stay blocked.';
const props=(f:Fixture)=>({task:f.task,scope:f.scope,latest:f.plan,language:'en' as const,connected:true,approvalAllowed:true});
const newer=(f:Fixture)=>({...structuredClone(f.plan),digest:'f'.repeat(64),proposal:{...structuredClone(f.plan.proposal),revision:2,expected_revision:1}});
const seed=(f:Fixture,run:GoalRun,view=executionView(f,run))=>{vi.mocked(api.list).mockResolvedValue([run]);vi.mocked(api.get).mockResolvedValue(view);vi.mocked(goalApi.preview).mockResolvedValue(f.preview);};
async function open(user:ReturnType<typeof userEvent.setup>){await user.click(screen.getByRole('heading',{name:'Goal execution'}));await waitFor(()=>expect(screen.getByRole('button',{name:refreshName})).toBeEnabled());}
const refresh=()=>screen.getByRole('button',{name:refreshName});
function unknownView(f:Fixture,run:GoalRun,stopped=true):GoalRunView{
  const view=executionView(f,{...run,state:'outcome_unknown'});
  return {...view,run,nodes:[{...view.nodes[0],state:'outcome_unknown',stop_confirmed:stopped,actual_runtime_ms:stopped?23:null}]};
}
beforeEach(()=>vi.resetAllMocks());
describe('known lifecycle and unknown-effect evidence, not server authority',()=>{
  it.each(['cancelled','failed','awaiting_acceptance'])('rejects terminal %s hiding unknown effects even after owned stop',state=>{
    const f=goalExecutionFixture(),run={...f.run,state,cancel_requested:state==='cancelled'};
    const view=unknownView(f,run,true);expect(goalRunViewShape(view,f.task,f.scope)).toBe(false);expect(goalRunViewClosed(view)).toBe(false);expect(goalRunCanAdvanceView(view)).toBe(false);
  });
  it.each(['cancelled','failed','awaiting_acceptance'])('rejects terminal %s hiding unsupported historical node states',state=>{
    const f=goalExecutionFixture(),run={...f.run,state,cancel_requested:state==='cancelled'};const view=executionView(f,run);view.nodes[0]={...view.nodes[0],state:'future_unknown_effect'};
    expect(goalRunViewShape(view,f.task,f.scope)).toBe(false);expect(goalRunViewSupported(view)).toBe(false);expect(goalRunCanAdvanceView(view)).toBe(false);
  });
  it('keeps a future node state readable in a blocked run without treating no attempt as a known safe effect',()=>{
    const f=goalExecutionFixture(),run={...f.run,state:'blocked'};const view={...f.view,run,nodes:[{...f.view.nodes[0],state:'future_unknown_effect'}]};
    expect(goalRunViewShape(view,f.task,f.scope)).toBe(true);expect(goalRunViewSupported(view)).toBe(false);expect(goalRunViewClosed(view)).toBe(false);expect(goalRunCanAdvanceView(view)).toBe(false);
  });
  it.each([false,true])('keeps truthful outcome_unknown readable, owned stop=%s never makes effects known',stopped=>{
    const f=goalExecutionFixture(),run={...f.run,state:'outcome_unknown'},view=unknownView(f,run,stopped);
    expect(goalRunViewShape(view,f.task,f.scope)).toBe(true);expect(goalRunViewSupported(view)).toBe(true);expect(goalRunViewClosed(view)).toBe(false);expect(goalRunCanAdvanceView(view)).toBe(false);
  });
  it.each(['candidate','cancelled','failed'])('does not manufacture known %s evidence without an attempt',state=>{
    const f=goalExecutionFixture(),view={...f.view,run:{...f.run,state:'blocked'},nodes:[{...f.view.nodes[0],state,stop_confirmed:true}]};
    expect(goalRunViewShape(view,f.task,f.scope)).toBe(false);expect(goalRunViewClosed(view)).toBe(false);expect(goalRunCanAdvanceView(view)).toBe(false);
  });
  it('requires not_started to have no attempted session or stop claim',()=>{
    const f=goalExecutionFixture(),view={...f.view,nodes:[{...f.view.nodes[0],session_id:'session',attempt_id:'attempt',stop_confirmed:true}]};expect(goalRunViewShape(view,f.task,f.scope)).toBe(false);expect(goalRunCanAdvanceView(view)).toBe(false);
  });
  it('permits a known stopped candidate plus never-started owner node, preserving the ordinary adoption path',()=>{
    const f=goalExecutionFixture(),run={...f.run,state:'blocked'},candidate=executionView(f,{...run,state:'awaiting_acceptance'}).nodes[0];const view={...f.view,run,nodes:[candidate,{...f.view.nodes[0],node:{...f.view.nodes[0].node,id:'owner'}}]};expect(goalRunViewShape(view,f.task,f.scope)).toBe(true);expect(goalRunViewClosed(view)).toBe(true);expect(goalRunCanAdvanceView(view)).toBe(true);
  });
  it('does not infer an advance capability from an unsupported run state either',()=>{
    const f=goalExecutionFixture(),view={...f.view,run:{...f.run,state:'future_run_state'}};expect(goalRunViewShape(view,f.task,f.scope)).toBe(true);expect(goalRunViewSupported(view)).toBe(false);expect(goalRunCanAdvanceView(view)).toBe(false);
  });
});
describe('actual component adversarial response controls, mocked transport',()=>{
  it('a contradictory cancelled record with unknown effects cannot enable a new native approval',async()=>{
    const user=userEvent.setup(),f=goalExecutionFixture(),run={...f.run,revision:3,state:'cancelled',cancel_requested:true};seed(f,run,unknownView(f,run,true));render(<GoalExecution {...props(f)}/>);await open(user);
    expect(await screen.findByRole('alert')).toHaveTextContent('Foreign, stale');expect(screen.getByRole('button',{name:approveName})).toBeDisabled();expect(api.approve).not.toHaveBeenCalled();expect(goalApi.preview).not.toHaveBeenCalled();
  });
  it('a future node state stays visible but cannot unlock adoption of another validated plan',async()=>{
    const user=userEvent.setup(),f=goalExecutionFixture(),run={...f.run,revision:3,state:'blocked'},view={...f.view,run,nodes:[{...f.view.nodes[0],state:'future_unknown_effect'}]};seed(f,run,view);render(<GoalExecution {...props(f)} latest={newer(f)}/>);await open(user);
    expect(screen.getByText('work · Unknown state: future_unknown_effect · Plan revision 1',{selector:'summary'})).toBeVisible();expect(screen.getByText(unresolved)).toBeVisible();expect(screen.getByRole('button',{name:advanceName})).toBeDisabled();expect(screen.getByRole('button',{name:approveName})).toBeDisabled();expect(api.advancePlan).not.toHaveBeenCalled();
  });
  it('a real cancellation acknowledgement resolves only its request intent while unknown effects remain blocked and visible',async()=>{
    const user=userEvent.setup(),f=goalExecutionFixture(),run={...f.run,revision:3,state:'outcome_unknown'};seed(f,run,unknownView(f,run,true));const ack={...run,revision:4,cancel_requested:true};vi.mocked(api.cancel).mockImplementation(async()=>{seed(f,ack,unknownView(f,ack,true));return ack;});render(<GoalExecution {...props(f)} latest={newer(f)}/>);await open(user);await user.click(screen.getByRole('button',{name:cancelName}));await waitFor(()=>expect(api.cancel).toHaveBeenCalledOnce());
    expect(screen.getByText('Outcome unknown; reconciliation required · '+run.id,{selector:'summary'})).toBeVisible();expect(screen.getByText(unresolved)).toBeVisible();expect(screen.queryByText(/^Cancellation recorded/,{selector:'summary'})).toBeNull();expect(screen.getByRole('button',{name:advanceName})).toBeDisabled();expect(screen.getByRole('button',{name:approveName})).toBeDisabled();expect(api.approve).not.toHaveBeenCalled();
  });
  it('does not clear an uncertain cancel intent from contradictory stopped history across remount',async()=>{
    const user=userEvent.setup(),f=goalExecutionFixture(),run={...f.run,revision:3,state:'running'};seed(f,run);vi.mocked(api.cancel).mockRejectedValue(new Error('cancel response lost'));const first=render(<GoalExecution {...props(f)}/>);await open(user);await user.click(screen.getByRole('button',{name:cancelName}));await screen.findByRole('alert');const sent=vi.mocked(api.cancel).mock.calls[0];first.unmount();
    const malformed={...run,revision:4,state:'cancelled',cancel_requested:true};seed(f,malformed,unknownView(f,malformed,true));render(<GoalExecution {...props(f)}/>);await open(user);expect(await screen.findByRole('alert')).toHaveTextContent('Foreign, stale');expect(screen.getByRole('button',{name:recoverName})).toBeDisabled();expect(api.cancel).toHaveBeenCalledOnce();expect(vi.mocked(api.cancel).mock.calls[0]).toEqual(sent);
    const known={...run,revision:4,state:'outcome_unknown',cancel_requested:true};seed(f,known,unknownView(f,known,true));await user.click(refresh());await waitFor(()=>expect(screen.queryByRole('button',{name:recoverName})).toBeNull());expect(screen.getByText(unresolved)).toBeVisible();expect(screen.getByRole('button',{name:approveName})).toBeDisabled();expect(api.cancel).toHaveBeenCalledOnce();
  });
  it('retains the exact uncertain adoption while future history is unsupported, without automatic recovery or another key',async()=>{
    const user=userEvent.setup(),f=goalExecutionFixture(),run={...f.run,revision:4,state:'awaiting_acceptance'},next=newer(f);seed(f,run);vi.mocked(api.advancePlan).mockRejectedValue(new Error('advance response lost'));render(<GoalExecution {...props(f)} latest={next}/>);await open(user);await user.click(screen.getByRole('button',{name:advanceName}));await screen.findByRole('alert');const sent=vi.mocked(api.advancePlan).mock.calls[0];
    const observed={...run,revision:6,state:'blocked',plan_revision:2,plan_digest:next.digest};const unsupported={...f.view,run:observed,nodes:[executionView(f,run).nodes[0],{...f.view.nodes[0],plan_revision:2,state:'future_unknown_effect'}]};seed(f,observed,unsupported);await user.click(refresh());expect(screen.getByRole('button',{name:recoverName})).toBeDisabled();expect(screen.getByText(unresolved)).toBeVisible();expect(api.advancePlan).toHaveBeenCalledOnce();expect(api.approve).not.toHaveBeenCalled();
    const supported={...unsupported,nodes:[unsupported.nodes[0],{...f.view.nodes[0],plan_revision:2}]};seed(f,observed,supported);await user.click(refresh());expect(screen.getByRole('button',{name:recoverName})).toBeEnabled();vi.mocked(api.advancePlan).mockResolvedValue({...observed,revision:5,state:'queued'});await user.click(screen.getByRole('button',{name:recoverName}));await waitFor(()=>expect(api.advancePlan).toHaveBeenCalledTimes(2));expect(vi.mocked(api.advancePlan).mock.calls[1]).toEqual(sent);await waitFor(()=>expect(screen.queryByRole('button',{name:recoverName})).toBeNull());expect(api.approve).not.toHaveBeenCalled();
  });
});
