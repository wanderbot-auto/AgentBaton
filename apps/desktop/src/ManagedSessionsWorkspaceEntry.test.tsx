import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ComponentProps } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { api, DaemonError, type ManagedWorkspace, type Project, type Task } from './api';
import { ManagedSessions } from './ManagedSessions';
import { sessionApi, type NamedConnection } from './session-api';
import type { CodeWorkspacePreparation } from './workspace-preparation-wire';
import type { TaskGitWorkspacePreparation, TaskWorkspaceBindingView } from './task-workspace-binding-wire';
import type { TaskCodeWorkspace } from './TaskCodeWorkspace';

// Capture only the optional UI notification. The real preparation component,
// including its original key/uncertainty state machine, still renders below.
const notifications: { taskId: string; ready: ((id: string) => void) | undefined }[] = [];
vi.mock('./TaskCodeWorkspace', async original => {
  const actual = await original<typeof import('./TaskCodeWorkspace')>();
  return { ...actual, TaskCodeWorkspace: (props: ComponentProps<typeof TaskCodeWorkspace>) => { notifications.push({ taskId: props.task.id, ready: props.onBindingReady }); return <actual.TaskCodeWorkspace {...props} />; } };
});
vi.mock('./api', async original => ({ ...await original<typeof import('./api')>(), api: { taskGet: vi.fn(), projectsList: vi.fn(), workspacesList: vi.fn(), workspacesResolve: vi.fn(), workspacesPrepareTaskGit: vi.fn(), snapshot: vi.fn(), events: vi.fn() } }));
vi.mock('./session-api', () => ({ sessionApi: { connectionsList: vi.fn(), sessionsList: vi.fn(), grantsList: vi.fn(), grantsCreate: vi.fn(), grantsRevoke: vi.fn(), sessionsCreate: vi.fn(), sessionsGet: vi.fn(), sessionsSend: vi.fn(), sessionsCancel: vi.fn() } }));
const date = '2026-10-30T00:00:00Z';
let serial = 0;
function fixture() {
  const n = ++serial;
  const project: Project = { id: `entry-project-${n}`, name: 'Selected project', revision: 3, archived: false, created_at: date, updated_at: date, identity: { canonical_root: `/source-${n}`, device: 1, inode: 2, kind: { kind: 'git', common_dir: `/source-${n}/.git`, head: 'a'.repeat(40), dirty: false } } };
  const task: Task = { id: `entry-task-${n}`, project_id: project.id, goal: 'An explicit task', state: 'draft', revision: 1, spec_revision: 1, code_workspace_binding: null, created_at: date, updated_at: date };
  const pin = { canonical_root: `/source-${n}`, device: '9007199254740993', inode: '18446744073709551615', common_dir: `/source-${n}/.git`, head: 'a'.repeat(40) };
  const preparation: CodeWorkspacePreparation = { task_id: task.id, task_revision: 1, spec_revision: 1, project_id: project.id, project_revision: 3, host_id: 'local', source: { canonical_root: pin.canonical_root, device: pin.device, inode: pin.inode, kind: { kind: 'git', common_dir: pin.common_dir, head: pin.head, dirty: false } }, source_head: pin.head, preparation_key: 'code-prep:' + 'b'.repeat(64), state: 'needs_git_workspace', reason: 'isolated_git_workspace_required', candidates: [], creates_workspace: false, execution_authorized: false };
  const own: ManagedWorkspace = { id: `own-${n}`, project_id: project.id, root: `/managed/own-${n}`, state: 'ready', revision: 1, created_at: date, base_revision: pin.head, identity: { ...project.identity, canonical_root: `/managed/own-${n}` } };
  const other = { ...own, id: `other-${n}`, root: `/managed/other-${n}` };
  const connection: NamedConnection = { id: `connection-${n}`, name: 'User connection', provider: 'custom_acp', executable: '/explicit/program', arguments: [], executable_sha256: 'd'.repeat(64), environment_id: 'env', identity_ref: 'unknown:identity', config_revision: 'config', host_id: 'local', approval_id: 'native-probe', probe_state: 'ready', revision: 2, capability_snapshot_id: 'caps', identity_status: 'unknown', config_status: 'private_environment_not_sandbox', version_evidence: null, created_at: date };
  function view(selection: TaskGitWorkspacePreparation, key: string): TaskWorkspaceBindingView {
    const receipt = { version: 1 as const, binding_id: `binding-${n}`, workspace_id: own.id, task_id: task.id, project_id: project.id, host_id: 'local' as const, original_task_revision: selection.expected_task_revision, reserved_task_revision: selection.expected_task_revision + 1, spec_revision: selection.expected_spec_revision, project_revision: selection.expected_project_revision, source_pin: structuredClone(selection.expected_source_pin), managed_root: '/managed', preparation_key: selection.preparation_key, request_key: key, request_digest: 'c'.repeat(64), created_at: date };
    const managed = { ...pin, canonical_root: own.root, inode: '9007199254740995' };
    return { original_receipt: receipt, binding: { receipt: structuredClone(receipt), revision: 2, state: 'ready', managed_pin: managed }, task_revision: 3, observed_managed_pin: managed, identity_revalidated: true, execution_authorized: false };
  }
  return { project, task, pin, preparation, own, other, connection, view };
}
type Fixture = ReturnType<typeof fixture>;
const prepare = () => screen.getByRole('button', { name: 'Prepare isolated Git workspace for this task' });
const workspace = () => screen.getByLabelText('Managed workspace');
const grant = () => screen.getByRole('button', { name: 'Create bounded execution grant' });
const session = () => screen.getByRole('button', { name: 'Create managed session' });
const refresh = () => screen.getByRole('button', { name: 'Refresh session records' });
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; }
function setup(f: Fixture) {
  const server = { task: f.task, rows: [f.other] };
  vi.mocked(api.taskGet).mockImplementation(async id => { if (id !== f.task.id) throw new Error('Unknown fixture Task'); return structuredClone(server.task); });
  vi.mocked(api.projectsList).mockResolvedValue([f.project]); vi.mocked(api.workspacesList).mockImplementation(async () => [...server.rows]); vi.mocked(api.workspacesResolve).mockResolvedValue(f.preparation);
  vi.mocked(api.snapshot).mockResolvedValue({ tasks: [f.task], sequence: 0, api_version:1,database_schema_version:17 }); vi.mocked(api.events).mockResolvedValue({ events: [], next_cursor: 0, sequence: 0, gap: false });
  vi.mocked(sessionApi.connectionsList).mockResolvedValue([f.connection]); vi.mocked(sessionApi.sessionsList).mockResolvedValue([]); vi.mocked(sessionApi.grantsList).mockResolvedValue([]);
  function commit(selection: TaskGitWorkspacePreparation, key: string) { const value = f.view(selection, key); server.task = { ...f.task, revision: 3, code_workspace_binding: value.binding }; server.rows = [f.other, f.own]; return value; }
  return { server, commit };
}
function element(f: Fixture, extras: Partial<ComponentProps<typeof ManagedSessions>> = {}) { return <ManagedSessions language="en" connected projects={[f.project]} tasks={[f.task]} projectId={f.project.id} initialTaskId={f.task.id} onProjectChange={vi.fn()} refreshTasks={vi.fn().mockResolvedValue(undefined)} {...extras} />; }
async function entryReady() { await waitFor(() => expect(prepare()).toBeEnabled()); }
function noExecution() { expect(sessionApi.grantsCreate).not.toHaveBeenCalled(); expect(sessionApi.sessionsCreate).not.toHaveBeenCalled(); expect(sessionApi.sessionsSend).not.toHaveBeenCalled(); }
beforeEach(() => { vi.resetAllMocks(); notifications.length = 0; });

describe('Execution workspace entry: real preparation component with mocked fixed transport, not FS/Owner/Provider proof', () => {
  it('reads the authoritative resolver and opens the reused entry without preparing or granting', async () => {
    const f = fixture(); setup(f); render(element(f)); await entryReady();
    expect(api.workspacesResolve).toHaveBeenCalledWith({ task_id: f.task.id, expected_task_revision: 1, expected_project_revision: 3, host_id: 'local' });
    expect(screen.getByTestId('task-code-workspace')).toHaveAttribute('open'); expect(workspace()).toBeDisabled(); expect(workspace()).toHaveValue(''); expect(grant()).toBeDisabled(); expect(session()).toBeDisabled(); expect(api.workspacesPrepareTaskGit).not.toHaveBeenCalled(); noExecution();
  });
  it('refreshes Task and scope lists only after real component preparation and then locks to its own ready workspace', async () => {
    const user = userEvent.setup(), f = fixture(), state = setup(f); vi.mocked(api.workspacesPrepareTaskGit).mockImplementation(async (selection, key) => state.commit(selection, key));
    render(element(f)); await entryReady(); const beforeTask = vi.mocked(api.taskGet).mock.calls.length, beforeLists = vi.mocked(api.workspacesList).mock.calls.length;
    await user.click(prepare()); await waitFor(() => expect(workspace()).toHaveValue(f.own.id));
    expect(workspace()).toBeDisabled(); expect(screen.queryByTestId('session-task-workspace-entry')).toBeNull(); expect(vi.mocked(api.taskGet).mock.calls.length).toBeGreaterThan(beforeTask); expect(vi.mocked(api.workspacesList).mock.calls.length).toBeGreaterThan(beforeLists); expect(vi.mocked(sessionApi.grantsList).mock.calls.length).toBeGreaterThan(1);
    expect(api.workspacesPrepareTaskGit).toHaveBeenCalledExactlyOnceWith({ task_id: f.task.id, expected_task_revision: 1, expected_spec_revision: 1, expected_project_revision: 3, host_id: 'local', expected_source_pin: f.pin, preparation_key: f.preparation.preparation_key }, expect.any(String));
    expect(screen.getByLabelText('Execution grant')).toHaveValue(''); expect(session()).toBeDisabled(); noExecution();
  });
  it('keeps a known not-started failure visible and needs an explicit refresh and user retry', async () => {
    const user = userEvent.setup(), f = fixture(), state = setup(f); vi.mocked(api.workspacesPrepareTaskGit).mockRejectedValueOnce(new DaemonError('WORKSPACE_BUSY', 'Writer still active', 'not_started')).mockImplementationOnce(async (selection, key) => state.commit(selection, key));
    render(element(f)); await entryReady(); await user.click(prepare()); expect(await screen.findByRole('alert')).toHaveTextContent('Writer still active'); expect(workspace()).toBeDisabled(); expect(api.workspacesPrepareTaskGit).toHaveBeenCalledOnce();
    await user.click(screen.getByRole('button', { name: 'Check workspace preparation' })); await entryReady(); expect(api.workspacesPrepareTaskGit).toHaveBeenCalledOnce(); await user.click(prepare()); await waitFor(() => expect(workspace()).toHaveValue(f.own.id)); noExecution();
  });
  it('uses the original component’s lost-reply reconciliation and same-key recovery before unlocking', async () => {
    const user = userEvent.setup(), f = fixture(), state = setup(f); let created = 0;
    vi.mocked(api.workspacesPrepareTaskGit).mockImplementationOnce(async (selection, key) => { ++created; state.commit(selection, key); throw new Error('actual response deliberately lost by mocked transport'); }).mockImplementationOnce(async (selection, key) => f.view(selection, key));
    render(element(f)); await entryReady(); await user.click(prepare()); await screen.findByText(/Request outcome unknown/); const original = vi.mocked(api.workspacesPrepareTaskGit).mock.calls[0];
    expect(grant()).toBeDisabled(); expect(session()).toBeDisabled(); expect(workspace()).toHaveValue('');
    await user.click(screen.getByRole('button', { name: 'Check workspace preparation' })); const recover = screen.getByRole('button', { name: 'Recover the exact preparation' }); await waitFor(() => expect(recover).toBeEnabled());
    expect(api.workspacesPrepareTaskGit).toHaveBeenCalledOnce(); expect(workspace()).toHaveValue(''); await user.click(recover); await waitFor(() => expect(workspace()).toHaveValue(f.own.id)); expect(created).toBe(1); expect(vi.mocked(api.workspacesPrepareTaskGit).mock.calls[1]).toEqual(original); noExecution();
  });
  it.each(['ready', 'pending', 'outcome_unknown'] as const)('keeps original recovery through parent Refresh when the lost preparation is stored %s', async stored => {
    const user = userEvent.setup(), f = fixture(), state = setup(f);
    vi.mocked(api.workspacesPrepareTaskGit).mockImplementationOnce(async (selection, key) => {
      const view = state.commit(selection, key);
      if (stored !== 'ready') {
        state.server.task = { ...f.task, revision: 2, code_workspace_binding: { ...view.binding, state: stored, revision: 1, managed_pin: null } };
        state.server.rows = [f.other, { ...f.own, state: stored === 'pending' ? 'creating' : 'outcome_unknown', identity: null }];
      }
      throw new Error('preparation reply lost');
    }).mockImplementationOnce(async (selection, key) => {
      if (stored !== 'ready') throw new DaemonError('OUTCOME_UNKNOWN', 'Original preparation remains unresolved', 'unknown');
      return f.view(selection, key);
    });
    render(element(f)); await entryReady(); await user.selectOptions(screen.getByLabelText('Named connection'), f.connection.id); await user.click(prepare()); await screen.findByText(/Request outcome unknown/);
    const original = vi.mocked(api.workspacesPrepareTaskGit).mock.calls[0];
    // Deliberately use the outer refresh, not the component's own Check button.
    await user.click(refresh()); const recover = await screen.findByRole('button', { name: 'Recover the exact preparation' }); await waitFor(() => expect(recover).toBeEnabled());
    expect(screen.getByLabelText('I authorize this exact recipient, workspace and bounded execution scope.')).toBeDisabled(); expect(grant()).toBeDisabled(); expect(session()).toBeDisabled(); expect(api.workspacesPrepareTaskGit).toHaveBeenCalledOnce();
    await user.click(recover); await waitFor(() => expect(api.workspacesPrepareTaskGit).toHaveBeenCalledTimes(2)); expect(vi.mocked(api.workspacesPrepareTaskGit).mock.calls[1]).toEqual(original);
    if (stored === 'ready') await waitFor(() => expect(workspace()).toHaveValue(f.own.id));
    else { await screen.findByText(/Original preparation remains unresolved/); expect(grant()).toBeDisabled(); expect(session()).toBeDisabled(); expect(screen.getByRole('button', { name: 'Recover the exact preparation' })).toBeDisabled(); }
    noExecution();
  });
  it('blocks arbitrary workspace and grants while the initial resolver is pending', async () => {
    const f = fixture(); setup(f); const gate = deferred<CodeWorkspacePreparation>(); vi.mocked(api.workspacesResolve).mockReturnValueOnce(gate.promise); render(element(f));
    await waitFor(() => expect(api.workspacesResolve).toHaveBeenCalled()); expect(workspace()).toBeDisabled(); expect(grant()).toBeDisabled(); expect(session()).toBeDisabled(); expect(screen.queryByTestId('session-task-workspace-entry')).toBeNull();
    await act(async () => { gate.resolve(f.preparation); }); await entryReady(); noExecution();
  });
  it('shows a resolver refusal and allows an explicit read retry without a fallback workspace', async () => {
    const user = userEvent.setup(), f = fixture(); setup(f); vi.mocked(api.workspacesResolve).mockRejectedValueOnce(new DaemonError('CAPABILITY_UNSUPPORTED', 'Unsupported source or host', 'not_started')).mockResolvedValue(f.preparation); render(element(f));
    expect(await screen.findByRole('alert')).toHaveTextContent('Unsupported source or host'); expect(workspace()).toBeDisabled(); expect(grant()).toBeDisabled(); expect(api.workspacesPrepareTaskGit).not.toHaveBeenCalled();
    await user.click(refresh()); await entryReady(); noExecution();
  });
  it('keeps directory/data explicit workspace selection and never mounts the Git preparation entry', async () => {
    const user = userEvent.setup(), f = fixture(); f.project.identity.kind = { kind: 'directory' }; setup(f); vi.mocked(api.workspacesResolve).mockResolvedValue({ ...f.preparation, source: { ...f.preparation.source!, kind: { kind: 'directory' } }, source_head: null, preparation_key: null, state: 'needs_dataset_selection', reason: 'selected_dataset_required' }); render(element(f));
    await waitFor(() => expect(workspace()).toBeEnabled()); expect(screen.queryByTestId('task-code-workspace')).toBeNull(); await user.selectOptions(workspace(), f.other.id); expect(workspace()).toHaveValue(f.other.id); expect(api.workspacesPrepareTaskGit).not.toHaveBeenCalled(); noExecution();
  });
  it('does not use a personal placeholder as a Git source, host or implicit copy', async () => {
    const f = fixture(); f.task = { ...f.task, project_id: 'local' }; setup(f); vi.mocked(api.projectsList).mockResolvedValue([]); vi.mocked(api.workspacesList).mockResolvedValue([]); vi.mocked(api.workspacesResolve).mockResolvedValue({ ...f.preparation, project_id: 'local', project_revision: null, source: null, source_head: null, preparation_key: null, state: 'needs_registered_project', reason: 'registered_project_required' }); render(element(f, { projects: [], projectId: 'local' }));
    await waitFor(() => expect(api.workspacesResolve).toHaveBeenCalledWith(expect.objectContaining({ expected_project_revision: 0, host_id: 'local' }))); expect(screen.queryByTestId('task-code-workspace')).toBeNull(); expect(grant()).toBeDisabled(); expect(api.workspacesPrepareTaskGit).not.toHaveBeenCalled(); noExecution();
  });
  it.each(['cancelled', 'outcome_unknown', 'accepted'])('does not prepare a new workspace for historical %s tasks', async state => {
    const f = fixture(); f.task = { ...f.task, state, ...(state === 'accepted' ? { acceptance_ref: 'acceptance:actual-reference', acceptance_sequence: 1 } : {}) }; setup(f); render(element(f));
    await waitFor(() => expect(api.taskGet).toHaveBeenCalledWith(f.task.id)); await waitFor(() => expect(workspace()).toBeEnabled()); expect(api.workspacesResolve).not.toHaveBeenCalled(); expect(screen.queryByTestId('task-code-workspace')).toBeNull(); expect(session()).toBeDisabled(); noExecution();
  });
  it('preserves global manual Grant scope when no Task is selected', async () => {
    const user = userEvent.setup(), f = fixture(); setup(f); render(element(f, { initialTaskId: undefined })); await waitFor(() => expect(workspace()).toBeEnabled()); await user.selectOptions(workspace(), f.other.id);
    expect(workspace()).toHaveValue(f.other.id); expect(api.taskGet).not.toHaveBeenCalled(); expect(api.workspacesResolve).not.toHaveBeenCalled(); expect(screen.queryByTestId('task-code-workspace')).toBeNull(); noExecution();
  });
  it('rejects an old TaskA preparation callback after parent A-B-A selection epochs', async () => {
    const user = userEvent.setup(), a = fixture(), b = fixture(); b.task = { ...b.task, project_id: a.project.id }; b.project = a.project;
    const state = setup(a); const directory: CodeWorkspacePreparation = { ...b.preparation, task_id: b.task.id, project_id: a.project.id, source: { ...b.preparation.source!, kind: { kind: 'directory' } }, source_head: null, preparation_key: null, state: 'needs_dataset_selection', reason: 'selected_dataset_required' };
    vi.mocked(api.taskGet).mockImplementation(async id => id === a.task.id ? state.server.task : b.task); vi.mocked(api.workspacesResolve).mockImplementation(async selected => selected.task_id === a.task.id ? a.preparation : directory);
    const page = render(element(a, { tasks: [a.task, b.task] })); await entryReady(); const old = notifications.filter(row => row.taskId === a.task.id).at(-1)!.ready!;
    page.rerender(element(a, { initialTaskId: b.task.id, tasks: [a.task, b.task] })); await waitFor(() => expect(workspace()).toBeEnabled()); await user.selectOptions(workspace(), a.other.id);
    page.rerender(element(a, { initialTaskId: a.task.id, tasks: [a.task, b.task] })); await entryReady(); const gets = vi.mocked(api.taskGet).mock.calls.length, lists = vi.mocked(api.workspacesList).mock.calls.length;
    await act(async () => { old(a.task.id); }); expect(vi.mocked(api.taskGet).mock.calls.length).toBe(gets); expect(vi.mocked(api.workspacesList).mock.calls.length).toBe(lists); expect(workspace()).toBeDisabled(); noExecution();
  });
  it('does not apply a completed old Task preparation after the user selects another Task', async () => {
    const user = userEvent.setup(), a = fixture(), b = fixture(); b.task = { ...b.task, project_id: a.project.id }; b.project = a.project; const state = setup(a), gate = deferred<TaskWorkspaceBindingView>();
    vi.mocked(api.taskGet).mockImplementation(async id => id === a.task.id ? state.server.task : b.task); vi.mocked(api.workspacesResolve).mockImplementation(async selection => selection.task_id === a.task.id ? a.preparation : { ...a.preparation, task_id: b.task.id, source: { ...a.preparation.source!, kind: { kind: 'directory' } }, source_head: null, preparation_key: null, state: 'needs_dataset_selection', reason: 'selected_dataset_required' });
    vi.mocked(api.workspacesPrepareTaskGit).mockReturnValue(gate.promise); render(element(a, { tasks: [a.task, b.task] })); await entryReady(); await user.click(prepare()); const original = vi.mocked(api.workspacesPrepareTaskGit).mock.calls[0];
    await user.selectOptions(screen.getByLabelText('Task binding'), b.task.id); await waitFor(() => expect(workspace()).toBeEnabled()); await user.selectOptions(workspace(), a.other.id);
    await act(async () => { gate.resolve(state.commit(...original)); }); expect(workspace()).toHaveValue(a.other.id); expect(screen.getByLabelText('Task binding')).toHaveValue(b.task.id); noExecution();
  });
  it('keeps short Chinese preparation copy in the reused entry', async () => {
    const f = fixture(); setup(f); render(element(f, { language: 'zh' })); expect(await screen.findByText('执行授权前，请先准备此任务独立的代码工作区。')).toBeVisible(); expect(await screen.findByRole('button', { name: '为此任务准备隔离 Git 工作区' })).toBeVisible(); noExecution();
  });
});
