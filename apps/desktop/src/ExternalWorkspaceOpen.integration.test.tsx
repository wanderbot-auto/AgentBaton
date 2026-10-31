/** Requires Root mount076fcb1: real TaskCodeWorkspace -> real ExternalWorkspaceOpen/API.
 * Only daemon record methods and Tauri transport are mocked; no OS/IPC proof. */
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { api, DaemonError, type ManagedWorkspace, type Project, type Task } from './api';
import { TaskCodeWorkspace } from './TaskCodeWorkspace';
import { workspaceOpenMessages as openText } from './workspace-open-i18n';
import { taskCodeWorkspaceMessages as codeText } from './task-code-workspace-i18n';
import type { CodeWorkspacePreparation } from './workspace-preparation-wire';
import type { WorkspaceOpenResult, WorkspaceOpenSelection } from './workspace-open-wire';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('./api', async original => {
  const actual = await original<typeof import('./api')>();
  return { ...actual, api: { ...actual.api, taskGet: vi.fn(), projectsList: vi.fn(), workspacesList: vi.fn(), workspacesResolve: vi.fn(), workspacesPrepareTaskGit: vi.fn(), workspacesCreate: vi.fn() } };
});
const PROJECT = '11111111-1111-4111-8111-111111111111';
const GIT = '22222222-2222-4222-8222-222222222222';
const DATA = '33333333-3333-4333-8333-333333333333';
const PENDING = '44444444-4444-4444-8444-444444444444';
const NEW_PROJECT = '66666666-6666-4666-8666-666666666666';
const NEW_DATA = '77777777-7777-4777-8777-777777777777';
const date = '2026-10-31T00:00:00Z';
let serial = 0;
type Mode = 'dirty_resolver_refused' | 'non_code_directory' | 'pending_binding';
function fixture(mode: Mode = 'dirty_resolver_refused') {
  const number = ++serial;
  const project: Project = { id: PROJECT, name: 'Registered source', revision: 3, archived: false, created_at: date, updated_at: date,
    identity: { canonical_root: '/source dirty', device: 1, inode: 2, kind: { kind: 'git', common_dir: '/source dirty/.git', head: 'a'.repeat(40), dirty: true } } };
  let task: Task = { id: `workspace-open-mounted-${number}`, project_id: PROJECT, goal: 'Keep the existing task', state: 'draft', revision: 1, spec_revision: 2, created_at: date, updated_at: date };
  const rows: ManagedWorkspace[] = [
    { id: GIT, project_id: PROJECT, revision: 4, root: '/managed/git', state: 'ready', base_revision: 'a'.repeat(40), created_at: date, identity: { canonical_root: '/managed/git', device: 1, inode: 3, kind: { kind: 'git', common_dir: '/source dirty/.git', head: 'a'.repeat(40), dirty: true } } },
    { id: DATA, project_id: PROJECT, revision: 5, root: '/managed/data', state: 'ready', base_revision: '', created_at: date, identity: { canonical_root: '/managed/data', device: 1, inode: 4, kind: { kind: 'directory' } } },
    { id: PENDING, project_id: PROJECT, revision: 1, root: `/managed/${PENDING}`, state: 'creating', base_revision: 'a'.repeat(40), created_at: date, identity: null },
  ];
  vi.mocked(api.workspacesResolve).mockRejectedValue(new DaemonError('INPUT_CHANGED', 'Dirty source cannot prepare code', 'not_started'));
  if (mode === 'non_code_directory') {
    project.identity = { canonical_root: '/source data', device: 1, inode: 2, kind: { kind: 'directory' } };
    const preparation: CodeWorkspacePreparation = { task_id: task.id, task_revision: 1, spec_revision: 2, project_id: PROJECT, project_revision: 3, host_id: 'local', source: { canonical_root: '/source data', device: '1', inode: '2', kind: { kind: 'directory' } }, source_head: null, preparation_key: null, state: 'needs_dataset_selection', reason: 'selected_dataset_required', candidates: [], execution_authorized: false, creates_workspace: false };
    vi.mocked(api.workspacesResolve).mockResolvedValue(preparation);
  } else if (mode === 'pending_binding') {
    task = { ...task, revision: 2, code_workspace_binding: { state: 'pending', revision: 1, managed_pin: null, receipt: { version: 1, binding_id: `binding-${number}`, workspace_id: PENDING, task_id: task.id, project_id: PROJECT, host_id: 'local', original_task_revision: 1, reserved_task_revision: 2, spec_revision: 2, project_revision: 3, source_pin: { canonical_root: '/source dirty', device: '1', inode: '2', common_dir: '/source dirty/.git', head: 'a'.repeat(40) }, managed_root: '/managed', preparation_key: 'code-prep:' + 'b'.repeat(64), request_key: `pending-${number}`, request_digest: 'c'.repeat(64), created_at: date } } };
  }
  vi.mocked(api.taskGet).mockResolvedValue(task);
  vi.mocked(api.projectsList).mockResolvedValue([project]);
  vi.mocked(api.workspacesList).mockResolvedValue(rows);
  vi.mocked(invoke).mockResolvedValue({ status: 'handoff_accepted', code: 'accepted' });
  return { task, project, rows };
}
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; }
const target = () => screen.getByRole('combobox', { name: openText.en.target });
const open = () => screen.getByRole('button', { name: openText.en.open });
const sourceSelection = (projectId = PROJECT, revision = 3): WorkspaceOpenSelection => ({ kind: 'project_source', project_id: projectId, expected_project_revision: revision });
const dataSelection = (projectId = PROJECT, revision = 3, workspaceId = DATA): WorkspaceOpenSelection => ({ kind: 'managed_workspace', project_id: projectId, expected_project_revision: revision, workspace_id: workspaceId, expected_workspace_revision: 5 });
async function select(selection: WorkspaceOpenSelection) { await waitFor(() => expect(target()).toBeEnabled()); await userEvent.selectOptions(target(), JSON.stringify(selection)); }
function codePreparationStillClosed() {
  const prepare = screen.queryByRole('button', { name: codeText.en.prepare });
  if (prepare) { expect(prepare).toBeDisabled(); fireEvent.click(prepare); }
  expect(api.workspacesPrepareTaskGit).not.toHaveBeenCalled(); expect(api.workspacesCreate).not.toHaveBeenCalled();
}
function newProject(f: ReturnType<typeof fixture>) {
  const project: Project = { ...f.project, id: NEW_PROJECT, name: 'New parent source', revision: 4, identity: { canonical_root: '/new source', device: 1, inode: 6, kind: { kind: 'directory' } } };
  const rows: ManagedWorkspace[] = [{ ...f.rows[1], id: NEW_DATA, project_id: NEW_PROJECT, root: '/new managed/data', identity: { canonical_root: '/new managed/data', device: 1, inode: 7, kind: { kind: 'directory' } } }];
  vi.mocked(api.projectsList).mockResolvedValue([f.project, project]);
  return { project, rows, task: { ...f.task, project_id: NEW_PROJECT, revision: f.task.revision + 1 } };
}
beforeEach(() => vi.resetAllMocks());
describe('Root076 TaskCodeWorkspace mount / independent directory-only action, mocked transport', () => {
  it.each(['dirty_resolver_refused', 'non_code_directory', 'pending_binding'] as const)('keeps code preparation closed in %s while explicit source and ready data handoffs remain independent', async mode => {
    const f = fixture(mode);
    render(<TaskCodeWorkspace task={f.task} language="en" connected autoOpen />);
    if (mode === 'dirty_resolver_refused') await screen.findByText(/Dirty source cannot prepare code/);
    else if (mode === 'non_code_directory') await screen.findByText(codeText.en.directory);
    else await screen.findByText(codeText.en.pending);
    codePreparationStillClosed(); expect(invoke).not.toHaveBeenCalled();
    await select(mode === 'dirty_resolver_refused' ? sourceSelection() : dataSelection());
    expect(screen.queryByRole('option', { name: new RegExp(PENDING) })).toBeNull();
    await userEvent.click(open()); await screen.findByText(openText.en.accepted);
    expect(invoke).toHaveBeenCalledExactlyOnceWith('open_workspace_terminal', { request: { selection: mode === 'dirty_resolver_refused' ? sourceSelection() : dataSelection(), terminal: 'macos_terminal' } });
    codePreparationStillClosed(); expect(f.task.state).toBe('draft');
    expect(api.workspacesList).toHaveBeenCalledWith(PROJECT);
    if (mode === 'pending_binding') expect(api.workspacesResolve).not.toHaveBeenCalled();
  });
  it('uses props project epoch even when the parent Task read stays stale, and discards its old catalog', async () => {
    const f = fixture('non_code_directory'), old = deferred<ManagedWorkspace[]>();
    vi.mocked(api.workspacesList).mockReturnValueOnce(old.promise);
    const view = render(<TaskCodeWorkspace task={f.task} language="en" connected autoOpen />);
    await waitFor(() => expect(api.workspacesList).toHaveBeenCalledWith(PROJECT));
    await screen.findByText(codeText.en.directory);
    const next = newProject(f);
    vi.mocked(api.workspacesList).mockImplementation(async id => id === NEW_PROJECT ? next.rows : f.rows);
    // taskGet deliberately continues returning the old parent record. This must
    // keep code preparation closed, not override the child's new props epoch.
    view.rerender(<TaskCodeWorkspace task={next.task} language="en" connected autoOpen />);
    await waitFor(() => expect(api.workspacesList).toHaveBeenCalledWith(NEW_PROJECT));
    await act(async () => old.resolve(f.rows));
    await select(dataSelection(NEW_PROJECT, 4, NEW_DATA));
    expect(screen.queryByRole('option', { name: new RegExp('Project source · /source data') })).toBeNull();
    expect(invoke).not.toHaveBeenCalled(); codePreparationStillClosed();
    await userEvent.click(open()); await screen.findByText(openText.en.accepted);
    expect(invoke).toHaveBeenCalledExactlyOnceWith('open_workspace_terminal', { request: { selection: dataSelection(NEW_PROJECT, 4, NEW_DATA), terminal: 'macos_terminal' } });
    codePreparationStillClosed();
  });
  it.each(['project', 'task'] as const)('rejects old Native reply after %s switch, releases only the UI flight gate, and requires a new explicit current selection', async change => {
    const f = fixture('non_code_directory'), native = deferred<WorkspaceOpenResult>(); vi.mocked(invoke).mockReturnValueOnce(native.promise);
    const view = render(<TaskCodeWorkspace task={f.task} language="en" connected autoOpen />);
    await screen.findByText(codeText.en.directory); await select(sourceSelection()); await userEvent.click(open());
    expect(invoke).toHaveBeenCalledExactlyOnceWith('open_workspace_terminal', { request: { selection: sourceSelection(), terminal: 'macos_terminal' } });
    const next = newProject(f); if (change === 'task') next.task.id += '-next';
    vi.mocked(api.workspacesList).mockResolvedValue(next.rows);
    view.rerender(<TaskCodeWorkspace task={next.task} language="en" connected autoOpen />);
    // Flush the parent's stale read refusal before settling the old Native
    // call. An unrelated late parent render must not mask a stuck flight gate.
    await screen.findByRole('alert');
    await act(async () => native.resolve({ status: 'handoff_accepted', code: 'accepted' }));
    expect(screen.queryByText(openText.en.accepted)).toBeNull(); expect(invoke).toHaveBeenCalledTimes(1); codePreparationStillClosed();
    // The earlier explicit OS request cannot be undone by a UI switch. Only a
    // manual new read + current selection can dispatch another request.
    const manualRefresh = screen.getByRole('button', { name: openText.en.refresh });
    await waitFor(() => expect(manualRefresh).toBeEnabled());
    expect(invoke).toHaveBeenCalledTimes(1);
    await userEvent.click(manualRefresh); await select(dataSelection(NEW_PROJECT, 4, NEW_DATA));
    vi.mocked(invoke).mockResolvedValue({ status: 'handoff_accepted', code: 'accepted' }); await userEvent.click(open()); await screen.findByText(openText.en.accepted);
    expect(invoke).toHaveBeenCalledTimes(2); expect(invoke).toHaveBeenLastCalledWith('open_workspace_terminal', { request: { selection: dataSelection(NEW_PROJECT, 4, NEW_DATA), terminal: 'macos_terminal' } });
    codePreparationStillClosed();
  });
});
