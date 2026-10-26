import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { App } from './App';
import { api, DaemonError, type Task, type Project } from './api';
import { sessionApi } from './session-api';
vi.mock('./session-api', () => ({ sessionApi: { connectionsList: vi.fn(), sessionsList: vi.fn() } }));

// These tests mock only the Tauri transport. They are not native IPC evidence.
vi.mock('./api', async importOriginal => {
  const actual = await importOriginal<typeof import('./api')>();
  return { ...actual, api: { snapshot: vi.fn(), create: vi.fn(), cancel: vi.fn(), events: vi.fn(), projectsList: vi.fn(), projectsImport: vi.fn(), projectsArchive: vi.fn(), projectsReassociate: vi.fn(), workspacesCreate: vi.fn(), workspacesList: vi.fn() } };
});
const task: Task = { id: 'task-1', goal: 'Check cancellation evidence', state: 'draft', revision: 1, created_at: '2026-10-27T00:00:00Z', updated_at: '2026-10-27T00:00:00Z' };
beforeEach(() => {
  vi.resetAllMocks();
  localStorage.clear();
  localStorage.setItem('agentbaton.language', 'en');
  vi.mocked(api.projectsList).mockResolvedValue([]);
  vi.mocked(api.workspacesList).mockResolvedValue([]);
  vi.mocked(sessionApi.connectionsList).mockResolvedValue([]);
  vi.mocked(sessionApi.sessionsList).mockResolvedValue([]);
  vi.mocked(api.snapshot).mockResolvedValue({ tasks: [], sequence: 0, api_version: 1 });
  vi.mocked(api.events).mockResolvedValue({ events: [], next_cursor: 0, sequence: 0, gap: false });
});
describe('Task workspace using mocked Tauri transport', () => {
  it('binds task drafts to an explicitly selected active project and disables archived projects', async () => {
    const user = userEvent.setup();
    const project: Project = { id: 'project-1', name: 'Product repository', identity: { canonical_root: '/source', device: 1, inode: 2, kind: { kind: 'directory' } }, archived: false, revision: 1, created_at: task.created_at, updated_at: task.updated_at };
    vi.mocked(api.projectsList).mockResolvedValue([project, { ...project, id: 'archived', name: 'Archive', archived: true }]);
    vi.mocked(api.create).mockResolvedValue({ ...task, project_id: project.id });
    render(<App />);
    await screen.findByText('Local daemon connected');
    await user.selectOptions(screen.getByLabelText('Project'), project.id);
    expect(screen.getByRole('option', { name: 'Archive (Archived)' })).toBeDisabled();
    await user.type(screen.getByLabelText('Task goal'), task.goal);
    await user.click(screen.getByRole('button', { name: 'Create task' }));
    await waitFor(() => expect(api.create).toHaveBeenCalledWith(task.goal, expect.any(String), project.id));
  });
  it('shows a disconnected error and never inserts demonstration tasks', async () => {
    vi.mocked(api.snapshot).mockRejectedValue(new Error('AUTH_REQUIRED'));
    render(<App />);
    expect(await screen.findByRole('heading', { name: 'Local daemon unavailable' })).toBeVisible();
    expect(screen.getByRole('alert')).toHaveTextContent('AUTH_REQUIRED');
    expect(screen.getByRole('button', { name: 'Create task' })).toBeDisabled();
    expect(screen.getByText('No tasks yet. Create a task to record your goal.')).toBeVisible();
  });
  it('creates a task through the API and preserves cancellation-request semantics', async () => {
    const user = userEvent.setup();
    vi.mocked(api.create).mockResolvedValue(task);
    render(<App />);
    await screen.findByText('Local daemon connected');
    await user.type(screen.getByLabelText('Task goal'), task.goal);
    vi.mocked(api.snapshot).mockResolvedValue({ tasks: [task], sequence: 1, api_version: 1 });
    await user.click(screen.getByRole('button', { name: 'Create task' }));
    await waitFor(() => expect(api.create).toHaveBeenCalledWith(task.goal, expect.any(String)));
    expect(await screen.findByRole('heading', { name: task.goal })).toBeVisible();
    expect(screen.getByText('No execution evidence yet. This task has not been dispatched to a Provider.')).toBeVisible();
    vi.mocked(api.cancel).mockResolvedValue({ ...task, state: 'cancel_requested', revision: 2 });
    vi.mocked(api.snapshot).mockResolvedValue({ tasks: [{ ...task, state: 'cancel_requested', revision: 2 }], sequence: 2, api_version: 1 });
    await user.click(screen.getByRole('tab', { name: 'Details' }));
    await user.click(screen.getByRole('button', { name: 'Request cancellation' }));
    await waitFor(() => expect(api.cancel).toHaveBeenCalledWith(task, expect.any(String)));
    expect(await screen.findByText('Cancellation requested')).toBeVisible();
    expect(screen.getByRole('button', { name: 'Request cancellation' })).toBeDisabled();
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
    vi.mocked(api.snapshot).mockResolvedValue({ tasks: [{ ...task, state: 'provider_extension' }], sequence: 1, api_version: 1 });
    render(<App />);
    expect(await screen.findByText('Unknown status: provider_extension')).toBeVisible();
    fireEvent.change(screen.getByLabelText('Language'), { target: { value: 'zh' } });
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
    await user.click(screen.getByText('task.created'));
    expect(screen.getByText(/<script>malicious/)).toBeInTheDocument();
    expect(document.querySelector('script')).toBeNull();
  });
  it('opens the newest event window beyond 100 events and explicitly pages older history', async () => {
    const user = userEvent.setup();
    vi.mocked(api.snapshot).mockResolvedValue({ tasks: [task], sequence: 230, api_version: 1 });
    vi.mocked(api.events).mockImplementation(async cursor => ({
      events: Array.from({ length: 100 }, (_, i) => ({ sequence: cursor + i + 1, event_id: `event-${cursor + i + 1}`, aggregate_id: task.id, revision: 1, kind: `record.${cursor + i + 1}`, occurred_at: task.created_at, source: 'user_action', payload: {} })),
      next_cursor: cursor + 100, sequence: 230, gap: false,
    }));
    render(<App />);
    await waitFor(() => expect(api.events).toHaveBeenCalledWith(130));
    await user.click(screen.getByRole('button', { name: 'History' }));
    expect(await screen.findByText('record.230')).toBeVisible();
    expect(screen.queryByText('record.1')).not.toBeInTheDocument();
    expect(screen.getByText('Earlier journal events are not displayed yet.')).toBeVisible();
    await user.click(screen.getByRole('button', { name: 'Load earlier events' }));
    await waitFor(() => expect(api.events).toHaveBeenCalledWith(30));
    expect(await screen.findByText('record.31')).toBeVisible();
    expect(screen.getByText('record.230')).toBeVisible();
  });
});
