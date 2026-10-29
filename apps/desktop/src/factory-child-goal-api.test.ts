// Mocked Native transport only; no real daemon or approval.
import { beforeEach,describe,it,expect,vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { factoryChildGoalApi } from './factory-api';
import { childGoalFixture } from './factory-child-goal-test-fixtures';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
beforeEach(()=>{vi.mocked(invoke).mockReset();vi.mocked(invoke).mockResolvedValue({result:{},warnings:[]});});
describe('schema15 five fixed Owner bridges, mocked Tauri',()=>{
 it('preserves complete saved reference and snake DTO inside exact camelCase args; explicit get null is retained',async()=>{
  const f=await childGoalFixture(),request={task_id:f.task.id,plan:f.plan.reference,expected_factory_run_revision:7};
  await factoryChildGoalApi.start(f.scope.id,request,2,'start-key');await factoryChildGoalApi.get(f.scope.id,f.task.id,null);await factoryChildGoalApi.list(f.scope.id,f.task.id);await factoryChildGoalApi.cancel(f.scope.id,f.task.id,f.runId,4,2,'cancel-key');await factoryChildGoalApi.advancePlan(f.scope.id,f.runId,request,5,2,'advance-key');
  expect(vi.mocked(invoke).mock.calls).toEqual([['factory_child_goals_start',{scopeId:f.scope.id,request,expectedRevision:2,idempotencyKey:'start-key'}],['factory_child_goals_get',{scopeId:f.scope.id,taskId:f.task.id,goalRunId:null}],['factory_child_goals_list',{scopeId:f.scope.id,taskId:f.task.id}],['factory_child_goals_cancel',{scopeId:f.scope.id,taskId:f.task.id,goalRunId:f.runId,expectedGoalRunRevision:4,expectedRevision:2,idempotencyKey:'cancel-key'}],['factory_child_goals_advance_plan',{scopeId:f.scope.id,goalRunId:f.runId,request,expectedGoalRunRevision:5,expectedRevision:2,idempotencyKey:'advance-key'}]]);
  expect(Object.keys(factoryChildGoalApi)).toHaveLength(5);expect(Object.keys(request)).toEqual(['task_id','plan','expected_factory_run_revision']);
 });
 it.each(['PERMISSION_DENIED','STALE_REVISION','INPUT_CHANGED','OUTCOME_UNKNOWN'])('preserves %s without fallback dispatch or invented success',async code=>{
  vi.mocked(invoke).mockResolvedValue({error:{code,message:'explicit original failure',retryable:false,side_effect_state:code==='OUTCOME_UNKNOWN'?'unknown':'not_started'},warnings:[]});await expect(factoryChildGoalApi.cancel('scope','task','run',3,1,'original-key')).rejects.toMatchObject({code});expect(invoke).toHaveBeenCalledTimes(1);
 });
 it('rejects unverified original observation text before any trusted Graph projection',async()=>{
  const f=await childGoalFixture(),v=f.makeView();v.details.validations=[{observation:{goal_io:{state:'worker_joined'}},observation_json:'{"goal_io":{"state":"worker_joined"}}',result_digest:'0'.repeat(64)} as typeof v.details.validations[number]];vi.mocked(invoke).mockResolvedValue({result:v,warnings:[]});await expect(factoryChildGoalApi.get(f.scope.id,f.task.id,f.runId)).rejects.toThrow('Original validator observation');expect(invoke).toHaveBeenCalledTimes(1);
 });
});
