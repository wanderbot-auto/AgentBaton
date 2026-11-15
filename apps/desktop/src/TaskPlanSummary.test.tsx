import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { TaskPlanSummary } from './TaskPlanSummary';
import { goalApi } from './goal-api';
import { goalGraphApi } from './goal-graph-api';
import { goalGraphFixture } from './goal-graph-test-fixtures';
import { goalExecutionApi } from './goal-execution-api';
import { goalExecutionFixture, executionView } from './goal-execution-test-fixtures';
vi.mock('./goal-api', async original => ({ ...await original<typeof import('./goal-api')>(), goalApi: { scopesList: vi.fn(), plansList: vi.fn() } }));
vi.mock('./goal-execution-api', async original => ({ ...await original<typeof import('./goal-execution-api')>(), goalExecutionApi: { list: vi.fn(), get: vi.fn() } }));
vi.mock('./goal-graph-api', () => ({ goalGraphApi: { list: vi.fn(), get: vi.fn() } }));
let f = goalExecutionFixture();
beforeEach(() => {
  vi.clearAllMocks(); vi.mocked(goalGraphApi.list).mockResolvedValue([]); f = goalExecutionFixture();
  vi.mocked(goalApi.scopesList).mockResolvedValue([f.scope]); vi.mocked(goalApi.plansList).mockResolvedValue([f.plan]);
  vi.mocked(goalExecutionApi.list).mockResolvedValue([f.run]); vi.mocked(goalExecutionApi.get).mockResolvedValue(f.view);
});
function open(container: HTMLElement) { const details = container.querySelector('details')!; details.open = true; fireEvent(details,new Event('toggle')); }
it('reads recorded proposal and execution only when expanded', async () => {
  const { container } = render(<TaskPlanSummary task={f.task} language="en" connected />);
  expect(goalApi.scopesList).not.toHaveBeenCalled(); open(container);
  expect(await screen.findByText('Recorded proposal · Revision 1')).toBeInTheDocument();
  expect(screen.getByText('Execution record · Revision 1: Queued')).toBeInTheDocument();
  expect(screen.getByRole('heading',{name:'Pending items'})).toBeInTheDocument();
  expect(screen.getByText('Produce report: Not started')).toBeInTheDocument();
});
it('keeps acceptance pending when execution produced a candidate', async () => {
  const run = {...f.run,state:'awaiting_acceptance'};
  vi.mocked(goalExecutionApi.list).mockResolvedValue([run]); vi.mocked(goalExecutionApi.get).mockResolvedValue(executionView(f,run));
  const { container } = render(<TaskPlanSummary task={f.task} language="en" connected />); open(container);
  expect(await screen.findByText('Awaiting acceptance')).toBeInTheDocument();
  expect(screen.getByText('Execution evidence does not establish task acceptance.')).toBeInTheDocument();
  expect(screen.queryByText('No pending execution items in this record.')).not.toBeInTheDocument();
});
it('reports failed loading without claiming there is no plan', async () => {
  vi.mocked(goalApi.plansList).mockRejectedValue(new Error('private source'));
  const { container } = render(<TaskPlanSummary task={f.task} language="en" connected />); open(container);
  expect(await screen.findByRole('alert')).toHaveTextContent('could not be verified');
  expect(screen.queryByText('No plan has been recorded.')).not.toBeInTheDocument();
  expect(screen.queryByText('private source')).not.toBeInTheDocument();
});
it('rejects foreign task data before reading its plans', async () => {
  vi.mocked(goalApi.scopesList).mockResolvedValue([{...f.scope,task_id:'foreign'}]);
  const { container } = render(<TaskPlanSummary task={f.task} language="en" connected />); open(container);
  await screen.findByRole('alert'); expect(goalApi.plansList).not.toHaveBeenCalled();
});
it('rejects a run detail older than the observed run', async () => {
  vi.mocked(goalExecutionApi.list).mockResolvedValue([{...f.run,revision:2}]);
  const { container } = render(<TaskPlanSummary task={f.task} language="en" connected />); open(container);
  await screen.findByRole('alert'); expect(screen.queryByText('Recorded proposal · Revision 1')).not.toBeInTheDocument();
});
it('does not let an old task response replace the new task', async () => {
  let resolve!: (value: typeof f.scope[]) => void;
  vi.mocked(goalApi.scopesList).mockImplementationOnce(() => new Promise(r => { resolve = r; }));
  const old = f, next = goalExecutionFixture();
  const { container, rerender } = render(<TaskPlanSummary task={old.task} language="en" connected />); open(container);
  await waitFor(() => expect(goalApi.scopesList).toHaveBeenCalledWith(old.task.id));
  vi.mocked(goalApi.scopesList).mockResolvedValue([]); vi.mocked(goalExecutionApi.list).mockResolvedValue([]);
  rerender(<TaskPlanSummary task={next.task} language="en" connected />);
  await screen.findByText('No plan has been recorded.');
  await act(async () => resolve([old.scope]));
  expect(screen.getByText('No plan has been recorded.')).toBeInTheDocument(); expect(goalApi.plansList).not.toHaveBeenCalled();
});
it('shows disconnected status and makes no requests', async () => {
  const { container } = render(<TaskPlanSummary task={f.task} language="zh" connected={false} />); open(container);
  expect(await screen.findByText('连接后可读取已记录的计划。')).toBeInTheDocument(); expect(goalApi.scopesList).not.toHaveBeenCalled();
});

it('shows current graph execution instead of a false no-execution message', async () => {
  const g = goalGraphFixture(); f = g;
  vi.mocked(goalApi.scopesList).mockResolvedValue([g.scope]); vi.mocked(goalApi.plansList).mockResolvedValue([g.plan]);
  vi.mocked(goalExecutionApi.list).mockResolvedValue([]); vi.mocked(goalGraphApi.list).mockResolvedValue([g.summary]); vi.mocked(goalGraphApi.get).mockResolvedValue(g.graphView);
  const { container } = render(<TaskPlanSummary task={g.task} language="en" connected />); open(container);
  expect(await screen.findByText('Graph execution')).toBeInTheDocument();
  expect(screen.getByText('Execution record · Revision 1: Queued')).toBeInTheDocument();
  expect(screen.queryByText('No execution has been recorded for this scope.')).not.toBeInTheDocument();
  expect(goalExecutionApi.get).not.toHaveBeenCalled();
});
it('rejects graph regression across refreshes', async () => {
  const g = goalGraphFixture(); const newer = {...g.graphView, run:{...g.graphView.run,revision:2}};
  vi.mocked(goalApi.scopesList).mockResolvedValue([g.scope]); vi.mocked(goalApi.plansList).mockResolvedValue([g.plan]);
  vi.mocked(goalExecutionApi.list).mockResolvedValue([]); vi.mocked(goalGraphApi.list).mockResolvedValue([newer]); vi.mocked(goalGraphApi.get).mockResolvedValue(newer);
  const { container } = render(<TaskPlanSummary task={g.task} language="en" connected />); open(container);
  await screen.findByText('Graph execution');
  vi.mocked(goalGraphApi.list).mockResolvedValue([g.summary]); vi.mocked(goalGraphApi.get).mockResolvedValue(g.graphView);
  fireEvent.click(screen.getByRole('button',{name:'Refresh plan'}));
  await screen.findByRole('alert'); expect(screen.queryByText('Execution record · Revision 1: Queued')).not.toBeInTheDocument();
});
it('ignores equivalent task object replacements from App polling', async () => {
  const { container, rerender } = render(<TaskPlanSummary task={f.task} language="en" connected />); open(container);
  await screen.findByText('Recorded proposal · Revision 1');
  const before = vi.mocked(goalApi.plansList).mock.calls.length;
  rerender(<TaskPlanSummary task={{...f.task}} language="en" connected />);
  await act(async () => {}); expect(goalApi.plansList).toHaveBeenCalledTimes(before);
});
it('excludes previous revision nodes from pending items', async () => {
  const plan = {...f.plan, proposal:{...f.plan.proposal,revision:2,expected_revision:1}};
  const run = {...f.run,plan_revision:2,revision:2};
  vi.mocked(goalApi.plansList).mockResolvedValue([f.plan,plan]); vi.mocked(goalExecutionApi.list).mockResolvedValue([run]);
  vi.mocked(goalExecutionApi.get).mockResolvedValue({...f.view,run,nodes:[{...f.view.nodes[0],node:{...f.view.nodes[0].node,goal:'Historical pending'}},{...f.view.nodes[0],plan_revision:2}]});
  const { container } = render(<TaskPlanSummary task={f.task} language="en" connected />); open(container);
  await screen.findByText('Execution record · Revision 2: Queued');
  expect(screen.getByText('Historical pending · Revision 1: Not started')).toBeInTheDocument();
  expect(screen.queryByText('Historical pending: Not started')).not.toBeInTheDocument();
});
it('does not claim no pending items for an unknown run state', async () => {
  const run = {...f.run,state:'future_state'};
  vi.mocked(goalExecutionApi.list).mockResolvedValue([run]);
  vi.mocked(goalExecutionApi.get).mockResolvedValue({...f.view,run,nodes:[{...f.view.nodes[0],state:'candidate',session_id:'session',attempt_id:'attempt',stop_confirmed:true}]});
  const { container } = render(<TaskPlanSummary task={f.task} language="en" connected />); open(container);
  await screen.findByText('Execution record · Revision 1: Unknown execution state');
  expect(screen.getByText('Unknown execution state')).toBeInTheDocument();
  expect(screen.queryByText('No pending execution items in this record.')).not.toBeInTheDocument();
});
it('rejects legacy regression across refreshes', async () => {
  const run = {...f.run,revision:2};
  vi.mocked(goalExecutionApi.list).mockResolvedValue([run]); vi.mocked(goalExecutionApi.get).mockResolvedValue({...f.view,run});
  const { container } = render(<TaskPlanSummary task={f.task} language="en" connected />); open(container);
  await screen.findByText('Execution record · Revision 1: Queued');
  vi.mocked(goalExecutionApi.list).mockResolvedValue([f.run]); vi.mocked(goalExecutionApi.get).mockResolvedValue(f.view);
  fireEvent.click(screen.getByRole('button',{name:'Refresh plan'}));
  await screen.findByRole('alert');
  expect(screen.queryByText('Execution record · Revision 1: Queued')).not.toBeInTheDocument();
});
