import { beforeEach,expect,it,vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { api,taskAllowsExecution,taskHasAcceptanceProjection,type Task } from './api';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
const task:Task={id:'task',project_id:'project',goal:'Goal',state:'accepted',revision:7,spec_revision:1,acceptance_ref:'acceptance',acceptance_sequence:21,created_at:'2026-10-28',updated_at:'2026-10-28'};
beforeEach(()=>vi.resetAllMocks());
it('reads a real-shaped accepted Task projection with its exact receipt and sequence but forbids new execution',async()=>{
  vi.mocked(invoke).mockResolvedValue({result:{tasks:[task],sequence:21,api_version:1},warnings:[]});expect((await api.snapshot()).tasks).toEqual([task]);expect(taskHasAcceptanceProjection(task)).toBe(true);expect(taskAllowsExecution(task)).toBe(false);expect(invoke).toHaveBeenCalledExactlyOnceWith('system_snapshot',undefined);
});
it.each([undefined,null,'',true,1])('rejects accepted Task with a missing or fake acceptance reference %j',async ref=>{
  vi.mocked(invoke).mockResolvedValue({result:{tasks:[{...task,acceptance_ref:ref}],sequence:21,api_version:1},warnings:[]});await expect(api.snapshot()).rejects.toThrow('Invalid accepted Task projection');
});
it.each([undefined,null,0,-1,true,'21',Number.MAX_SAFE_INTEGER+1])('rejects missing or unsafe acceptance sequence %j',async sequence=>{
  vi.mocked(invoke).mockResolvedValue({result:{tasks:[{...task,acceptance_sequence:sequence}],sequence:21,api_version:1},warnings:[]});await expect(api.snapshot()).rejects.toThrow('Invalid accepted Task projection');
});
it('allows old draft/running projections but never unknown/future/accepted states or stale acceptance references',()=>{
  const old={...task,acceptance_ref:undefined,acceptance_sequence:undefined};for(const state of ['draft','running'])expect(taskAllowsExecution({...old,state})).toBe(true);for(const state of ['accepted','cancelled','cancel_requested','outcome_unknown','constructor','future_state'])expect(taskAllowsExecution({...old,state})).toBe(false);expect(taskAllowsExecution({...task,state:'draft'})).toBe(false);
});

it('rejects an accepted Task receipt from a future snapshot sequence',async()=>{vi.mocked(invoke).mockResolvedValue({result:{tasks:[task],sequence:20,api_version:1},warnings:[]});await expect(api.snapshot()).rejects.toThrow('Invalid accepted Task projection');});
