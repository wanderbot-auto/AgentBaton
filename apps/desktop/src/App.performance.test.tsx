import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { App } from './App';
import { api, type Project, type Snapshot, type Task } from './api';
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
