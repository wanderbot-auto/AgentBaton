// @vitest-environment node
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import dataRaw from './goal-graph-actual-lossless-data-fixture.json?raw';
import commandRaw from './goal-graph-actual-lossless-command-fixture.json?raw';
import oldRaw from './goal-graph-actual-integrity-task-fixture.json?raw';
import {goalGraphApi} from './goal-graph-api';
import {graphDetailsShape,graphDetailsMeetFloor,graphViewConsistent,graphTaskAcceptanceMatchesTask,verifyGraphObservationText} from './goal-graph-guards';
import type {GraphRunDetails} from './goal-graph-types';
import type {GoalScope} from './goal-api';
import type {Task} from './api';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
beforeEach(()=>vi.mocked(invoke).mockReset());
afterEach(()=>vi.unstubAllGlobals());
type Packet={graph_details:GraphRunDetails;scope:GoalScope;task:Task};
const cases=[{name:'Data',raw:dataRaw},{name:'system Command',raw:commandRaw}];
it.each(cases)('verifies original UTF-8 observation bytes before trusting the actual $name projection',async({raw})=>{
  const {graph_details:view,scope,task}=JSON.parse(raw) as Packet;
  expect(graphDetailsShape(view,task,scope)).toBe(false);
  vi.mocked(invoke).mockResolvedValue({result:view,warnings:[]});
  const returned=await goalGraphApi.get(view.run.id);
  expect(returned).toBe(view);
  expect(graphDetailsShape(returned,task,scope)).toBe(true);
  expect(graphViewConsistent(returned)).toBe(true);
  expect(graphTaskAcceptanceMatchesTask(returned.task_acceptance,task,scope,returned)).toBe(true);
  expect(invoke).toHaveBeenCalledExactlyOnceWith('goal_graph_runs_get',{runId:view.run.id});
});
it.each(['digest','raw-bytes','projection','not-json','crypto-unavailable'])('fails closed for %s without issuing another RPC',async mode=>{
  const {graph_details:view}=JSON.parse(commandRaw) as Packet;
  const receipt=view.validations[0];
  if(mode==='digest')receipt.result_digest='0'.repeat(64);
  if(mode==='raw-bytes')receipt.observation_json+=' ';
  if(mode==='projection')(receipt.observation as Record<string,any>).goal_io.artifact_bytes++;
  if(mode==='not-json')receipt.observation_json='{';
  if(mode==='crypto-unavailable')vi.stubGlobal('crypto',undefined);
  vi.mocked(invoke).mockResolvedValue({result:view,warnings:[]});
  await expect(goalGraphApi.get(view.run.id)).rejects.toThrow('Original validator observation could not be verified');
  expect(graphViewConsistent(view)).toBe(false);
  expect(invoke).toHaveBeenCalledOnce();
});
it('invalidates verified opaque text after its projection is changed and never relaxes candidate FilePin checks',async()=>{
  const {graph_details:view,task,scope}=JSON.parse(commandRaw) as Packet;
  const receipt=view.validations[0];
  await verifyGraphObservationText(receipt);
  expect(graphDetailsShape(view,task,scope)).toBe(true);
  const pin=view.candidates[0].artifacts[0].file_pin;
  const original=pin.inode;
  pin.inode=Number.MAX_SAFE_INTEGER+1;
  expect(graphDetailsShape(view,task,scope)).toBe(false);
  pin.inode=original;
  (receipt.observation as Record<string,any>).goal_io.artifact_bytes++;
  expect(graphDetailsShape(view,task,scope)).toBe(false);
});
it('keeps persisted receipt floors stable when only the authenticated display projection is added',async()=>{
  // Controlled projection upgrade over the preserved actual integer-valued
  // Integrity packet; not a claim about Rust f64 re-encoding.
  const {graph_details:before,task,scope}=JSON.parse(oldRaw) as Packet;
  const next=structuredClone(before);
  const receipt=next.validations[0];
  receipt.observation_json=JSON.stringify(receipt.observation);
  await verifyGraphObservationText(receipt);
  expect(graphDetailsShape(next,task,scope)).toBe(true);
  expect(graphDetailsMeetFloor(next,{summary:before,details:before})).toBe(true);
  receipt.result_digest='0'.repeat(64);
  expect(graphDetailsShape(next,task,scope)).toBe(false);
  expect(graphDetailsMeetFloor(next,{summary:before,details:before})).toBe(false);
});
it('leaves fixed summary reads and narrowing cancellation independent of raw verification availability',async()=>{
  const {graph_details:view}=JSON.parse(commandRaw) as Packet;
  vi.stubGlobal('crypto',undefined);
  vi.mocked(invoke)
    .mockResolvedValueOnce({result:[view],warnings:[]})
    .mockResolvedValueOnce({result:view,warnings:[]})
    .mockResolvedValueOnce({result:{...view,run:{...view.run,cancel_requested:true}},warnings:[]});
  expect(await goalGraphApi.list(view.run.task_id)).toEqual([view]);
  await expect(goalGraphApi.get(view.run.id)).rejects.toThrow('could not be verified');
  await goalGraphApi.cancel(view.run.id,view.run.revision,'same-cancel-key');
  expect(vi.mocked(invoke).mock.calls).toEqual([
    ['goal_graph_runs_list',{taskId:view.run.task_id}],
    ['goal_graph_runs_get',{runId:view.run.id}],
    ['goal_graph_runs_cancel',{runId:view.run.id,expectedRevision:view.run.revision,idempotencyKey:'same-cancel-key'}],
  ]);
});
it('rejects observation projection changes during asynchronous digest verification',async()=>{
  const {graph_details:view,task,scope}=JSON.parse(commandRaw) as Packet;
  const receipt=view.validations[0],original=globalThis.crypto;
  vi.stubGlobal('crypto',{subtle:{digest:async(algorithm:AlgorithmIdentifier,data:BufferSource)=>{
    const bytes=await original.subtle.digest(algorithm,data);
    (receipt.observation as Record<string,any>).goal_io.artifact_bytes++;
    return bytes;
  }}});
  await expect(verifyGraphObservationText(receipt)).rejects.toThrow('could not be verified');
  expect(graphDetailsShape(view,task,scope)).toBe(false);
});
