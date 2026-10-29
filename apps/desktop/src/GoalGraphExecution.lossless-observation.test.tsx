import {render,screen,within} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {expect,it} from 'vitest';
import dataRaw from './goal-graph-actual-lossless-data-fixture.json?raw';
import commandRaw from './goal-graph-actual-lossless-command-fixture.json?raw';
import {GoalGraphExecutionHistory} from './GoalGraphExecutionHistory';
import type {GraphRunDetails} from './goal-graph-types';
import type {GoalScope} from './goal-api';
import type {Task} from './api';
type Packet={graph_details:GraphRunDetails;scope:GoalScope;task:Task};
it.each([{name:'Data',raw:dataRaw},{name:'system Command',raw:commandRaw}])('displays the exact original $name observation text without serializing rounded values',async({raw})=>{
  const user=userEvent.setup(),{graph_details:view,scope,task}=JSON.parse(raw) as Packet;
  // This component test checks readonly original text, not API integrity or
  // Task acceptance. Those require the separate asynchronous verifier tests.
  render(<GoalGraphExecutionHistory view={view} scope={scope} task={task} language="en"/>);
  await user.click(screen.getByText('Result checks · 1',{selector:'summary'}));
  const pre=screen.getByTestId('goal-graph-original-observation');
  await user.click(within(pre.parentElement!).getByText('Details',{selector:'summary'}));
  expect(pre).toBeVisible();
  expect(pre.textContent).toBe(view.validations[0].observation_json);
  const envelope=JSON.parse(pre.parentElement!.querySelector('pre')!.textContent!);
  expect(envelope).not.toHaveProperty('observation');
  expect(envelope.result_digest).toBe(view.validations[0].result_digest);
  expect(envelope.observation_json).toBe(view.validations[0].observation_json);
  expect(screen.queryByRole('button')).toBeNull();
  expect(screen.queryByRole('link')).toBeNull();
});
it('renders raw metadata as text without interpreting HTML or adding an action',()=>{
  const {graph_details:view}=JSON.parse(dataRaw) as Packet;
  view.validations[0].observation_json='<img src=x onerror=alert(1)>';
  const {container}=render(<GoalGraphExecutionHistory view={view} language="en"/>);
  expect(screen.getByTestId('goal-graph-original-observation').textContent).toBe('<img src=x onerror=alert(1)>');
  expect(container.querySelector('img')).toBeNull();
  expect(container.querySelector('script')).toBeNull();
  expect(screen.queryByRole('button')).toBeNull();
});
