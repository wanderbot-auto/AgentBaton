import { request } from './api';
import { utf8Bytes } from './goal-api';
import { verifyGraphObservationText } from './goal-graph-guards';
import type { GoalRunSelection, GoalPlanAdvance } from './goal-execution-api';
import type { GraphRunSummary, GraphRunDetails, GraphOwnerDecisionResult } from './goal-graph-types';
export type GoalGraphSelection=GoalRunSelection;
export type GoalGraphAdvance=GoalPlanAdvance;
export interface GoalNodeSelection { run_id:string; node_id:string; expected_plan_revision:number; expected_ledger_revision:number }
// Draft DTO source is owned by Core; runtime guards and real fixtures must
// agree before an action is enabled. This transport carries references only.
export const goalGraphApi={
  approve:(selection:GoalGraphSelection,idempotencyKey:string)=>request<GraphRunSummary>('goal_graph_run_approve',{selection,idempotencyKey}),
  list:(taskId:string)=>request<GraphRunSummary[]>('goal_graph_runs_list',{taskId}),
  get:async(runId:string)=>{
    const view=await request<GraphRunDetails>('goal_graph_runs_get',{runId});
    if(Array.isArray(view?.validations)){
      if(view.validations.length>65536||utf8Bytes(JSON.stringify(view))>4194304)throw new Error('Graph details exceed the existing display limit');
      for(const validation of view.validations)await verifyGraphObservationText(validation);
    }
    return view;
  },
  cancel:(runId:string,expectedRevision:number,idempotencyKey:string)=>request<GraphRunSummary>('goal_graph_runs_cancel',{runId,expectedRevision,idempotencyKey}),
  advancePlan:(selection:GoalGraphAdvance,expectedRevision:number,idempotencyKey:string)=>request<GraphRunSummary>('goal_graph_run_advance_plan',{runId:selection.run_id,expectedPlanRevision:selection.expected_plan_revision,expectedLedgerRevision:selection.expected_ledger_revision,expectedRevision,idempotencyKey}),
  rejectNode:(selection:GoalNodeSelection,expectedRevision:number,idempotencyKey:string)=>request<GraphOwnerDecisionResult>('goal_node_reject',{selection,expectedRevision,idempotencyKey}),
  approveNode:(selection:GoalNodeSelection,expectedRevision:number,idempotencyKey:string)=>request<GraphOwnerDecisionResult>('goal_node_approve',{selection,expectedRevision,idempotencyKey}),
};
