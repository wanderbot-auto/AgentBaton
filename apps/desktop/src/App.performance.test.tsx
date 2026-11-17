import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { App } from './App';
import { api, DaemonError, type Project, type Snapshot, type Task } from './api';
import { sessionApi } from './session-api';
vi.mock('./api', async original => ({ ...await original<typeof import('./api')>(), api: { snapshot: vi.fn(), events: vi.fn(), projectsList: vi.fn() } }));
vi.mock('./session-api', () => ({ sessionApi: { connectionsList: vi.fn(), sessionsList: vi.fn() } }));
vi.mock('./RecoveryCenter', () => ({ useRecoveryCenter: () => ({}), RecoveryBanner: () => null, RecoveryCenter: () => null }));
function deferred<T>() { let resolve!: (v: T) => void, reject!: (e: Error) => void; const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; }
const snapshot: Snapshot = { tasks: [], sequence: 0, api_version: 1, database_schema_version: 18 };
beforeEach(() => {
  vi.resetAllMocks(); localStorage.clear(); localStorage.setItem('agentbaton.language', 'en');
  vi.mocked(api.snapshot).mockResolvedValue(snapshot);
  vi.mocked(api.events).mockResolvedValue({ events: [], sequence: 0, next_cursor: 0, gap: false });
  vi.mocked(api.projectsList).mockResolvedValue([]);
  vi.mocked(sessionApi.connectionsList).mockResolvedValue([]);
  vi.mocked(sessionApi.sessionsList).mockResolvedValue([]);
});
async function ready() { render(<App />); await screen.findByText('Local daemon connected'); await waitFor(() => expect(sessionApi.sessionsList).toHaveBeenCalledTimes(1)); }
it('starts independent catalog reads while a connected refresh snapshot is pending', async () => {
  await ready(); const pending = deferred<Snapshot>(); vi.mocked(api.snapshot).mockReturnValueOnce(pending.promise);
  fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
  expect(api.projectsList).toHaveBeenCalledTimes(2);
  expect(sessionApi.connectionsList).toHaveBeenCalledTimes(2);
  expect(sessionApi.sessionsList).toHaveBeenCalledTimes(2);
  // Cursor and receipt recovery still depend on a verified snapshot.
  expect(api.events).toHaveBeenCalledTimes(1);
  await act(async () => pending.resolve(snapshot));
  await waitFor(() => expect(api.events).toHaveBeenCalledTimes(2));
});
it('waits for cold-start bootstrap before attempting ordinary daemon reads', async () => {
  const pending = deferred<Snapshot>(); vi.mocked(api.snapshot).mockReturnValueOnce(pending.promise); render(<App />);
  expect(api.projectsList).not.toHaveBeenCalled(); expect(sessionApi.connectionsList).not.toHaveBeenCalled();
  await act(async () => pending.resolve(snapshot));
  await waitFor(() => expect(sessionApi.sessionsList).toHaveBeenCalledTimes(1));
});
it('handles an early catalog failure and failed refresh without publishing catalog results', async () => {
  await ready(); const pending = deferred<Snapshot>(); vi.mocked(api.snapshot).mockReturnValueOnce(pending.promise);
  vi.mocked(api.projectsList).mockRejectedValueOnce(new Error('catalog unavailable'));
  fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
  await act(async () => {});
  await act(async () => pending.reject(new Error('snapshot unavailable')));
  expect(await screen.findByRole('alert')).toHaveTextContent('snapshot unavailable');
  expect(screen.getByRole('heading', { name: 'Local daemon unavailable' })).toBeVisible();
  expect(api.events).toHaveBeenCalledTimes(1);
  expect(api.projectsList).toHaveBeenCalledTimes(2);
});
it('rechecks only failed speculative catalogs once after successful daemon recovery', async () => {
  await ready(); const pending = deferred<Snapshot>(); vi.mocked(api.snapshot).mockReturnValueOnce(pending.promise);
  vi.mocked(sessionApi.connectionsList).mockRejectedValueOnce(new DaemonError('PROVIDER_DISCONNECTED', 'old daemon stopped', 'not_started'));
  fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
  await act(async () => {}); expect(sessionApi.connectionsList).toHaveBeenCalledTimes(2);
  await act(async () => pending.resolve(snapshot));
  await waitFor(() => expect(sessionApi.connectionsList).toHaveBeenCalledTimes(3));
  expect(api.projectsList).toHaveBeenCalledTimes(2); expect(sessionApi.sessionsList).toHaveBeenCalledTimes(2);
  expect(screen.getByText('Local daemon connected')).toBeVisible(); expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});
it('reports persistent catalog failures after at most one recovery read', async () => {
  await ready(); vi.mocked(sessionApi.sessionsList).mockRejectedValue(new DaemonError('AUTH_REQUIRED', 'catalog authentication failed', 'not_started'));
  fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('catalog authentication failed');
  expect(sessionApi.sessionsList).toHaveBeenCalledTimes(3);
  expect(screen.getByRole('heading', { name: 'Local daemon unavailable' })).toBeVisible();
});
it('does not retry failed catalog reads after a newer refresh supersedes them', async () => {
  await ready(); const old = deferred<Project[]>(); vi.mocked(api.projectsList).mockReturnValueOnce(old.promise);
  fireEvent.click(screen.getByRole('button', { name: 'Refresh' })); await screen.findByText('Local daemon connected');
  fireEvent.click(screen.getByRole('button', { name: 'Refresh' })); await waitFor(() => expect(api.events).toHaveBeenCalledTimes(3));
  await act(async () => old.reject(new Error('old read failed')));
  expect(api.projectsList).toHaveBeenCalledTimes(3); expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});
it('does not reread conversation text or stringify folded event payloads while typing', async () => {
  let textReads = 0;
  const task: Task = { id: 'task-1', project_id: 'local', goal: 'Task thread', state: 'draft', revision: 1, created_at: 'now', updated_at: 'now' };
  vi.mocked(api.snapshot).mockResolvedValue({ ...snapshot, tasks: [task], sequence: 1 });
  vi.mocked(sessionApi.sessionsList).mockResolvedValue([{ id: 'session-1', task_id: task.id, project_id: 'local', workspace_id: 'w', connection_id: 'c', grant_id: 'g', state: 'ready', revision: 1, native_session_id: null, created_at: 'now', updated_at: 'now', last_error: null, turn_state: null }]);
  vi.mocked(api.events).mockResolvedValue({ events: [{ sequence: 1, event_id: 'e', aggregate_id: 'session-1', revision: 1, kind: 'session.message_queued', occurred_at: 'now', source: 'user', payload: { get text() { textReads++; return 'Recorded message'; }, extra: 'x'.repeat(128 * 1024) } }], sequence: 1, next_cursor: 1, gap: false });
  await ready(); fireEvent.click(screen.getByTestId('task-card')); await screen.findByText('Recorded message');
  const input = screen.getByTestId('goal-input'); expect(input).toBeEnabled(); const before = textReads;
  for (let i = 0; i < 10; i++) fireEvent.change(input, { target: { value: `Message ${i}` } });
  expect(input).toHaveValue('Message 9'); expect(textReads).toBe(before);
});
it('keeps the real App card subtree unchanged across ten composer edits', async () => {
  let reads = 0;
  const task: Task = { id: 'task-1', get goal() { reads++; return 'Original card'; }, state: 'draft', revision: 1, created_at: 'now', updated_at: 'now' };
  vi.mocked(api.snapshot).mockResolvedValue({ ...snapshot, tasks: [task] });
  await ready(); expect(screen.getByTestId('task-card')).toHaveTextContent('Original card');
  const before = reads;
  for (let i = 0; i < 10; i++) fireEvent.change(screen.getByRole('textbox', { name: 'Task goal' }), { target: { value: `Draft ${i}` } });
  expect(screen.getByRole('textbox', { name: 'Task goal' })).toHaveValue('Draft 9');
  expect(reads).toBe(before);
});
it('ignores catalog results from an older overlapping warm refresh', async () => {
  await ready(); const old = deferred<Project[]>(); vi.mocked(api.projectsList).mockReturnValueOnce(old.promise);
  fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
  await screen.findByText('Local daemon connected');
  const project: Project = { id: 'latest', name: 'Latest project', identity: { canonical_root: '/source', device: 1, inode: 2, kind: { kind: 'directory' } }, revision: 1, archived: false, created_at: 'now', updated_at: 'now' };
  vi.mocked(api.projectsList).mockResolvedValueOnce([project]);
  fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
  await screen.findByRole('option', { name: 'Latest project' });
  await act(async () => old.resolve([{ ...project, id: 'old', name: 'Obsolete project' }]));
  expect(screen.getByRole('option', { name: 'Latest project' })).toBeInTheDocument();
  expect(screen.queryByRole('option', { name: 'Obsolete project' })).not.toBeInTheDocument();
});
