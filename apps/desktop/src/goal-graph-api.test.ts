import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { goalGraphApi as api } from './goal-graph-api';
import { DaemonError } from './api';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
beforeEach(()=>vi.mocked(invoke).mockReset());
describe('Goal graph fixed Native transport, mocked Tauri',()=>{
  it('sends independent graph approval references without renderer authority fields',async()=>{
    vi.mocked(invoke).mockResolvedValue({result:{id:'graph-run'},warnings:[]});
    const selection={scope_id:'scope',expected_plan_revision:7,expected_ledger_revision:91};
    expect(await api.approve(selection,'graph-key')).toEqual({id:'graph-run'});
    expect(invoke).toHaveBeenCalledExactlyOnceWith('goal_graph_run_approve',{selection,idempotencyKey:'graph-key'});
  });
  it('keeps node and graph proposal kinds separate and includes exact run CAS',async()=>{
    vi.mocked(invoke).mockResolvedValue({result:{id:'decision'},warnings:[]});
    const selection={run_id:'graph-run',node_id:'owner-node',expected_plan_revision:7,expected_ledger_revision:91};
    await api.approveNode(selection,13,'owner-key');
    expect(invoke).toHaveBeenCalledExactlyOnceWith('goal_node_approve',{selection,expectedRevision:13,idempotencyKey:'owner-key'});
  });
  it('uses a separate fixed Owner reject command and never a renderer decision flag',async()=>{
    vi.mocked(invoke).mockResolvedValue({result:{decision:'rejected'},warnings:[]});
    const selection={run_id:'graph-run',node_id:'owner-node',expected_plan_revision:7,expected_ledger_revision:91};
    await api.rejectNode(selection,13,'reject-key');
    expect(invoke).toHaveBeenCalledExactlyOnceWith('goal_node_reject',{selection,expectedRevision:13,idempotencyKey:'reject-key'});
  });
  it('forwards only fixed readonly IDs and fixed cancellation or plan adoption CAS',async()=>{
    vi.mocked(invoke).mockResolvedValue({result:[],warnings:[]});
    await api.list('task');await api.get('run');await api.cancel('run',3,'cancel-key');await api.advancePlan({run_id:'run',expected_plan_revision:4,expected_ledger_revision:91},3,'advance-key');
    expect(vi.mocked(invoke).mock.calls).toEqual([
      ['goal_graph_runs_list',{taskId:'task'}],['goal_graph_runs_get',{runId:'run'}],
      ['goal_graph_runs_cancel',{runId:'run',expectedRevision:3,idempotencyKey:'cancel-key'}],
      ['goal_graph_run_advance_plan',{runId:'run',expectedPlanRevision:4,expectedLedgerRevision:91,expectedRevision:3,idempotencyKey:'advance-key'}],
    ]);
  });
  it('preserves a Core rejection and post-finalization uncertainty without transport replay',async()=>{
    vi.mocked(invoke).mockResolvedValue({error:{code:'OUTCOME_UNKNOWN',message:'reconcile',retryable:false,side_effect_state:'unknown'},warnings:[]});
    const failure=await api.approveNode({run_id:'run',node_id:'owner',expected_plan_revision:1,expected_ledger_revision:10},2,'exact-key').catch(value=>value);
    expect(failure).toBeInstanceOf(DaemonError);if(!(failure instanceof DaemonError))throw new Error('missing structured daemon error');expect(failure.code).toBe('OUTCOME_UNKNOWN');expect(failure.sideEffectState).toBe('unknown');expect(invoke).toHaveBeenCalledOnce();
  });
});
