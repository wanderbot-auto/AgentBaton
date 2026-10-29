// Mocked Tauri/daemon controls only. No Core15, actual Native dialog or Provider runs.
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
import { factoryScopeShape,factoryDetailsShape } from './factory-guards';
import { childContextShape,childViewShape } from './factory-child-plan';
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
 vi.mocked(api.snapshot).mockImplementation(async()=>({tasks:[structuredClone(state.task)],sequence:1000,api_version:1}));
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
it('starts the exact selected saved reference with both real CAS values and no origin/self-attested authority',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();records(f);render(<FactoryChildGoals {...props(f)}/>);await open(user);await waitFor(()=>expect(screen.getByRole('button',{name:t.start})).toBeEnabled());await user.click(screen.getByRole('button',{name:t.start}));await waitFor(()=>expect(factoryChildGoalApi.start).toHaveBeenCalledOnce());expect(factoryChildGoalApi.start).toHaveBeenCalledWith(f.scope.id,{task_id:f.task.id,plan:f.plan.reference,expected_factory_run_revision:1},1,expect.any(String));await waitFor(()=>expect(screen.getByTestId('goal-graph-history')).toBeInTheDocument());expect(factoryApi).not.toHaveProperty('execute');expect(goalGraphApi.approveNode).not.toHaveBeenCalled();expect(screen.getByRole('button',{name:t.start})).toBeDisabled();
});
it('shows mandatory server origin, original Native approval, complete binding and exact initial/adopted raw text',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();records(f,f.makeView());render(<FactoryChildGoals {...props(f)}/>);await open(user);await screen.findByTestId('goal-graph-history');await user.click(screen.getByText(t.origin,{selector:'summary'}));expect(screen.getByText(f.scope.approval_id,{selector:'dd'})).toBeVisible();expect(screen.getByText(`${f.grant.id} · r1`,{selector:'dd'})).toBeVisible();await user.click(screen.getByText(t.initial,{selector:'summary'}));expect(screen.getByLabelText(t.initial)).toHaveTextContent(f.plan.proposal.plan_id);expect(factoryChildGoalApi.start).not.toHaveBeenCalled();
});
it('does not silently execute an older selected ref when the saved head changed',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();await f.nextPlan();records(f);render(<FactoryChildGoals {...props(f,f.plan)}/>);await open(user);await waitFor(()=>expect(factoryChildPlanApi.get).toHaveBeenCalled());expect(screen.getByRole('button',{name:t.start})).toBeDisabled();expect(factoryChildGoalApi.start).not.toHaveBeenCalled();
});
it('preserves denial and stale CAS without retrying automatically',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();records(f);vi.mocked(factoryChildGoalApi.start).mockRejectedValue(new DaemonError('STALE_REVISION','original Factory run CAS changed','not_started'));render(<FactoryChildGoals {...props(f)}/>);await open(user);await waitFor(()=>expect(screen.getByRole('button',{name:t.start})).toBeEnabled());await user.click(screen.getByRole('button',{name:t.start}));expect(await screen.findByRole('alert')).toHaveTextContent('STALE_REVISION');expect(factoryChildGoalApi.start).toHaveBeenCalledTimes(1);expect(screen.getByRole('button',{name:t.start})).toBeDisabled();
});
it('keeps the exact request across lost reply and remount; explicit recovery returns a historical receipt without rollback',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture(),state=records(f);const first=render(<FactoryChildGoals {...props(f)}/>);vi.mocked(factoryChildGoalApi.start).mockImplementationOnce(async()=>{state.view=f.makeView();f.detail.run.revision=2;throw new Error('transport response lost after persist');});await open(user);await waitFor(()=>expect(screen.getByRole('button',{name:t.start})).toBeEnabled());await user.click(screen.getByRole('button',{name:t.start}));await screen.findByRole('alert');const exact=vi.mocked(factoryChildGoalApi.start).mock.calls[0];expect(factoryChildGoalApi.start).toHaveBeenCalledTimes(1);first.unmount();
 state.view=f.makeView(f.plan,3,'blocked');vi.mocked(factoryChildGoalApi.start).mockResolvedValue(f.makeView().summary);render(<FactoryChildGoals {...props(f)}/>);await open(user);await waitFor(()=>expect(screen.getByRole('button',{name:t.recover})).toBeEnabled());expect(factoryChildGoalApi.start).toHaveBeenCalledTimes(1);await user.click(screen.getByRole('button',{name:t.recover}));await waitFor(()=>expect(screen.queryByRole('button',{name:t.recover})).toBeNull());expect(vi.mocked(factoryChildGoalApi.start).mock.calls[1]).toEqual(exact);expect(screen.getByText(/r3$/,{selector:'span'})).toBeInTheDocument();expect(factoryChildGoalApi.cancel).not.toHaveBeenCalled();
});
it('uses the fixed adoption route, displays immutable initial separately and retains old Graph nodes',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture(),before=f.makeView(),p2=await f.nextPlan(),state=records(f,before);vi.mocked(factoryChildGoalApi.advancePlan).mockImplementation(async()=>{const v=f.makeView(p2,2);v.details.nodes=[...structuredClone(before.details.nodes),...v.details.nodes];state.view=v;return structuredClone(v.summary);});render(<FactoryChildGoals {...props(f,p2)}/>);await open(user);await waitFor(()=>expect(screen.getByRole('button',{name:t.advance})).toBeEnabled());await user.click(screen.getByRole('button',{name:t.advance}));await waitFor(()=>expect(factoryChildGoalApi.advancePlan).toHaveBeenCalledOnce());expect(factoryChildGoalApi.advancePlan).toHaveBeenCalledWith(f.scope.id,f.runId,{task_id:f.task.id,plan:p2.reference,expected_factory_run_revision:1},1,1,expect.any(String));await waitFor(()=>expect(screen.getAllByTestId('goal-graph-node')).toHaveLength(4));expect(factoryChildGoalApi.start).not.toHaveBeenCalled();
});
it('permits only narrowing cancellation from trusted summary when unsafe details block Owner actions',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture(true),bad=f.ownerReady();bad.details.candidates[0].artifacts[0].file_pin.inode=Number.MAX_SAFE_INTEGER+1;records(f,bad);render(<FactoryChildGoals {...props(f)}/>);await open(user);await screen.findByRole('alert');expect(screen.queryByRole('button',{name:t.owner})).toBeNull();await waitFor(()=>expect(screen.getByRole('button',{name:t.cancel})).toBeEnabled());await user.click(screen.getByRole('button',{name:t.cancel}));await waitFor(()=>expect(factoryChildGoalApi.cancel).toHaveBeenCalledWith(f.scope.id,f.task.id,f.runId,2,1,expect.any(String)));expect(goalGraphApi.approveNode).not.toHaveBeenCalled();expect(factoryChildGoalApi.start).not.toHaveBeenCalled();expect(screen.queryByText('Task accepted')).toBeNull();
});
it('keeps cancellation requested distinct from confirmed stop and never reopens a run budget',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture(),state=records(f,f.makeView());render(<FactoryChildGoals {...props(f)}/>);await open(user);await screen.findByTestId('goal-graph-history');await user.click(screen.getByRole('button',{name:t.cancel}));await waitFor(()=>expect(factoryChildGoalApi.cancel).toHaveBeenCalledOnce());await waitFor(()=>expect(screen.getByText(/Cancellation requested; stop not confirmed/,{selector:'span'})).toBeInTheDocument());expect(state.view!.details.nodes[0].stop_confirmed).toBe(false);expect(screen.getByRole('button',{name:t.start})).toBeDisabled();expect(factoryChildGoalApi.start).not.toHaveBeenCalled();
});
it('blocks fresh dispatch after revocation while preserving authenticated history and actual scope CAS for cancel',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture(),v=f.makeView();f.scope.revoked=true;f.scope.revision=2;records(f,v);render(<FactoryChildGoals {...props(f)}/>);await open(user);await screen.findByTestId('goal-graph-history');expect(screen.getByRole('button',{name:t.start})).toBeDisabled();await user.click(screen.getByRole('button',{name:t.cancel}));await waitFor(()=>expect(factoryChildGoalApi.cancel).toHaveBeenCalledWith(f.scope.id,f.task.id,f.runId,1,2,expect.any(String)));expect(factoryChildGoalApi.start).not.toHaveBeenCalled();
});
it('reuses exact fixed Native Owner node approval with references, not a synthetic Factory or legacy approval',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture(true);records(f,f.ownerReady());vi.mocked(goalGraphApi.approveNode).mockRejectedValue(new DaemonError('PERMISSION_DENIED','Native Cancel','not_started'));render(<FactoryChildGoals {...props(f)}/>);await open(user);await waitFor(()=>expect(screen.getByRole('button',{name:t.owner})).toBeEnabled());await user.click(screen.getByRole('button',{name:t.owner}));await waitFor(()=>expect(goalGraphApi.approveNode).toHaveBeenCalledWith({run_id:f.runId,node_id:'owner',expected_plan_revision:1,expected_ledger_revision:22},2,expect.any(String)));expect(await screen.findByRole('alert')).toHaveTextContent('Native Cancel');expect(factoryChildGoalApi.start).not.toHaveBeenCalled();expect(goalGraphApi.rejectNode).not.toHaveBeenCalled();expect(screen.queryByText('Task accepted')).toBeNull();
});
it.each(['missing','foreign','initial-hash','node'])('blocks Owner when %s evidence cannot match complete server origin/saved plan',async mode=>{
 const user=userEvent.setup(),f=await childGoalFixture(true),v=f.ownerReady();if(mode==='missing')delete (v as Partial<typeof v>).origin;if(mode==='foreign')v.origin.original_native_approval_id='foreign';if(mode==='initial-hash')v.origin.initial_plan.raw_sha256='f'.repeat(64);if(mode==='node')v.details.nodes[1].node.goal='different criterion';records(f,v);render(<FactoryChildGoals {...props(f)}/>);await open(user);await screen.findByRole('alert');expect(screen.queryByRole('button',{name:t.owner})).toBeNull();expect(goalGraphApi.approveNode).not.toHaveBeenCalled();expect(factoryChildGoalApi.start).not.toHaveBeenCalled();
});
it('renders untrusted plan text as text rather than executable HTML',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();const malicious='<img src=x onerror="window.unapproved=true"><script>bad()</script>';f.proposal.nodes[0].goal=malicious;const p=await f.view();f.plans.set(1,p);records(f);render(<FactoryChildGoals {...props(f,p)}/>);await open(user);await user.click(screen.getByText(t.selected,{selector:'summary'}));expect(screen.getByLabelText(t.raw)).toHaveTextContent('<img');expect(document.querySelector('img')).toBeNull();expect(document.querySelector('script')).toBeNull();
});
it('requires the durable Task projection as well as the exact Owner receipt before claiming Task accepted',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture(true),before=f.ownerReady(),state=records(f,before);
 const after=structuredClone(before),decisionId='decision-'+f.task.id,approvalId='native-node-approval-'+f.task.id,acceptanceId='acceptance-'+f.task.id;
 after.details.run={...after.details.run,revision:3,state:'accepted',task_accepted:true};after.details.task_accepted=true;after.details.ledger_revision=30;
 after.details.nodes[0].acceptance_state='accepted';after.details.nodes[1]={...after.details.nodes[1],state:'accepted',acceptance_state:'accepted',owner_decision_ids:[decisionId]};
 after.details.owner_decisions=[{id:decisionId,graph_contract_version:1,approval_id:approvalId,run_id:f.runId,plan_revision:1,plan_digest:f.plan.reference.plan_digest,node_id:'owner',rule_id:'factory-child-owner-review',criterion_digest:'2'.repeat(64),candidate_refs:after.details.candidates[0].artifacts,validation_ids:[],decision:'accepted',created_at:f.plan.created_at,source:'local_owner_confirmation'}];
 after.details.task_acceptance={id:acceptanceId,graph_contract_version:1,task_id:f.task.id,task_revision:7,spec_revision:1,project_id:f.task.project_id!,run_id:f.runId,scope_id:f.derivedScope,root_grant_id:f.grant.id,plan_revision:1,plan_digest:f.plan.reference.plan_digest,input_revision:f.context.goal.input_revision,workspace_id:f.workspace.id,capability_digest:'b'.repeat(64),criterion_digest:'2'.repeat(64),artifact_refs:after.details.candidates[0].artifacts,validation_ids:[],owner_decision_ids:[decisionId],accepted_at:f.plan.created_at,sequence:30,source:'baton_goal_acceptance'};f.sync(after);
 vi.mocked(goalGraphApi.approveNode).mockImplementation(async()=>{state.view=after;return {graph_contract_version:1,approval_id:approvalId,run_id:f.runId,node_id:'owner',plan_revision:1,decision:'accepted',owner_decision_id:decisionId,run_revision:3,task_accepted:true};});
 render(<FactoryChildGoals {...props(f)}/>);await open(user);await waitFor(()=>expect(screen.getByRole('button',{name:t.owner})).toBeEnabled());await user.click(screen.getByRole('button',{name:t.owner}));await waitFor(()=>expect(goalGraphApi.approveNode).toHaveBeenCalledOnce());await waitFor(()=>expect(screen.queryByRole('button',{name:t.owner})).toBeNull());expect(screen.queryByText('Task accepted')).toBeNull();
 state.task={...state.task,state:'accepted',revision:7,acceptance_ref:acceptanceId,acceptance_sequence:30};await user.click(screen.getByRole('button',{name:t.refresh}));await waitFor(()=>expect(screen.getByText('Task accepted')).toBeInTheDocument());expect(factoryChildGoalApi.start).not.toHaveBeenCalled();expect(factoryChildGoalApi.advancePlan).not.toHaveBeenCalled();
});

it('keeps authenticated old-spec history inspectable while denying fresh execution and Owner actions',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture(true),state=records(f,f.ownerReady());state.task={...state.task,revision:2,spec_revision:2,goal:'Changed current Task specification'};render(<FactoryChildGoals {...props(f)}/>);await open(user);await screen.findByTestId('goal-graph-history');expect(screen.getByRole('button',{name:t.start})).toBeDisabled();expect(screen.getByRole('button',{name:t.advance})).toBeDisabled();expect(screen.getByRole('button',{name:t.owner})).toBeDisabled();expect(screen.getByRole('button',{name:t.cancel})).toBeEnabled();expect(goalGraphApi.approveNode).not.toHaveBeenCalled();expect(factoryChildGoalApi.start).not.toHaveBeenCalled();
});

it('keeps Work-only planning readable but blocks dispatch until actual Owner/validator coverage exists',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture(false);records(f);render(<FactoryChildGoals {...props(f)}/>);await open(user);await waitFor(()=>expect(screen.getByText(t.incomplete)).toBeInTheDocument());expect(screen.getByRole('button',{name:t.start})).toBeDisabled();expect(factoryChildGoalApi.start).not.toHaveBeenCalled();
});
it('does not open failed-Graph adoption merely because reviewed rework limits are positive',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture(),policy=f.selection.child_goal_execution_policy!;f.selection.root_budget.reworks=1;f.selection.project_budget.reworks=1;policy.root_budget.reworks=1;policy.project_budget.reworks=1;records(f,f.makeView(f.plan,2,'failed'));const p2=await f.nextPlan();render(<FactoryChildGoals {...props(f,p2)}/>);await open(user);await screen.findByTestId('goal-graph-history');expect(screen.getByText(t.failedGate)).toBeInTheDocument();expect(screen.getByRole('button',{name:t.advance})).toBeDisabled();expect(factoryChildGoalApi.advancePlan).not.toHaveBeenCalled();
});

// Deferred mocked reads reproduce the parent poll/child read overlap, not a
// Core run or an OS approval. No mutation is retried by these controls.
function holdNextHead(){
 let release!:(value:FactoryChildPlanView)=>void;
 const reply=new Promise<FactoryChildPlanView>(resolve=>{release=resolve;});
 vi.mocked(factoryChildPlanApi.get).mockImplementationOnce(()=>reply);
 return release;
}
it('keeps one explicit start across a same-snapshot parent read-only refresh',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();records(f);
 const current=props(f),rendered=render(<FactoryChildGoals {...current} parentReadIdentity="same-authority"/>);
 await open(user);await waitFor(()=>expect(screen.getByRole('button',{name:t.start})).toBeEnabled());
 const before=vi.mocked(factoryChildPlanApi.get).mock.calls.length,release=holdNextHead();
 await user.click(screen.getByRole('button',{name:t.start}));await waitFor(()=>expect(factoryChildPlanApi.get).toHaveBeenCalledTimes(before+1));
 rendered.rerender(<FactoryChildGoals {...current} scope={structuredClone(f.scope)} target={structuredClone(f.target)} run={structuredClone(f.detail.run)} selected={structuredClone(f.plan)} parentReady={false} parentRefreshOnly parentReadIdentity="same-authority"/>);
 await act(async()=>release(structuredClone(f.plan)));
 await waitFor(()=>expect(factoryChildGoalApi.start).toHaveBeenCalledOnce());
 expect(factoryChildGoalApi.start).toHaveBeenCalledWith(f.scope.id,{task_id:f.task.id,plan:f.plan.reference,expected_factory_run_revision:1},1,expect.any(String));
 expect(screen.queryByText(t.stale)).toBeNull();
});
it('does not treat an unready or unknown parent as a read-only refresh',async()=>{
 const user=userEvent.setup(),f=await childGoalFixture();records(f);
 const current=props(f),rendered=render(<FactoryChildGoals {...current} parentReadIdentity="same-authority"/>);
 await open(user);await waitFor(()=>expect(screen.getByRole('button',{name:t.start})).toBeEnabled());
 const before=vi.mocked(factoryChildPlanApi.get).mock.calls.length,release=holdNextHead();await user.click(screen.getByRole('button',{name:t.start}));await waitFor(()=>expect(factoryChildPlanApi.get).toHaveBeenCalledTimes(before+1));
 rendered.rerender(<FactoryChildGoals {...current} parentReady={false} parentRefreshOnly={false} parentReadIdentity="same-authority"/>);
 await act(async()=>release(structuredClone(f.plan)));expect(await screen.findByRole('alert')).toHaveTextContent(t.stale);expect(factoryChildGoalApi.start).not.toHaveBeenCalled();
});
it.each(['root','scope','target','run','connection','grant','pending-unknown'])('blocks a changed %s authority snapshot during the deferred read',async changed=>{
 const user=userEvent.setup(),f=await childGoalFixture();records(f);
 const current=props(f),rendered=render(<FactoryChildGoals {...current} parentReadIdentity="original-authority"/>);
 await open(user);await waitFor(()=>expect(screen.getByRole('button',{name:t.start})).toBeEnabled());
 const before=vi.mocked(factoryChildPlanApi.get).mock.calls.length,release=holdNextHead();await user.click(screen.getByRole('button',{name:t.start}));await waitFor(()=>expect(factoryChildPlanApi.get).toHaveBeenCalledTimes(before+1));
 const next={...current,scope:structuredClone(f.scope),target:structuredClone(f.target),run:structuredClone(f.detail.run)};
 if(changed==='scope')next.scope.revision++;
 if(changed==='target')next.target.revision++;
 if(changed==='run')next.run.revision++;
 // Root/connection/Grant are owned by the ancestor. Its complete validated
 // read fingerprint changes even when the child reference remains identical.
 rendered.rerender(<FactoryChildGoals {...next} parentReady={false} parentRefreshOnly={changed!=='pending-unknown'} parentReadIdentity={changed==='scope'||changed==='target'||changed==='run'?'original-authority':changed+'-changed'}/>);
 await act(async()=>release(structuredClone(f.plan)));expect(await screen.findByRole('alert')).toHaveTextContent(t.stale);expect(factoryChildGoalApi.start).not.toHaveBeenCalled();
});
it.each(['expired','revoked','spec','unknown'])('preserves fresh %s rejection while the parent only refreshes',async changed=>{
 const user=userEvent.setup(),f=await childGoalFixture(),state=records(f);
 const current=props(f),rendered=render(<FactoryChildGoals {...current} parentReadIdentity="same-authority"/>);
 await open(user);await waitFor(()=>expect(screen.getByRole('button',{name:t.start})).toBeEnabled());
 const before=vi.mocked(factoryChildPlanApi.get).mock.calls.length,release=holdNextHead();await user.click(screen.getByRole('button',{name:t.start}));await waitFor(()=>expect(factoryChildPlanApi.get).toHaveBeenCalledTimes(before+1));
 if(changed==='expired')vi.spyOn(Date,'now').mockReturnValue(f.scope.expires_at_ms+1);
 // These are observed by the current root/child props as well as server
 // records; a read-only marker must never bypass an actual change.
 const next={...current,scope:structuredClone(f.scope)};
 if(changed==='revoked')next.scope.revoked=true;
 if(changed==='spec'){state.task.spec_revision=2;next.scope.bindings.root_task={...next.scope.bindings.root_task,spec_revision:2};}
 if(changed==='unknown'){state.task.state='outcome_unknown';next.scope.bindings.root_task={...next.scope.bindings.root_task,state:'outcome_unknown'};}
 rendered.rerender(<FactoryChildGoals {...next} parentReady={false} parentRefreshOnly parentReadIdentity="same-authority"/>);
 try{await act(async()=>release(structuredClone(f.plan)));await screen.findByRole('alert');expect(factoryChildGoalApi.start).not.toHaveBeenCalled();}
 finally{vi.restoreAllMocks();}
});
it.each(['draft','outcome_unknown'])('keeps the exact Factory child path with a separate generic-Unknown Root and %s child',async childState=>{
 const user=userEvent.setup(),f=await childGoalFixture();
 // A separate parent prevents the old single-task fixture from conflating the
 // generic Root projection with the Task selected for child execution. These
 // are mocked renderer records, not proof of the server's private stop chain.
 const root={...structuredClone(f.task),id:'separate-root-'+f.task.id,state:'outcome_unknown',revision:4};
 f.scope.root_task_id=f.selection.root_task_id=root.id;
 // Keep the approved revision1 snapshot separate from current Root revision4.
 f.scope.bindings.root_task={...structuredClone(f.scope.bindings.root_task),id:root.id};
 f.detail.run.root_task_id=root.id;f.context.goal.root_task_id=root.id;f.proposal.goal.root_task_id=root.id;
 // Rebuild this synthetic saved plan from its corrected proposal before any
 // read, including the original UTF8/raw SHA. Do not patch immutable raw text.
 const corrected=await f.view();corrected.root_task_id=root.id;f.plan=corrected;f.plans.set(1,corrected);
 f.target.parent_task_id=root.id;f.target.root_task_id=root.id;f.target.delegation_id='exact-child-delegation';f.target.state='queued';
 const rootHandle={...structuredClone(f.target),task_id:root.id,parent_task_id:null,delegation_id:'',state:'waiting_child',revision:6,attempt_id:'normal-root-attempt',confirmed_stop:true};
 f.detail.tasks=[rootHandle,f.target];f.detail.run.active_task_id=null;f.detail.run.active_session_id=null;f.detail.run.state='waiting_child';
 f.detail.waiting_relations=[{parent_task_id:root.id,child_task_id:f.task.id,parent_session_id:'normal-root-session',parent_stop_confirmed:true,handoff_digest:null}];
 expect(f.scope.bindings.root_task.revision).toBe(f.selection.expected_task_revision);expect(root.revision).toBe(4);
 expect(factoryScopeShape(f.scope,root)).toBe(true);expect(factoryDetailsShape(f.detail,root,f.scope.id)).toBe(true);
 expect(childContextShape(f.context,f.scope,f.target)).toBe(true);expect(childViewShape(f.plan,f.context,f.scope)).toBe(true);
 const state=records(f);state.task.state=childState;
 vi.mocked(api.snapshot).mockImplementation(async()=>({tasks:[structuredClone(root),structuredClone(state.task)],sequence:1000,api_version:1}));
 // The real daemon remains authoritative for private normal-handoff proof;
 // a fixed refusal avoids pretending this mock establishes a completed run.
 vi.mocked(factoryChildGoalApi.start).mockRejectedValue(new DaemonError('PERMISSION_DENIED','private Core proof still required','not_started'));
 render(<FactoryChildGoals {...props(f)} parentReadIdentity="exact-separate-root-and-child"/>);await open(user);
 if(childState==='outcome_unknown'){
  await waitFor(()=>expect(factoryChildPlanApi.get).toHaveBeenCalled());expect(screen.getByRole('button',{name:t.start})).toBeDisabled();expect(factoryChildGoalApi.start).not.toHaveBeenCalled();
 }else{
  await waitFor(()=>expect(screen.getByRole('button',{name:t.start})).toBeEnabled());await user.click(screen.getByRole('button',{name:t.start}));
  await waitFor(()=>expect(factoryChildGoalApi.start).toHaveBeenCalledOnce());expect(factoryChildGoalApi.start).toHaveBeenCalledWith(f.scope.id,{task_id:f.task.id,plan:f.plan.reference,expected_factory_run_revision:1},1,expect.any(String));
  expect(await screen.findByRole('alert')).toHaveTextContent('private Core proof still required');
 }
});
