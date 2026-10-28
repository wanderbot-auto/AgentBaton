import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { goalExecutionApi, goalRunShape, goalRunViewShape, goalRunFloor, goalRunMeetsFloor, type GoalRun, type GoalRunView } from './goal-execution-api';
import { goalExecutionFixture, executionView } from './goal-execution-test-fixtures';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
beforeEach(()=>{vi.resetAllMocks();});
describe('fixed Goal runtime renderer references',()=>{
  it('uses only five fixed commands and never sends dispatch/finalize/authority fields',async()=>{
    const f=goalExecutionFixture(),selection={scope_id:f.scope.id,expected_plan_revision:1,expected_ledger_revision:12};
    vi.mocked(invoke).mockResolvedValue({result:f.run,warnings:[]});
    await goalExecutionApi.approve(selection,'same-key');await goalExecutionApi.list(f.task.id);await goalExecutionApi.get(f.run.id);await goalExecutionApi.cancel(f.run.id,1,'cancel-key');await goalExecutionApi.advancePlan({run_id:f.run.id,expected_plan_revision:2,expected_ledger_revision:15},3,'advance-key');
    expect(vi.mocked(invoke).mock.calls).toEqual([['goal_run_approve',{selection,idempotencyKey:'same-key'}],['goal_runs_list',{taskId:f.task.id}],['goal_runs_get',{runId:f.run.id}],['goal_runs_cancel',{runId:f.run.id,expectedRevision:1,idempotencyKey:'cancel-key'}],['goal_run_advance_plan',{runId:f.run.id,expectedPlanRevision:2,expectedLedgerRevision:15,expectedRevision:3,idempotencyKey:'advance-key'}]]);
  });
  it.each(['task','project','scope','grant','revision','digest','accepted','cancel'])('refuses malformed or foreign run %s',kind=>{
    const f=goalExecutionFixture();let run:GoalRun={...f.run};
    if(kind==='task')run={...run,task_id:'foreign'};if(kind==='project')run={...run,project_id:'foreign'};if(kind==='scope')run={...run,scope_id:'foreign'};if(kind==='grant')run={...run,root_grant_id:'foreign'};if(kind==='revision')run={...run,revision:Number.MAX_SAFE_INTEGER+1};if(kind==='digest')run={...run,plan_digest:'model says done'};if(kind==='accepted')run={...run,task_accepted:true} as unknown as GoalRun;if(kind==='cancel')run={...run,state:'cancelled'};
    expect(goalRunShape(run,f.task,f.scope)).toBe(false);
  });
  it('preserves null observations separately from reservation and refuses non-finite values',()=>{
    const f=goalExecutionFixture();expect(goalRunViewShape(f.view,f.task,f.scope)).toBe(true);expect(f.view.nodes[0].actual_runtime_ms).toBeNull();expect(f.view.nodes[0].node.reservation.runtime_ms).toBe(1000);
    expect(goalRunViewShape({...f.view,nodes:[{...f.view.nodes[0],actual_runtime_ms:Number.NaN}]},f.task,f.scope)).toBe(false);
  });
  it.each(['empty','foreign-workspace','active-stopped','cancel-unjoined','failed-unjoined','accepted','wrong-kind','missing-meter','duplicate'])('refuses contradictory runtime view %s',kind=>{
    const f=goalExecutionFixture();let view:GoalRunView=structuredClone(f.view);
    if(kind==='empty')view={...view,nodes:[]};if(kind==='foreign-workspace')view.nodes[0].node.workspace_id='foreign';if(kind==='active-stopped')view={...view,nodes:[{...view.nodes[0],state:'running',stop_confirmed:true}]};
    if(kind==='cancel-unjoined'||kind==='failed-unjoined'){const run={...f.run,state:kind==='cancel-unjoined'?'cancelled':'failed',cancel_requested:true};view=executionView(f,run);view={...view,nodes:[{...view.nodes[0],stop_confirmed:false}]};}
    if(kind==='accepted')view={...view,task_accepted:true} as unknown as GoalRunView;if(kind==='wrong-kind')view={...view,usage_kind:'actual'} as unknown as GoalRunView;if(kind==='missing-meter')view={...view,nodes:[{...view.nodes[0],actual_output_bytes:undefined}]} as unknown as GoalRunView;if(kind==='duplicate')view={...view,nodes:[view.nodes[0],view.nodes[0]]};
    expect(goalRunViewShape(view,f.task,f.scope)).toBe(false);
  });
  it('refuses same-revision state/cancellation changes and changed creation identity',()=>{
    const f=goalExecutionFixture(),floor=goalRunFloor(f.run);expect(goalRunMeetsFloor({...f.run,state:'failed'},floor)).toBe(false);expect(goalRunMeetsFloor({...f.run,cancel_requested:true},floor)).toBe(false);expect(goalRunMeetsFloor({...f.run,revision:2,created_at:'another-creation'},floor)).toBe(false);
  });
  it('retains more than256 historical nodes across revisions without truncating valid bounded history',()=>{
    const f=goalExecutionFixture(),run={...f.run,revision:3,plan_revision:2,plan_digest:'f'.repeat(64)};
    const nodes=Array.from({length:300},(_,index)=>({...structuredClone(f.view.nodes[0]),plan_revision:index<150?1:2,node:{...structuredClone(f.view.nodes[0].node),id:'node-'+(index%150)}}));
    expect(goalRunViewShape({...f.view,run,nodes},f.task,f.scope)).toBe(true);expect(nodes).toHaveLength(300);
  });
  it('keeps full cross-revision node histories distinct and enforces digest/version floors',()=>{
    const f=goalExecutionFixture(),older=goalRunFloor(f.run),later={...f.run,revision:3,plan_revision:2,plan_digest:'f'.repeat(64)};
    const view={...f.view,run:later,nodes:[f.view.nodes[0],{...f.view.nodes[0],plan_revision:2}]};
    expect(goalRunViewShape(view,f.task,f.scope)).toBe(true);expect(goalRunMeetsFloor(later,older)).toBe(true);expect(goalRunMeetsFloor({...f.run,revision:2,plan_digest:'f'.repeat(64)},older)).toBe(false);expect(goalRunMeetsFloor(f.run,goalRunFloor(later))).toBe(false);
  });
});
