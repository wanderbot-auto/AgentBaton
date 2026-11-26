import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { App } from './App';
import { api, request, DaemonError, type Task, type Project } from './api';
import { sessionApi, type ManagedSession } from './session-api';
vi.mock('./session-api', () => ({ sessionApi: { connectionsList: vi.fn(), sessionsList: vi.fn() } }));

// These tests mock only the Tauri transport. They are not native IPC evidence.
vi.mock('./api', async importOriginal => {
  const actual = await importOriginal<typeof import('./api')>();
  return { ...actual, request: vi.fn(), api: { snapshot: vi.fn(), create: vi.fn(), cancel: vi.fn(), events: vi.fn(), projectsList: vi.fn(), projectsImport: vi.fn(), projectsArchive: vi.fn(), projectsReassociate: vi.fn(), workspacesCreate: vi.fn(), workspacesList: vi.fn() } };
});
const task: Task = { id: 'task-1', goal: 'Check cancellation evidence', state: 'draft', revision: 1, created_at: '2026-10-27T00:00:00Z', updated_at: '2026-10-27T00:00:00Z' };
async function openEvent(kind: string, user: ReturnType<typeof userEvent.setup>) {
  const raw = await screen.findByText(kind);
  const disclosure = raw.closest('details')!;
  expect(disclosure).not.toHaveAttribute('open');
  expect(raw).not.toBeVisible();
  await user.click(disclosure.querySelector('summary')!);
  expect(raw).toBeVisible();
}
const session: ManagedSession = {
  id: 'session-1', task_id: task.id, project_id: 'project-1', workspace_id: 'workspace-1',
  connection_id: 'connection-1', grant_id: 'grant-1', state: 'stopped', revision: 2,
  native_session_id: null, created_at: task.created_at, updated_at: task.updated_at,
  last_error: null, turn_state: 'completed',
};
beforeEach(() => {
  vi.resetAllMocks();
  localStorage.clear();
  localStorage.setItem('agentbaton.language', 'en');
  vi.mocked(request).mockImplementation(async (command,args) => command === 'tasks_history' ? { task_id: args?.taskId, events: [], next_cursor: null } as never : undefined as never);
  vi.mocked(api.projectsList).mockResolvedValue([]);
  vi.mocked(api.workspacesList).mockResolvedValue([]);
  vi.mocked(sessionApi.connectionsList).mockResolvedValue([]);
  vi.mocked(sessionApi.sessionsList).mockResolvedValue([]);
  vi.mocked(api.snapshot).mockResolvedValue({ tasks: [], sequence: 0, api_version:1,database_schema_version:17 });
  vi.mocked(api.events).mockResolvedValue({ events: [], next_cursor: 0, sequence: 0, gap: false });
});
describe('Task workspace using mocked Tauri transport', () => {
  it.each([
    {
      language: 'en', open: 'Open task', taskState: 'Outcome unknown', send: 'Send message', sendGoal: 'Send task goal',
      labels: [
        'Recorded session state: Stopped (confirmed) · Turn ended (not accepted)',
        'Recorded session state: Cancellation requested · User message queued',
        'Recorded session state: Failed · Turn cancelled',
        'Recorded session state: Outcome unknown · Turn interrupted',
        'Recorded session state: Stopped (confirmed) · Outcome unknown',
        'Recorded session state: Starting',
        'Recorded session state: Unknown: label · Unknown: grant',
      ],
    },
    {
      language: 'zh', open: '打开任务', taskState: '结果未知', send: '发送消息', sendGoal: '发送任务目标',
      labels: [
        '已记录的会话状态: 已停止（已确认） · 本轮已结束（未验收）',
        '已记录的会话状态: 已请求取消 · 用户消息已入队',
        '已记录的会话状态: 失败 · 本轮已取消',
        '已记录的会话状态: 结果未知 · 本轮已中断',
        '已记录的会话状态: 已停止（已确认） · 结果未知',
        '已记录的会话状态: 正在启动',
        '已记录的会话状态: 未知: label · 未知: grant',
      ],
    },
  ])('shows recorded session and turn state independently in $language', async ({
    language, open, taskState, send, sendGoal, labels,
  }) => {
    localStorage.setItem('agentbaton.language', language);
    vi.mocked(api.snapshot).mockResolvedValue({
      tasks: [{ ...task, project_id: session.project_id, state: 'running' }],
      sequence: 1, api_version: 1, database_schema_version: 19,
    });
    vi.mocked(sessionApi.sessionsList).mockResolvedValue([
      session,
      { ...session, id: 'pending', state: 'cancel_requested', turn_state: 'pending' },
      { ...session, id: 'cancelled', state: 'failed', turn_state: 'cancelled' },
      { ...session, id: 'interrupted', state: 'outcome_unknown', turn_state: 'interrupted' },
      { ...session, id: 'unknown-turn', turn_state: 'outcome_unknown' },
      { ...session, id: 'starting', state: 'starting', turn_state: null },
      { ...session, id: 'unknown', state: 'label' as ManagedSession['state'], turn_state: 'grant' },
    ]);
    render(<App />);
    await userEvent.setup().click(await screen.findByRole('button', { name: `${open}: ${task.goal}` }));
    const thread = within(screen.getByTestId('task-thread'));
    const evidence = thread.getByText(language === 'zh' ? '原始可观察事件' : 'Raw observable events');
    expect(evidence.closest('details')).not.toHaveAttribute('open');
    await userEvent.click(evidence);
    for (const label of labels) {
      const description = label.replace(/^.*?: /, '');
      expect(await thread.findByText(text => text.startsWith(description), { selector: 'p' })).toBeVisible();
    }
    await userEvent.click(thread.getByText(language === 'zh' ? '任务操作' : 'Task actions'));
    expect(thread.getByText(taskState, { exact: true })).toBeVisible();
    expect(screen.queryByRole('button', { name: send })).toBeNull();
    expect(thread.getByRole('button', { name: sendGoal })).toBeDisabled();
  });

  it('keeps a ready session available after a recorded turn ends', async () => {
    vi.mocked(api.snapshot).mockResolvedValue({
      tasks: [{ ...task, project_id: session.project_id, state: 'running' }],
      sequence: 1, api_version: 1, database_schema_version: 19,
    });
    vi.mocked(sessionApi.sessionsList).mockResolvedValue([{ ...session, state: 'ready', turn_state: 'ended' }]);
    render(<App />);
    await userEvent.setup().click(await screen.findByRole('button', { name: `Open task: ${task.goal}` }));
    await userEvent.click(screen.getByText('Raw observable events'));
    expect(screen.getByText(text => text.startsWith('Ready · Turn ended (not accepted)'), { selector: 'p' })).toBeVisible();
    await userEvent.click(screen.getByText('Task actions'));
    expect(screen.getByRole('button', { name: 'Send task goal' })).toBeEnabled();
  });

  it('binds task drafts to an explicitly selected active project and disables archived projects', async () => {
    const user = userEvent.setup();
    const project: Project = { id: 'project-1', name: 'Product repository', identity: { canonical_root: '/source', device: 1, inode: 2, kind: { kind: 'directory' } }, archived: false, revision: 1, created_at: task.created_at, updated_at: task.updated_at };
    vi.mocked(api.projectsList).mockResolvedValue([project, { ...project, id: 'archived', name: 'Archive', archived: true }]);
    vi.mocked(api.create).mockResolvedValue({ ...task, project_id: project.id });
    render(<App />);
    await screen.findByText('Local daemon connected');
    const active = await screen.findByRole('option', { name: project.name });
    expect(active).toBeEnabled();
    await user.selectOptions(screen.getByLabelText('Project'), project.id);
    expect(screen.getByLabelText('Project')).toHaveValue(project.id);
    expect(screen.getByRole('option', { name: 'Archive (Archived)' })).toBeDisabled();
    // This case checks project binding; keyboard-by-keyboard creation remains
    // covered below. Paste the goal through the actual input/onChange path.
    const goal = screen.getByLabelText('Task goal');
    await user.click(goal);
    await user.paste(task.goal);
    expect(goal).toHaveValue(task.goal);
    await user.click(screen.getByRole('button', { name: 'Create task' }));
    await waitFor(() => expect(api.create).toHaveBeenCalledWith(task.goal, expect.any(String), project.id));
  });
  it('shows a disconnected error and never inserts demonstration tasks', async () => {
    vi.mocked(api.snapshot).mockRejectedValue(new Error('AUTH_REQUIRED'));
    render(<App />);
    expect(await screen.findByText('Background disconnected')).toBeVisible();
    expect(screen.getByText('AUTH_REQUIRED')).not.toBeVisible();
    await userEvent.click(screen.getByText('Connection evidence'));
    expect(screen.getByText('AUTH_REQUIRED')).toBeVisible();
    expect(screen.queryByRole('button', { name: 'Create task' })).toBeNull();
    expect(screen.queryByText('No tasks yet.')).toBeNull();
    expect(screen.getByRole('button', { name: 'Retry connection' })).toBeEnabled();
  });
  it('creates a task through the API and preserves cancellation-request semantics', async () => {
    const user = userEvent.setup();
    vi.mocked(api.create).mockResolvedValue(task);
    render(<App />);
    await screen.findByText('Local daemon connected');
    await user.type(screen.getByLabelText('Task goal'), task.goal);
    vi.mocked(api.snapshot).mockResolvedValue({ tasks: [task], sequence: 1, api_version:1,database_schema_version:17 });
    await user.click(screen.getByRole('button', { name: 'Create task' }));
    await waitFor(() => expect(api.create).toHaveBeenCalledWith(task.goal, expect.any(String)));
    expect(await screen.findByRole('heading', { name: task.goal })).toBeVisible();
    expect(screen.getByRole('button', { name: 'Configure execution' })).toBeVisible();
    expect(screen.queryByLabelText('Task message')).toBeNull();
    vi.mocked(api.cancel).mockResolvedValue({ ...task, state: 'cancel_requested', revision: 2 });
    vi.mocked(api.snapshot).mockResolvedValue({ tasks: [{ ...task, state: 'cancel_requested', revision: 2 }], sequence: 2, api_version:1,database_schema_version:17 });
    await user.click(screen.getByRole('tab', { name: 'Details' }));
    await user.click(screen.getByRole('button', { name: 'Request cancellation' }));
    await waitFor(() => expect(api.cancel).toHaveBeenCalledWith(task, expect.any(String)));
    expect(await screen.findByText('Cancellation requested')).toBeVisible();
    expect(screen.queryByRole('button', { name: 'Request cancellation' })).toBeNull();
    expect(screen.queryByText('Stopped (confirmed)')).not.toBeInTheDocument();
  });
  it('reuses the create idempotency key after an uncertain outcome', async () => {
    const user = userEvent.setup();
    vi.mocked(api.create).mockRejectedValue(new DaemonError('OUTCOME_UNKNOWN', 'Request timed out', 'unknown'));
    render(<App />);
    await screen.findByText('Local daemon connected');
    await user.type(screen.getByLabelText('Task goal'), 'Do not duplicate this task');
    await user.click(screen.getByRole('button', { name: 'Create task' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Refresh the task before retrying');
    await user.click(screen.getByRole('button', { name: 'Create task' }));
    const calls = vi.mocked(api.create).mock.calls;
    expect(calls).toHaveLength(2);
    expect(calls[0][1]).toEqual(calls[1][1]);
  });
  it('switches language and exposes navigation and unknown states as text', async () => {
    const user = userEvent.setup();
    vi.mocked(api.snapshot).mockResolvedValue({ tasks: [{ ...task, state: 'provider_extension' }], sequence: 1, api_version:1,database_schema_version:17 });
    render(<App />);
    expect(await screen.findByText('Unknown status')).toBeVisible();
    await user.click(screen.getByRole('button', { name: 'Settings' }));
    fireEvent.change(screen.getByLabelText('Language'), { target: { value: 'zh' } });
    await user.click(screen.getByRole('button', { name: '关闭' }));
    expect(screen.getByRole('navigation', { name: '主导航' })).toBeVisible();
    await user.click(screen.getByRole('button', { name: '能力' }));
    expect(screen.getByRole('heading', { name: '具名连接' })).toBeVisible();
    expect(document.documentElement.lang).toEqual('zh-CN');
  });
  it('renders native event payload as text without interpreting injected markup', async () => {
    const user = userEvent.setup();
    vi.mocked(api.events).mockResolvedValue({ events: [{ sequence: 1, event_id: 'event-1', aggregate_id: task.id, revision: 1, kind: 'task.created', occurred_at: task.created_at, source: 'user_action', payload: { goal: '<script>malicious()</script>' } }], next_cursor: 1, sequence: 1, gap: false });
    render(<App />);
    await screen.findByText('Local daemon connected');
    await user.click(screen.getByRole('button', { name: 'History' }));
    await user.click(screen.getByText('Task created'));
    expect(screen.getByText('task.created')).toBeVisible();
    expect(screen.getByText(/<script>malicious/)).toBeInTheDocument();
    expect(document.querySelector('script')).toBeNull();
  });
  it('filters the loaded history without changing the stored event window', async () => {
    const user = userEvent.setup();
    const base = {aggregate_id: task.id, revision: 1, occurred_at: task.created_at, source: 'fixture', payload: {}};
    vi.mocked(api.events).mockResolvedValue({events:[
      {...base, sequence: 1, event_id: 'task-created', kind: 'task.created'},
      {...base, sequence: 2, event_id: 'session-update', kind: 'session.update'},
    ], sequence: 2, next_cursor: 2, gap: false});
    render(<App />);
    await screen.findByText('Local daemon connected');
    await user.click(screen.getByRole('button', { name: 'History' }));
    const filter = screen.getByRole('combobox', { name: 'Event type' });
    await user.selectOptions(filter, 'messages');
    expect(screen.getByText('Agent activity')).toBeVisible();
    expect(screen.queryByText('Task created')).toBeNull();
    await user.selectOptions(filter, 'all');
    expect(screen.getByText('Task created')).toBeVisible();
    expect(api.events).toHaveBeenCalledTimes(1);
  });
  it('opens the newest event window beyond 100 events and explicitly pages older history', async () => {
    const user = userEvent.setup();
    vi.mocked(api.snapshot).mockResolvedValue({ tasks: [task], sequence: 230, api_version:1,database_schema_version:17 });
    vi.mocked(api.events).mockImplementation(async cursor => ({
      events: Array.from({ length: 100 }, (_, i) => ({ sequence: cursor + i + 1, event_id: `event-${cursor + i + 1}`, aggregate_id: task.id, revision: 1, kind: `record.${cursor + i + 1}`, occurred_at: task.created_at, source: 'user_action', payload: {} })),
      next_cursor: cursor + 100, sequence: 230, gap: false,
    }));
    render(<App />);
    await waitFor(() => expect(api.events).toHaveBeenCalledWith(130));
    await user.click(screen.getByRole('button', { name: 'History' }));
    await openEvent('record.230', user);
    expect(screen.queryByText('record.1')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Load earlier events' })).toBeVisible();
    await user.click(screen.getByRole('button', { name: 'Load earlier events' }));
    await waitFor(() => expect(api.events).toHaveBeenCalledWith(30));
    await openEvent('record.31', user);
    expect(screen.getByText('record.230')).toBeVisible();
  });
  it('shows byte-limited history honestly and follows the returned cursor without accumulating large pages', async () => {
    const user = userEvent.setup();
    vi.mocked(api.snapshot).mockResolvedValue({ tasks: [task], sequence: 230, api_version:1,database_schema_version:17 });
    vi.mocked(api.events).mockImplementation(async cursor => ({
      events: [{ sequence: cursor + 1, event_id: `event-${cursor + 1}`, aggregate_id: task.id, revision: 1, kind: `record.${cursor + 1}`, occurred_at: task.created_at, source: 'generated_fixture', payload: {} }],
      next_cursor: cursor + 1, sequence: 230, gap: false, byte_limited: true, page_bytes: 100,
    }));
    render(<App />);
    await waitFor(() => expect(api.events).toHaveBeenCalledWith(130));
    await user.click(screen.getByRole('button', { name: 'History' }));
    await openEvent('record.131', user);
    expect(screen.getByText(/This journal page is incomplete/)).toBeVisible();
    await user.click(screen.getByRole('button', { name: 'Read next event page' }));
    await waitFor(() => expect(api.events).toHaveBeenCalledWith(131));
    await openEvent('record.132', user);
    expect(screen.queryByText('record.131')).not.toBeInTheDocument();
  });

  it('keeps authenticated task cancellation available when only history is resource-limited', async () => {
    const user = userEvent.setup();
    vi.mocked(api.snapshot).mockResolvedValue({ tasks: [task], sequence: 1, api_version:1,database_schema_version:17 });
    vi.mocked(api.cancel).mockResolvedValue({ ...task, state: 'cancelled', revision: 2 });
    vi.mocked(api.events).mockRejectedValue(new DaemonError('RESOURCE_LIMIT', 'Journal page unavailable', 'not_started'));
    render(<App />);
    await user.click(await screen.findByRole('button', { name: `Open task: ${task.goal}` }));
    await user.click(screen.getByRole('tab', { name: 'Details' }));
    expect(screen.getByText('Local daemon connected')).toBeVisible();
    expect(screen.getByRole('alert')).toHaveTextContent('Journal page unavailable');
    const cancel = screen.getByRole('button', { name: 'Request cancellation' });
    expect(cancel).toBeEnabled();
    await user.click(cancel);
    await waitFor(() => expect(api.cancel).toHaveBeenCalled());
  });

});

it('bounds accumulated history by bytes and explains that records remain in the local journal', async () => {
  const user = userEvent.setup();
  const text = 'x'.repeat(3 * 1024 * 1024);
  vi.mocked(api.snapshot).mockResolvedValue({ tasks: [task], sequence: 500, api_version:1,database_schema_version:17 });
  vi.mocked(api.events).mockImplementation(async cursor => ({
    events: [{ sequence: cursor + 100, event_id: `large-${cursor + 100}`, aggregate_id: task.id, revision: 1, kind: `large.${cursor + 100}`, occurred_at: task.created_at, source: 'generated_fixture', payload: { text } }],
    next_cursor: cursor + 100, sequence: 500, gap: false, page_bytes: 3 * 1024 * 1024,
  }));
  render(<App />);
  await screen.findByText('Local daemon connected');
  await user.click(screen.getByRole('button', { name: 'History' }));
  await openEvent('large.500', user);
  await user.click(screen.getByRole('button', { name: 'Load earlier events' }));
  await openEvent('large.400', user);
  expect(screen.getByText('large.500')).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Load earlier events' }));
  await openEvent('large.300', user);
  expect(screen.getByText(/8 MiB budget/)).toHaveTextContent('Other events remain in the local journal');
  expect(screen.queryByText('large.400')).not.toBeInTheDocument();
  expect(screen.queryByText('large.500')).not.toBeInTheDocument();
  expect(api.events).toHaveBeenNthCalledWith(3, 200);
}, 10_000);

it('labels byte-limited backend pages without claiming complete displayed history', async () => {
  const user = userEvent.setup();
  vi.mocked(api.events).mockResolvedValue({ events: [], next_cursor: 0, sequence: 10, gap: false, page_bytes: 4 * 1024 * 1024, byte_limited: true });
  render(<App />);
  await screen.findByText('Local daemon connected');
  await user.click(screen.getByRole('button', { name: 'History' }));
  expect(screen.getByText('This is a byte-limited page. Additional events remain in the local journal.')).toBeVisible();
});

it('keeps authenticated task controls available when a single oversized journal record is refused', async () => {
  const user = userEvent.setup();
  vi.mocked(api.snapshot).mockResolvedValue({ tasks: [task], sequence: 1, api_version:1,database_schema_version:17 });
  vi.mocked(api.events).mockRejectedValue(new DaemonError('RESOURCE_LIMIT', 'One journal record exceeds the bounded event page; it was not skipped', 'not_started'));
  render(<App />);
  expect(await screen.findByRole('alert')).toHaveTextContent('it was not skipped');
  expect(screen.getByText('Local daemon connected')).toBeVisible();
  await user.click(screen.getByText(task.goal));
  await user.click(screen.getByRole('tab', { name: 'Details' }));
  expect(screen.getByRole('button', { name: 'Request cancellation' })).toBeEnabled();
});
