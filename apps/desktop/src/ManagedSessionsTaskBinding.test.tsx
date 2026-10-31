import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { api, DaemonError, type ManagedWorkspace, type Project, type Task } from './api';
import { ManagedSessions, initialTaskSelectionCurrent } from './ManagedSessions';
import { sessionApi, type ExecutionGrant, type NamedConnection } from './session-api';
import type { TaskWorkspaceBinding } from './task-workspace-binding-wire';
vi.mock('./api', async original => ({ ...await original<typeof import('./api')>(), api: { taskGet: vi.fn(), workspacesResolve: vi.fn(), workspacesList: vi.fn(), snapshot: vi.fn(), events: vi.fn() } }));
vi.mock('./session-api', () => ({ sessionApi: { connectionsList: vi.fn(), sessionsList: vi.fn(), grantsList: vi.fn(), grantsCreate: vi.fn(), grantsRevoke: vi.fn(), sessionsCreate: vi.fn(), sessionsGet: vi.fn(), sessionsSend: vi.fn(), sessionsCancel: vi.fn() } }));
const date = '2026-10-29T00:00:00Z';
let serial = 0;
function fixture() {
  const id = ++serial;
  const project: Project = { id: `project-${id}`, name: 'Selected project', revision: 3, archived: false, created_at: date, updated_at: date, identity: { canonical_root: '/source', device: 1, inode: 2, kind: { kind: 'git', head: 'a'.repeat(40), common_dir: '/source/.git', dirty: false } } };
  const pin = { canonical_root: '/source', device: '9007199254740993', inode: '18446744073709551615', common_dir: '/source/.git', head: 'a'.repeat(40) };
  const binding: TaskWorkspaceBinding = { state: 'ready', revision: 2, receipt: { version: 1, binding_id: `binding-${id}`, workspace_id: `workspace-${id}`, task_id: `task-${id}`, project_id: project.id, host_id: 'local', original_task_revision: 1, reserved_task_revision: 2, spec_revision: 1, project_revision: 3, source_pin: pin, managed_root: '/managed', preparation_key: 'code-prep:' + 'b'.repeat(64), request_key: 'original-key', request_digest: 'c'.repeat(64), created_at: date }, managed_pin: { ...pin, canonical_root: `/managed/workspace-${id}`, inode: '9007199254740995' } };
  const task: Task = { id: `task-${id}`, project_id: project.id, goal: 'Task with recorded binding', state: 'draft', revision: 3, spec_revision: 1, code_workspace_binding: binding, created_at: date, updated_at: date };
  const workspace: ManagedWorkspace = { id: binding.receipt.workspace_id, project_id: project.id, root: binding.managed_pin!.canonical_root, revision: 1, state: 'ready', identity: { ...project.identity, canonical_root: binding.managed_pin!.canonical_root }, base_revision: pin.head, created_at: date };
  const other = { ...workspace, id: `other-${id}`, root: `/managed/other-${id}` };
  const connection: NamedConnection = { id: `connection-${id}`, name: 'Explicit connection', provider: 'custom_acp', executable: '/selected/program', arguments: [], executable_sha256: 'd'.repeat(64), environment_id: 'env', identity_ref: 'unknown:identity', config_revision: 'config', host_id: 'local', approval_id: 'approval', probe_state: 'ready', revision: 2, capability_snapshot_id: 'caps', identity_status: 'unknown', config_status: 'private_environment_not_sandbox', version_evidence: null, created_at: date };
  const grant: ExecutionGrant = { id: `grant-${id}`, project_id: project.id, root_grant_id: `grant-${id}`, parent_id: null, depth: 0, revision: 1, approval_id: 'native-owner', approved_at: date, expires_at_ms: Date.now() + 600000, revoked: false, scope: { workspace_ids: [workspace.id], connection_ids: [connection.id], host_ids: ['local'], max_attempts: 1, max_concurrency: 1, max_duration_ms: 60000, recipients: [], network: { mode: 'inherited' }, max_depth: 0, max_children: 0 }, enforcement: { baton_mediated_scope: true, os_filesystem_sandbox: false, os_network_sandbox: false, native_identity_isolation: false } };
  return { task, project, binding, workspace, other, connection, grant };
}
const workspacePicker = () => screen.getByLabelText('Managed workspace');
const taskPicker = () => screen.getByLabelText('Task binding');
const grantPicker = () => screen.getByLabelText('Execution grant');
const ack = () => screen.getByLabelText('I authorize this exact recipient, workspace and bounded execution scope.');
const create = () => screen.getByRole('button', { name: 'Create managed session' });
const authorize = () => screen.getByRole('button', { name: 'Create bounded execution grant' });
const refresh = () => screen.getByRole('button', { name: 'Refresh session records' });
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; }
function setup(f: ReturnType<typeof fixture>) {
  vi.mocked(api.taskGet).mockImplementation(async id => { if (id !== f.task.id) throw new Error('Unknown test Task'); return f.task; });
  vi.mocked(api.workspacesResolve).mockImplementation(async selection => {
    if (selection.task_id !== f.task.id || selection.expected_task_revision !== f.task.revision || selection.expected_project_revision !== f.project.revision || f.project.identity.kind.kind !== 'directory') throw new Error('Expected explicit directory fixture, not a coding bypass');
    return { task_id: f.task.id, task_revision: f.task.revision, spec_revision: f.task.spec_revision!, project_id: f.project.id, project_revision: f.project.revision, host_id: 'local', source: { canonical_root: '/source', device: '1', inode: '2', kind: { kind: 'directory' } }, source_head: null, preparation_key: null, state: 'needs_dataset_selection', reason: 'selected_dataset_required', candidates: [], execution_authorized: false, creates_workspace: false };
  });
  vi.mocked(api.workspacesList).mockResolvedValue([f.other, f.workspace]);
  vi.mocked(api.snapshot).mockResolvedValue({ tasks: [f.task], sequence: 0, api_version: 1 });
  vi.mocked(api.events).mockResolvedValue({ events: [], next_cursor: 0, sequence: 0, gap: false });
  vi.mocked(sessionApi.connectionsList).mockResolvedValue([f.connection]); vi.mocked(sessionApi.sessionsList).mockResolvedValue([]); vi.mocked(sessionApi.grantsList).mockResolvedValue([f.grant]);
}
function element(f: ReturnType<typeof fixture>, overrides: Partial<React.ComponentProps<typeof ManagedSessions>> = {}) {
  return <ManagedSessions language="en" connected projects={[f.project]} tasks={[f.task]} projectId={f.project.id} initialTaskId={f.task.id} onProjectChange={vi.fn()} refreshTasks={vi.fn().mockResolvedValue(undefined)} {...overrides} />;
}
async function ready(f: ReturnType<typeof fixture>) { await waitFor(() => expect(workspacePicker()).toHaveValue(f.workspace.id)); }
async function chooseConnection(user: ReturnType<typeof userEvent.setup>, f: ReturnType<typeof fixture>) { await user.selectOptions(screen.getByLabelText('Named connection'), f.connection.id); }
beforeEach(() => vi.resetAllMocks());

describe('Task-bound session selection / mocked fixed read and native transport; not FS or Provider proof', () => {
  it('uses the actual synchronous applied/rendered epoch predicate before effects, including A-B-A', () => {
    const appliedA = JSON.stringify([0, ['project', 'task-A']]), renderedB = JSON.stringify([1, ['project', 'task-B']]), returnedA = JSON.stringify([2, ['project', 'task-A']]);
    expect(initialTaskSelectionCurrent(appliedA, renderedB)).toBe(false);
    expect(initialTaskSelectionCurrent(appliedA, returnedA)).toBe(false);
    expect(initialTaskSelectionCurrent(renderedB, renderedB)).toBe(true);
    // A later manual task choice does not change the parent initial epoch.
    expect(initialTaskSelectionCurrent(appliedA, appliedA)).toBe(true);
  });
  it('invalidates old acknowledged scope on parent initialTaskId change and waits for B Get before permitting later manual choices', async () => {
    const user = userEvent.setup(), a = fixture(), b0 = fixture(); const b = { ...b0, project: a.project, workspace: { ...b0.workspace, project_id: a.project.id }, task: { ...b0.task, project_id: a.project.id, code_workspace_binding: { ...b0.binding, receipt: { ...b0.binding.receipt, project_id: a.project.id } } } };
    setup(a); vi.mocked(api.workspacesList).mockResolvedValue([a.workspace, b.workspace]); const delayedB = deferred<Task>();
    vi.mocked(api.taskGet).mockImplementation(id => id === a.task.id ? Promise.resolve(a.task) : delayedB.promise);
    const page = render(element(a, { tasks: [a.task, b.task] })); await ready(a); await chooseConnection(user, a); await user.click(ack()); await user.selectOptions(grantPicker(), a.grant.id); expect(create()).toBeEnabled();
    const before = vi.mocked(api.taskGet).mock.calls.filter(([id]) => id === a.task.id).length;
    page.rerender(element(a, { initialTaskId: b.task.id, tasks: [a.task, b.task] }));
    expect(ack()).not.toBeChecked(); expect(grantPicker()).toHaveValue(''); expect(authorize()).toBeDisabled(); expect(create()).toBeDisabled();
    await waitFor(() => expect(api.taskGet).toHaveBeenCalledWith(b.task.id));
    expect(vi.mocked(api.taskGet).mock.calls.filter(([id]) => id === a.task.id)).toHaveLength(before);
    expect(create()).toBeDisabled(); expect(sessionApi.sessionsCreate).not.toHaveBeenCalled();
    await act(async () => { delayedB.resolve(b.task); }); await waitFor(() => expect(workspacePicker()).toHaveValue(b.workspace.id)); expect(grantPicker()).toHaveValue(''); expect(ack()).not.toBeChecked();
    await user.selectOptions(taskPicker(), a.task.id); await ready(a); expect(taskPicker()).toHaveValue(a.task.id); expect(sessionApi.grantsCreate).not.toHaveBeenCalled(); expect(sessionApi.sessionsCreate).not.toHaveBeenCalled();
  });
  it('locates only the ready bound workspace despite list order and performs no implicit write', async () => {
    const f = fixture(); setup(f); render(element(f)); await ready(f);
    expect(api.taskGet).toHaveBeenCalledWith(f.task.id); expect(workspacePicker()).toBeDisabled();
    expect(screen.getByText(/Stored binding is not fresh filesystem evidence/)).toBeVisible();
    expect(grantPicker()).toHaveValue(''); expect(ack()).not.toBeChecked(); expect(sessionApi.grantsCreate).not.toHaveBeenCalled(); expect(sessionApi.sessionsCreate).not.toHaveBeenCalled(); expect(sessionApi.sessionsSend).not.toHaveBeenCalled();
  });
  it('reads the latest persisted binding when the parent still has a pre-preparation None/revision1 Task', async () => {
    const f = fixture(); setup(f); render(element(f, { tasks: [{ ...f.task, revision: 1, code_workspace_binding: null }] })); await ready(f);
    expect(api.taskGet).toHaveBeenCalledWith(f.task.id); expect(create()).toBeDisabled(); expect(sessionApi.sessionsCreate).not.toHaveBeenCalled();
  });
  it('requires explicit connection and existing grant selection before an exact Task-CAS session call', async () => {
    const user = userEvent.setup(), f = fixture(); setup(f); render(element(f)); await ready(f); await chooseConnection(user, f);
    expect(create()).toBeDisabled(); await user.selectOptions(grantPicker(), f.grant.id); expect(create()).toBeEnabled();
    vi.mocked(sessionApi.sessionsCreate).mockRejectedValue(new DaemonError('INPUT_CHANGED', 'Core fresh admission refused', 'not_started'));
    await user.click(create()); expect(sessionApi.sessionsCreate).toHaveBeenCalledExactlyOnceWith(f.task.id, 3, f.workspace.id, f.connection.id, f.grant.id, expect.any(String));
    expect(await screen.findByRole('alert')).toHaveTextContent('Core fresh admission refused'); expect(sessionApi.grantsCreate).not.toHaveBeenCalled(); expect(sessionApi.sessionsSend).not.toHaveBeenCalled();
  });
  it.each(['pending', 'outcome_unknown', 'input_changed'])('never falls back from %s binding to another ready workspace', async state => {
    const f = fixture(); f.task = { ...f.task, code_workspace_binding: { ...f.binding, state, managed_pin: state === 'pending' ? null : f.binding.managed_pin } as TaskWorkspaceBinding }; setup(f); render(element(f));
    await waitFor(() => expect(api.taskGet).toHaveBeenCalledWith(f.task.id)); await screen.findByText(/binding is pending, unknown, stale or unavailable/);
    expect(workspacePicker()).toHaveValue(''); expect(workspacePicker()).toBeDisabled(); expect(authorize()).toBeDisabled(); expect(create()).toBeDisabled();
  });
  it.each(['missing', 'creating', 'outcome_unknown', 'foreign-project', 'wrong-root', 'wrong-head', 'duplicate'])('rejects %s workspace records for the bound Task', async variant => {
    const f = fixture(); setup(f); let records = [f.other, f.workspace];
    if (variant === 'missing') records = [f.other];
    if (variant === 'creating' || variant === 'outcome_unknown') records = [f.other, { ...f.workspace, state: variant }];
    if (variant === 'foreign-project') records = [f.other, { ...f.workspace, project_id: 'foreign' }];
    if (variant === 'wrong-root') records = [f.other, { ...f.workspace, root: '/foreign' }];
    if (variant === 'wrong-head') records = [f.other, { ...f.workspace, base_revision: 'e'.repeat(40) }];
    if (variant === 'duplicate') records = [f.workspace, { ...f.workspace }];
    vi.mocked(api.workspacesList).mockResolvedValue(records); render(element(f));
    await screen.findByText(/binding is pending, unknown, stale or unavailable/); expect(workspacePicker()).toHaveValue(''); expect(authorize()).toBeDisabled(); expect(create()).toBeDisabled();
  });
  it.each(['spec', 'project-revision', 'foreign-receipt', 'host'])('refuses %s mismatch without changing the server contract', async variant => {
    const f = fixture(), changed = structuredClone(f.binding); setup(f);
    if (variant === 'spec') changed.receipt.spec_revision = 2;
    if (variant === 'project-revision') changed.receipt.project_revision = 2;
    if (variant === 'foreign-receipt') changed.receipt.project_id = 'foreign';
    if (variant === 'host') (changed.receipt as unknown as { host_id: string }).host_id = 'remote';
    f.task = { ...f.task, code_workspace_binding: changed }; render(element(f));
    await waitFor(() => expect(api.taskGet).toHaveBeenCalled()); expect(authorize()).toBeDisabled(); expect(create()).toBeDisabled(); expect(sessionApi.grantsCreate).not.toHaveBeenCalled();
  });
  it('clears TaskA workspace, acknowledgement and grant on TaskB selection and rejects late TaskA reads', async () => {
    const user = userEvent.setup(), a = fixture(), b0 = fixture(); const b = { ...b0, project: a.project, workspace: { ...b0.workspace, project_id: a.project.id }, task: { ...b0.task, project_id: a.project.id, code_workspace_binding: { ...b0.binding, receipt: { ...b0.binding.receipt, project_id: a.project.id } } } };
    setup(a); vi.mocked(api.workspacesList).mockResolvedValue([a.workspace, b.workspace]); const late = deferred<Task>();
    vi.mocked(api.taskGet).mockImplementation(async id => id === a.task.id ? a.task : b.task);
    const page = render(element(a, { tasks: [a.task, b.task] })); await ready(a); await chooseConnection(user, a); await user.click(ack()); await user.selectOptions(grantPicker(), a.grant.id); expect(ack()).toBeChecked();
    vi.mocked(api.taskGet).mockImplementation(id => id === a.task.id ? late.promise : Promise.resolve(b.task)); await user.click(refresh());
    await user.selectOptions(taskPicker(), b.task.id); await waitFor(() => expect(workspacePicker()).toHaveValue(b.workspace.id)); expect(ack()).not.toBeChecked(); expect(grantPicker()).toHaveValue('');
    await act(async () => { late.resolve(a.task); }); expect(workspacePicker()).toHaveValue(b.workspace.id); expect(create()).toBeDisabled(); page.unmount();
  });
  it('ignores late old-project workspace and Task responses after a project change', async () => {
    const a = fixture(), b = fixture(); setup(a); const oldWorkspaces = deferred<ManagedWorkspace[]>(), oldTask = deferred<Task>();
    vi.mocked(api.workspacesList).mockImplementation(id => id === a.project.id ? oldWorkspaces.promise : Promise.resolve([b.workspace]));
    vi.mocked(api.taskGet).mockImplementation(id => id === a.task.id ? oldTask.promise : Promise.resolve(b.task));
    const page = render(element(a)); page.rerender(element(b)); await ready(b);
    await act(async () => { oldWorkspaces.resolve([a.workspace]); oldTask.resolve(a.task); }); expect(workspacePicker()).toHaveValue(b.workspace.id); expect(screen.queryByRole('option', { name: a.workspace.root + ' (ready)' })).toBeNull(); expect(sessionApi.sessionsCreate).not.toHaveBeenCalled();
  });
  it('does not install a delayed native Grant result into another Task’s acknowledgement scope', async () => {
    const user = userEvent.setup(), a = fixture(), b0 = fixture(); const b = { ...b0, project: a.project, workspace: { ...b0.workspace, project_id: a.project.id }, task: { ...b0.task, project_id: a.project.id, code_workspace_binding: { ...b0.binding, receipt: { ...b0.binding.receipt, project_id: a.project.id } } } };
    setup(a); vi.mocked(api.workspacesList).mockResolvedValue([a.workspace, b.workspace]); vi.mocked(api.taskGet).mockImplementation(async id => id === a.task.id ? a.task : b.task); const gate = deferred<ExecutionGrant>(); vi.mocked(sessionApi.grantsCreate).mockReturnValue(gate.promise);
    const page = render(element(a, { tasks: [a.task, b.task] })); await ready(a); await chooseConnection(user, a); await user.click(ack()); await user.click(authorize());
    page.rerender(element(a, { initialTaskId: b.task.id, tasks: [a.task, b.task] })); await waitFor(() => expect(workspacePicker()).toHaveValue(b.workspace.id));
    await act(async () => { gate.resolve(a.grant); }); expect(grantPicker()).toHaveValue(''); expect(ack()).not.toBeChecked(); expect(sessionApi.sessionsCreate).not.toHaveBeenCalled();
  });
  it('clears acknowledgement and grant when the same Task binding becomes unknown', async () => {
    const user = userEvent.setup(), f = fixture(); setup(f); const page = render(element(f)); await ready(f); await chooseConnection(user, f); await user.click(ack()); await user.selectOptions(grantPicker(), f.grant.id);
    f.task = { ...f.task, revision: 4, code_workspace_binding: { ...f.binding, revision: 3, state: 'outcome_unknown' } }; page.rerender(element(f));
    await waitFor(() => expect(workspacePicker()).toHaveValue('')); expect(ack()).not.toBeChecked(); expect(grantPicker()).toHaveValue(''); expect(authorize()).toBeDisabled(); expect(create()).toBeDisabled();
  });
  it('blocks a failed or regressing current Task read instead of using a previously valid binding', async () => {
    const user = userEvent.setup(), f = fixture(); setup(f); render(element(f)); await ready(f); await chooseConnection(user, f);
    vi.mocked(api.taskGet).mockRejectedValueOnce(new Error('current Task read failed')); await user.click(refresh()); await screen.findByRole('alert'); expect(authorize()).toBeDisabled();
    vi.mocked(api.taskGet).mockResolvedValue({ ...f.task, revision: 2 }); await user.click(refresh()); await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('Refresh records after changing')); expect(create()).toBeDisabled();
  });
  it('keeps an unbound legacy Task workspace explicitly user-selected across unrelated readonly refresh', async () => {
    const user = userEvent.setup(), f = fixture(); f.task = { ...f.task, code_workspace_binding: null }; f.project.identity = { ...f.project.identity, kind: { kind: 'directory' } }; setup(f); render(element(f));
    await waitFor(() => expect(workspacePicker()).toBeEnabled()); expect(workspacePicker()).toHaveValue(''); await user.selectOptions(workspacePicker(), f.other.id); await user.click(refresh());
    await waitFor(() => expect(workspacePicker()).toBeEnabled()); expect(workspacePicker()).toHaveValue(f.other.id); expect(sessionApi.grantsCreate).not.toHaveBeenCalled();
  });
  it('preserves global manual Grant creation with no Task selected and still requires explicit native action', async () => {
    const user = userEvent.setup(), f = fixture(); setup(f); render(element(f, { initialTaskId: undefined })); await waitFor(() => expect(workspacePicker()).toBeEnabled());
    await user.selectOptions(workspacePicker(), f.other.id); await chooseConnection(user, f); expect(authorize()).toBeDisabled(); await user.click(ack());
    vi.mocked(sessionApi.grantsCreate).mockRejectedValue(new DaemonError('PERMISSION_DENIED', 'Native declined', 'not_started')); await user.click(authorize());
    expect(sessionApi.grantsCreate).toHaveBeenCalledExactlyOnceWith({ project_id: f.project.id, workspace_id: f.other.id, connection_id: f.connection.id, max_attempts: 1, max_concurrency: 1, max_duration_ms: 60000 }, expect.any(String)); expect(create()).toBeDisabled(); expect(api.taskGet).not.toHaveBeenCalled();
  });
  it('does not replace a user’s manual workspace after a same-Task unbound revision refresh', async () => {
    const user = userEvent.setup(), f = fixture(); f.task = { ...f.task, code_workspace_binding: null }; f.project.identity = { ...f.project.identity, kind: { kind: 'directory' } }; setup(f); const page = render(element(f)); await waitFor(() => expect(workspacePicker()).toBeEnabled()); await user.selectOptions(workspacePicker(), f.other.id);
    f.task = { ...f.task, revision: 2 }; page.rerender(element(f)); await waitFor(() => expect(api.taskGet).toHaveBeenCalledTimes(2)); expect(workspacePicker()).toHaveValue(f.other.id); expect(sessionApi.sessionsCreate).not.toHaveBeenCalled();
  });
  it('shows the Chinese convenience-only boundary without granting permission on selection', async () => {
    const f = fixture(); setup(f); render(element(f, { language: 'zh' })); await waitFor(() => expect(screen.getByLabelText('托管工作区')).toHaveValue(f.workspace.id)); expect(screen.getByText(/存储的绑定不构成当前文件系统证据/)).toBeVisible(); expect(sessionApi.grantsCreate).not.toHaveBeenCalled(); expect(sessionApi.sessionsCreate).not.toHaveBeenCalled();
  });
});
