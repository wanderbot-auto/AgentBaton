import {render,screen} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {expect,it} from 'vitest';
import dataRaw from './goal-graph-actual-data-fixture.json?raw';
import commandRaw from './goal-graph-actual-command-fixture.json?raw';
import {GoalGraphExecutionHistory} from './GoalGraphExecutionHistory';
import {graphDetailsShape,graphViewConsistent,graphTaskAcceptanceMatchesTask} from './goal-graph-guards';
import type {GraphRunDetails} from './goal-graph-types';
import type {GoalScope} from './goal-api';
import type {Task} from './api';
const actualCases=[
  {name:'Data',raw:dataRaw,scope:'data_contract_only',correctness:'approved_schema_ranges_metadata_only'},
] as const;
type Packet={task:Task;scope:GoalScope;graph_details:GraphRunDetails};
it.each(actualCases)('accepts the actual $name graph and Task projection within its declared verification scope',entry=>{
  const packet=JSON.parse(entry.raw) as Packet,{task,scope,graph_details:view}=packet;
  expect(graphDetailsShape(view,task,scope)).toBe(true);
  expect(graphViewConsistent(view)).toBe(true);
  expect(graphTaskAcceptanceMatchesTask(view.task_acceptance,task,scope,view)).toBe(true);
  const check=view.validations[0];
  expect(check.scope).toBe(entry.scope);
  expect(check.correctness).toBe(entry.correctness);
  expect(check.task_acceptance).toBe('not_established');
  expect(check.stop_confirmed).toBe(true);
  expect(check.io_completion).toBe('worker_joined');
  expect(view.task_acceptance!.owner_decision_ids.length).toBeGreaterThan(0);
  // The persisted Core result digest is opaque here. JavaScript re-encoding
  // is not asserted to reproduce Rust's float serialization for these types.
  expect(check.result_digest).toMatch(/^[a-f0-9]{64}$/);
});
it.each(actualCases)('preserves every original $name observation field in folded readonly history',async entry=>{
  const user=userEvent.setup(),{task,scope,graph_details:view}=JSON.parse(entry.raw) as Packet;
  render(<GoalGraphExecutionHistory view={view} task={task} scope={scope} language="en"/>);
  const toggle=screen.getByText('Result checks · 1',{selector:'summary'}),section=toggle.parentElement!;
  expect(section).not.toHaveAttribute('open');
  await user.click(toggle);
  const pre=section.querySelector('pre')!;
  const displayed=JSON.parse(pre.textContent!);
  expect(displayed).toEqual(view.validations[0]);
  expect(displayed.observation).toEqual(view.validations[0].observation);
  expect(displayed.scope).toBe(entry.scope);
  expect(displayed.correctness).toBe(entry.correctness);
  expect(displayed.task_acceptance).toBe('not_established');
  expect(displayed.observation.goal_io.total_filesystem_write_bytes).toBeNull();
  expect(displayed.observation.goal_io.total_filesystem_write_enforcement).toBe('not_enforced');
  expect(screen.getAllByText('File integrity, declared schemas and successful commands do not establish semantic acceptance.').length).toBeGreaterThan(0);
  expect(screen.queryByRole('button')).toBeNull();
  expect(screen.queryByRole('link')).toBeNull();
});
it('fails closed on the preserved actual Command packet whose original u64 inode is unsafe in a renderer',()=>{
  const {task,scope,graph_details:view}=JSON.parse(commandRaw) as Packet;
  const observation=view.validations[0].observation as Record<string,any>;
  expect(commandRaw).toContain('9223372036855074700');
  expect(Number.isSafeInteger(observation.contract.program.root_inode)).toBe(false);
  expect(graphDetailsShape(view,task,scope)).toBe(false);
  expect(graphViewConsistent(view)).toBe(false);
  expect(graphTaskAcceptanceMatchesTask(view.task_acceptance,task,scope,view)).toBe(false);
});
