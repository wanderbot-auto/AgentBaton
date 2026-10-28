import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { GoalExecution } from './GoalExecution';
import { DaemonError } from './api';
import { goalApi } from './goal-api';
import { goalExecutionApi as api, type GoalRun } from './goal-execution-api';
import { goalExecutionFixture, executionView } from './goal-execution-test-fixtures';
vi.mock('./goal-execution-api',async original=>({...await original<typeof import('./goal-execution-api')>(),goalExecutionApi:{list:vi.fn(),get:vi.fn(),approve:vi.fn(),cancel:vi.fn(),advancePlan:vi.fn()}}));
vi.mock('./goal-api',async original=>({...await original<typeof import('./goal-api')>(),goalApi:{preview:vi.fn()}}));
type Fixture=ReturnType<typeof goalExecutionFixture>;
const approveName='Review Goal execution in native dialog',refreshName='Refresh execution records',recoverName='Recover the same execution request',cancelName='Request Goal run cancellation',advanceName='Adopt the validated new plan';
function setup(f:Fixture,rows:GoalRun[]=[]){vi.mocked(api.list).mockResolvedValue(rows);vi.mocked(api.get).mockImplementation(async id=>executionView(f,rows.find(row=>row.id===id)!));vi.mocked(goalApi.preview).mockResolvedValue(f.preview);}
function props(f:Fixture){return {task:f.task,scope:f.scope,latest:f.plan,language:'en' as const,connected:true,approvalAllowed:true};}
async function open(user:ReturnType<typeof userEvent.setup>){await user.click(screen.getByRole('heading',{name:'Goal execution'}));await waitFor(()=>expect(screen.getByRole('button',{name:refreshName})).toBeEnabled());}
const refresh=()=>screen.getByRole('button',{name:refreshName});
async function loseCancel(user:ReturnType<typeof userEvent.setup>,f:Fixture){
  const running={...f.run,state:'running',revision:3};setup(f,[running]);vi.mocked(api.cancel).mockRejectedValue(new Error('response lost'));
  const view=render(<GoalExecution {...props(f)}/>);await open(user);await user.click(screen.getByRole('button',{name:cancelName}));expect(await screen.findByRole('alert')).toHaveTextContent('Request outcome unknown');return view;
}
beforeEach(()=>{vi.resetAllMocks();vi.mocked(api.list).mockResolvedValue([]);});
afterEach(()=>{vi.useRealTimers();});
describe('Goal execution UI, mocked transport',()=>{
  it('starts folded and reads scoped history only on expansion, with no automatic mutation',async()=>{
    const user=userEvent.setup(),f=goalExecutionFixture();setup(f);render(<GoalExecution {...props(f)}/>);
    expect(api.list).not.toHaveBeenCalled();expect(goalApi.preview).not.toHaveBeenCalled();await open(user);
    expect(api.list).toHaveBeenCalledWith(f.task.id);expect(api.approve).not.toHaveBeenCalled();expect(api.cancel).not.toHaveBeenCalled();expect(api.advancePlan).not.toHaveBeenCalled();expect(screen.getByRole('button',{name:approveName})).toBeEnabled();
  });
  it('reads a fresh exact ledger and sends only the independent native execution selection once',async()=>{
    const user=userEvent.setup(),f=goalExecutionFixture();setup(f);
    vi.mocked(api.approve).mockImplementation(async()=>{setup(f,[f.run]);return f.run;});render(<GoalExecution {...props(f)}/>);await open(user);await user.click(screen.getByRole('button',{name:approveName}));
    await waitFor(()=>expect(api.approve).toHaveBeenCalledOnce());expect(api.approve).toHaveBeenCalledWith({scope_id:f.scope.id,expected_plan_revision:1,expected_ledger_revision:12},expect.any(String));
    expect(goalApi.preview).toHaveBeenCalledWith(f.scope.id,1);expect(await screen.findByText(`Queued · ${f.run.id}`,{selector:'summary'})).toBeVisible();expect(api.advancePlan).not.toHaveBeenCalled();expect(api.cancel).not.toHaveBeenCalled();
  });
  it.each(['scope','state','dispatched','input'])('does not offer execution of a mismatched or unvalidated stored plan %s',async kind=>{
    const user=userEvent.setup(),f=goalExecutionFixture();setup(f);const latest=structuredClone(f.plan);if(kind==='scope')latest.scope_id='foreign';if(kind==='state')latest.state='model_says_approved';if(kind==='dispatched')latest.dispatched=true;if(kind==='input')latest.proposal.goal.input_revision='foreign';
    render(<GoalExecution {...props(f)} latest={latest}/>);await open(user);expect(screen.getByRole('button',{name:approveName})).toBeDisabled();expect(goalApi.preview).not.toHaveBeenCalled();expect(api.approve).not.toHaveBeenCalled();
  });
  it('a declined native dialog clears only a definitely not-started intent and never claims a run',async()=>{
    const user=userEvent.setup(),f=goalExecutionFixture();setup(f);vi.mocked(api.approve).mockRejectedValue(new DaemonError('PERMISSION_DENIED','Declined','not_started'));
    render(<GoalExecution {...props(f)}/>);await open(user);await user.click(screen.getByRole('button',{name:approveName}));expect(await screen.findByRole('alert')).toHaveTextContent('Native approval was declined');expect(screen.queryByRole('button',{name:recoverName})).toBeNull();expect(api.approve).toHaveBeenCalledOnce();expect(api.cancel).not.toHaveBeenCalled();
  });
  it.each([{scope_id:'foreign'},{plan_revision:3},{read_only:false},{reservation_made:true},{dispatched:true},{journal_sequence:Number.MAX_SAFE_INTEGER+1},{decision:{plan_digest:'f'.repeat(64),expected_plan_revision:1,expected_ledger_revision:12,admissions:[],blocked:{},required_checks_satisfied:false,requires_atomic_store_commit:true}}])('rejects stale or authority-bearing preapproval preview %j',async patch=>{
    const user=userEvent.setup(),f=goalExecutionFixture();setup(f);vi.mocked(goalApi.preview).mockResolvedValue({...f.preview,...patch});render(<GoalExecution {...props(f)}/>);await open(user);await user.click(screen.getByRole('button',{name:approveName}));expect(await screen.findByRole('alert')).toHaveTextContent('Foreign, stale');expect(api.approve).not.toHaveBeenCalled();expect(screen.getByRole('button',{name:approveName})).toBeDisabled();
  });
  it('retains the exact approval key/selection across remount and only explicitly recovers after a fresh scoped read',async()=>{
    const user=userEvent.setup(),f=goalExecutionFixture();setup(f);vi.mocked(api.approve).mockRejectedValue(new Error('lost reply'));
    const first=render(<GoalExecution {...props(f)}/>);await open(user);await user.click(screen.getByRole('button',{name:approveName}));expect(await screen.findByRole('alert')).toHaveTextContent('Request outcome unknown');const sent=vi.mocked(api.approve).mock.calls[0];first.unmount();
    render(<GoalExecution {...props(f)}/>);await open(user);expect(api.approve).toHaveBeenCalledOnce();expect(screen.getByRole('button',{name:recoverName})).toBeEnabled();
    vi.mocked(api.approve).mockImplementation(async()=>{setup(f,[f.run]);return f.run;});await user.click(screen.getByRole('button',{name:recoverName}));await waitFor(()=>expect(api.approve).toHaveBeenCalledTimes(2));expect(vi.mocked(api.approve).mock.calls[1]).toEqual(sent);await waitFor(()=>expect(screen.queryByRole('button',{name:recoverName})).toBeNull());expect(goalApi.preview).toHaveBeenCalledOnce();
  });
  it('retains a missing acknowledgement even if another matching run appears, rather than inventing its request receipt',async()=>{
    const user=userEvent.setup(),f=goalExecutionFixture();setup(f);vi.mocked(api.approve).mockRejectedValue(new Error('lost reply'));render(<GoalExecution {...props(f)}/>);await open(user);await user.click(screen.getByRole('button',{name:approveName}));await screen.findByRole('alert');setup(f,[f.run]);await user.click(refresh());expect(screen.getByRole('button',{name:recoverName})).toBeEnabled();expect(api.approve).toHaveBeenCalledOnce();expect(screen.getByRole('button',{name:approveName})).toBeDisabled();
  });
  it.each([2,3])('does not discard cancellation version floors on remount or accept a stale/changed terminal revision%s',async revision=>{
    const user=userEvent.setup(),f=goalExecutionFixture(),first=await loseCancel(user,f);first.unmount();
    const stale={...f.run,revision,state:'cancelled',cancel_requested:true};setup(f,[stale]);render(<GoalExecution {...props(f)}/>);await open(user);expect(await screen.findByRole('alert')).toHaveTextContent('Foreign, stale');expect(screen.getByRole('button',{name:recoverName})).toBeDisabled();expect(api.cancel).toHaveBeenCalledOnce();
    const requested={...f.run,revision:4,state:'cancel_requested',cancel_requested:true};setup(f,[requested]);await user.click(refresh());await waitFor(()=>expect(screen.queryByRole('button',{name:recoverName})).toBeNull());expect(screen.getByText(`Cancellation requested; stop not yet confirmed · ${f.run.id}`,{selector:'summary'})).toBeVisible();expect(screen.queryByText(/^Cancellation recorded/,{selector:'summary'})).toBeNull();expect(api.cancel).toHaveBeenCalledOnce();
  });
  it('retains an uncertain cancellation when the global ledger regresses despite a newer run revision',async()=>{
    const user=userEvent.setup(),f=goalExecutionFixture();await loseCancel(user,f);const requested={...f.run,revision:4,state:'cancel_requested',cancel_requested:true};setup(f,[requested]);vi.mocked(api.get).mockResolvedValue({...executionView(f,requested),ledger_revision:13});await user.click(refresh());expect(await screen.findByRole('alert')).toHaveTextContent('Foreign, stale');expect(screen.getByRole('button',{name:recoverName})).toBeDisabled();expect(api.cancel).toHaveBeenCalledOnce();vi.mocked(api.get).mockResolvedValue({...executionView(f,requested),ledger_revision:15});await user.click(refresh());await waitFor(()=>expect(screen.queryByRole('button',{name:recoverName})).toBeNull());expect(api.cancel).toHaveBeenCalledOnce();
  });
  it('displays a candidate and unknown measurements separately from declared reservations without Task acceptance',async()=>{
    const user=userEvent.setup(),f=goalExecutionFixture(),candidate={...f.run,revision:4,state:'awaiting_acceptance'};setup(f,[candidate]);render(<GoalExecution {...props(f)}/>);await open(user);await user.click(screen.getByText('work · candidate · Plan revision 1',{selector:'summary'}));
    expect(screen.getByText('Execution candidate; acceptance pending · '+f.run.id,{selector:'summary'})).toBeVisible();expect(screen.getByRole('heading',{name:'Declared per-attempt reservation'})).toBeVisible();expect(screen.getByRole('heading',{name:'Observed usage'})).toBeVisible();expect(screen.getAllByText('Not measured / unknown').length).toBeGreaterThanOrEqual(4);expect(screen.getByText('1000')).toBeVisible();expect(screen.getByText('23')).toBeVisible();expect(screen.queryByText('Task accepted')).toBeNull();expect(screen.getByRole('button',{name:approveName})).toBeDisabled();
  });
  it.each(['foreign','accepted','unjoined'])('blocks new actions when detail evidence is %s',async kind=>{
    const user=userEvent.setup(),f=goalExecutionFixture(),candidate={...f.run,state:'awaiting_acceptance'};setup(f,[candidate]);const detail=executionView(f,candidate);
    if(kind==='foreign')vi.mocked(api.get).mockResolvedValue({...detail,run:{...candidate,scope_id:'foreign'}});if(kind==='accepted')vi.mocked(api.get).mockResolvedValue({...detail,task_accepted:true} as unknown as typeof detail);if(kind==='unjoined')vi.mocked(api.get).mockResolvedValue({...detail,nodes:[{...detail.nodes[0],stop_confirmed:false}]});
    render(<GoalExecution {...props(f)}/>);await open(user);expect(await screen.findByRole('alert')).toHaveTextContent('Foreign, stale');expect(screen.getByRole('button',{name:approveName})).toBeDisabled();expect(api.approve).not.toHaveBeenCalled();
  });
  it('an unsupported runtime state remains unknown and cannot unlock approval or fabricate a cancellation action',async()=>{
    const user=userEvent.setup(),f=goalExecutionFixture(),run={...f.run,state:'future_state'};setup(f,[run]);render(<GoalExecution {...props(f)}/>);await open(user);expect(screen.getByText('Unknown state: future_state · '+run.id,{selector:'summary'})).toBeVisible();expect(screen.getByRole('button',{name:approveName})).toBeDisabled();expect(screen.queryByRole('button',{name:cancelName})).toBeNull();
  });
  it('holds a pending scope across scope navigation and does not send its recovery into another scope',async()=>{
    const user=userEvent.setup(),f=goalExecutionFixture();setup(f);vi.mocked(api.approve).mockRejectedValue(new Error('lost reply'));const tree=render(<GoalExecution {...props(f)}/>);await open(user);await user.click(screen.getByRole('button',{name:approveName}));await screen.findByRole('alert');
    tree.rerender(<GoalExecution {...props(f)} scope={{...f.scope,id:'another-scope'}}/>);expect(await screen.findByRole('alert')).toHaveTextContent('another Goal scope');expect(screen.getByRole('button',{name:recoverName})).toBeDisabled();expect(api.approve).toHaveBeenCalledOnce();
  });
  it('uses an in-flight guard while preparing a native approval and never submits twice',async()=>{
    const user=userEvent.setup(),f=goalExecutionFixture();setup(f);let release!:(value:typeof f.preview)=>void;vi.mocked(goalApi.preview).mockImplementation(()=>new Promise(resolve=>{release=resolve;}));vi.mocked(api.approve).mockImplementation(async()=>{setup(f,[f.run]);return f.run;});render(<GoalExecution {...props(f)}/>);await open(user);
    fireEvent.click(screen.getByRole('button',{name:approveName}));fireEvent.click(screen.getByRole('button',{name:approveName}));expect(goalApi.preview).toHaveBeenCalledOnce();await act(async()=>release(f.preview));await waitFor(()=>expect(api.approve).toHaveBeenCalledOnce());
  });
  it('pauses bounded automatic readonly polling without retrying mutations',async()=>{
    const f=goalExecutionFixture(),running={...f.run,state:'running'};setup(f,[running]);vi.useFakeTimers();
    try{render(<GoalExecution {...props(f)}/>);await act(async()=>{const details=document.querySelector('[data-testid="goal-execution"]>details') as HTMLDetailsElement;details.open=true;details.dispatchEvent(new Event('toggle'));});expect(refresh()).toBeEnabled();for(let i=0;i<22;i++)await act(async()=>{await vi.advanceTimersByTimeAsync(500);});expect(screen.getByText('Automatic observation paused after 20 reads. Refresh to inspect current state.')).toBeVisible();expect(api.list).toHaveBeenCalledTimes(21);expect(api.approve).not.toHaveBeenCalled();expect(api.cancel).not.toHaveBeenCalled();expect(api.advancePlan).not.toHaveBeenCalled();}finally{vi.useRealTimers();}
  });
  it('keeps the Chinese execution boundary and no-acceptance copy',async()=>{
    const user=userEvent.setup(),f=goalExecutionFixture();setup(f);render(<GoalExecution {...props(f)} language="zh"/>);await user.click(screen.getByRole('heading',{name:'Goal 执行'}));expect(await screen.findByText('单独的原生批准可在既有权限和共享限额内运行该计划。原计划范围批准本身不会启动执行。')).toBeVisible();expect(api.approve).not.toHaveBeenCalled();
  });
});
describe('explicit same-scope persisted plan adoption',()=>{
  function newer(f:Fixture){return {...structuredClone(f.plan),digest:'f'.repeat(64),proposal:{...structuredClone(f.plan.proposal),revision:2,expected_revision:1}};}
  it('adopts only the newer persisted validated plan using the freshly read run/ledger, no second Owner dialog',async()=>{
    const user=userEvent.setup(),f=goalExecutionFixture(),old={...f.run,revision:4,state:'awaiting_acceptance'},next=newer(f);setup(f,[old]);
    vi.mocked(api.advancePlan).mockImplementation(async()=>{const run={...old,revision:5,plan_revision:2,plan_digest:next.digest,state:'queued'};vi.mocked(api.list).mockResolvedValue([run]);vi.mocked(api.get).mockResolvedValue({...executionView(f,run),nodes:[executionView(f,old).nodes[0],{...f.view.nodes[0],plan_revision:2}]});return run;});
    render(<GoalExecution {...props(f)} latest={next}/>);await open(user);await user.click(screen.getByRole('button',{name:advanceName}));await waitFor(()=>expect(api.advancePlan).toHaveBeenCalledOnce());expect(api.advancePlan).toHaveBeenCalledWith({run_id:old.id,expected_plan_revision:2,expected_ledger_revision:14},4,expect.any(String));expect(api.approve).not.toHaveBeenCalled();expect(await screen.findByText('work · candidate · Plan revision 1',{selector:'summary'})).toBeVisible();expect(screen.getByText('work · not_started · Plan revision 2',{selector:'summary'})).toBeVisible();
  });
  it('recovers an uncertain adoption with exactly the same plan/ledger/key/run revision and retains old history',async()=>{
    const user=userEvent.setup(),f=goalExecutionFixture(),old={...f.run,revision:4,state:'awaiting_acceptance'},next=newer(f);setup(f,[old]);vi.mocked(api.advancePlan).mockRejectedValue(new Error('advance response lost'));render(<GoalExecution {...props(f)} latest={next}/>);await open(user);await user.click(screen.getByRole('button',{name:advanceName}));expect(await screen.findByRole('alert')).toHaveTextContent('Request outcome unknown');const sent=vi.mocked(api.advancePlan).mock.calls[0];
    const run={...old,revision:5,plan_revision:2,plan_digest:next.digest,state:'queued'};vi.mocked(api.list).mockResolvedValue([run]);vi.mocked(api.get).mockResolvedValue({...executionView(f,run),nodes:[executionView(f,old).nodes[0],{...f.view.nodes[0],plan_revision:2}]});await user.click(refresh());expect(screen.getByRole('button',{name:recoverName})).toBeEnabled();expect(api.advancePlan).toHaveBeenCalledOnce();vi.mocked(api.advancePlan).mockResolvedValue(run);await user.click(screen.getByRole('button',{name:recoverName}));await waitFor(()=>expect(api.advancePlan).toHaveBeenCalledTimes(2));expect(vi.mocked(api.advancePlan).mock.calls[1]).toEqual(sent);await waitFor(()=>expect(screen.queryByRole('button',{name:recoverName})).toBeNull());expect(api.approve).not.toHaveBeenCalled();
  });
  it.each(['active','unknown','cancelled','foreign-latest','unvalidated'])('cannot adopt when %s',async kind=>{
    const user=userEvent.setup(),f=goalExecutionFixture(),next=newer(f);let run={...f.run,revision:4,state:'awaiting_acceptance'};
    if(kind==='active')run={...run,state:'running'};if(kind==='unknown')run={...run,state:'outcome_unknown'};if(kind==='cancelled')run={...run,state:'cancelled',cancel_requested:true};if(kind==='foreign-latest')next.scope_id='foreign';if(kind==='unvalidated')next.state='model_says_approved';setup(f,[run]);render(<GoalExecution {...props(f)} latest={next}/>);await open(user);expect(screen.getByRole('button',{name:advanceName})).toBeDisabled();expect(api.advancePlan).not.toHaveBeenCalled();expect(api.approve).not.toHaveBeenCalled();
  });
});
