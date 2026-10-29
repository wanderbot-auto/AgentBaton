import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TaskCodeWorkspace } from './TaskCodeWorkspace';
import { api, DaemonError, type Project, type Task } from './api';
import type { CodeWorkspacePreparation } from './workspace-preparation-wire';
import type { TaskWorkspaceBindingView } from './task-workspace-binding-wire';
vi.mock('./api', async original => { const actual = await original<typeof import('./api')>(); return { ...actual, api: { ...actual.api, taskGet: vi.fn(), projectsList: vi.fn(), workspacesResolve: vi.fn(), workspacesPrepareTaskGit: vi.fn() } }; });
let serial = 0;
const date = '2026-10-29T00:00:00Z';
function fixture() {
  const id = `code-task-${++serial}`;
  const task: Task = { id, project_id: 'project', goal: 'Prepare isolated code', state: 'draft', revision: 1, spec_revision: 2, created_at: date, updated_at: date };
  const project: Project = { id: 'project', name: 'Selected repository', identity: { canonical_root: '/source', device: 1, inode: 2, kind: { kind: 'git', common_dir: '/source/.git', head: 'a'.repeat(40), dirty: true } }, revision: 3, archived: false, created_at: date, updated_at: date };
  const pin = { canonical_root: '/source', device: '9007199254740993', inode: '18446744073709551615', common_dir: '/source/.git', head: 'a'.repeat(40) };
  const preparation: CodeWorkspacePreparation = { task_id: id, task_revision: 1, spec_revision: 2, project_id: 'project', project_revision: 3, host_id: 'local', source: { canonical_root: pin.canonical_root, device: pin.device, inode: pin.inode, kind: { kind: 'git', common_dir: pin.common_dir, head: pin.head, dirty: true } }, source_head: pin.head, preparation_key: 'code-prep:' + 'b'.repeat(64), state: 'needs_git_workspace', reason: 'isolated_git_workspace_required', candidates: [], execution_authorized: false, creates_workspace: false };
  function view(key: string): TaskWorkspaceBindingView {
    const receipt = { version: 1 as const, binding_id: `binding-${serial}`, workspace_id: `workspace-${serial}`, task_id: id, project_id: 'project', host_id: 'local' as const, original_task_revision: 1, reserved_task_revision: 2, spec_revision: 2, project_revision: 3, source_pin: structuredClone(pin), managed_root: '/managed', preparation_key: preparation.preparation_key!, request_key: key, request_digest: 'c'.repeat(64), created_at: date };
    const managed = { ...pin, canonical_root: `/managed/${receipt.workspace_id}`, inode: '9007199254740995' };
    return { original_receipt: receipt, binding: { receipt: structuredClone(receipt), state: 'ready', revision: 2, managed_pin: managed }, task_revision: 3, observed_managed_pin: { ...managed, head: 'd'.repeat(40) }, identity_revalidated: true, execution_authorized: false };
  }
  return { task, project, pin, preparation, view };
}
function setup(f: ReturnType<typeof fixture>) {
  vi.mocked(api.taskGet).mockResolvedValue(f.task);
  vi.mocked(api.projectsList).mockResolvedValue([f.project]);
  vi.mocked(api.workspacesResolve).mockResolvedValue(f.preparation);
}
const prepare = () => screen.getByRole('button', { name: 'Prepare isolated Git workspace for this task' });
const refresh = () => screen.getByRole('button', { name: 'Check workspace preparation' });
const recover = () => screen.getByRole('button', { name: 'Recover the exact preparation' });
async function expand(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByText('Task code workspace', { selector: 'summary' }));
  await waitFor(() => expect(refresh()).toBeEnabled());
}
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; }
beforeEach(() => vi.resetAllMocks());

describe('Task code workspace component / mocked fixed daemon transport only', () => {
  it('is folded and performs no reads or writes on task-detail mount', () => {
    const f = fixture(); render(<TaskCodeWorkspace task={f.task} language="en" connected />);
    expect(screen.getByTestId('task-code-workspace')).not.toHaveAttribute('open');
    expect(api.taskGet).not.toHaveBeenCalled(); expect(api.workspacesResolve).not.toHaveBeenCalled(); expect(api.workspacesPrepareTaskGit).not.toHaveBeenCalled();
  });
  it('resolves exact task/project/local CAS only on expansion and preserves opaque source identity', async () => {
    const user = userEvent.setup(), f = fixture(); setup(f); render(<TaskCodeWorkspace task={f.task} language="en" connected />); await expand(user);
    expect(api.workspacesResolve).toHaveBeenCalledExactlyOnceWith({ task_id: f.task.id, expected_task_revision: 1, expected_project_revision: 3, host_id: 'local' });
    expect(screen.getByText('/source')).toBeVisible(); expect(screen.getByText(/uncommitted changes/)).toBeVisible(); expect(prepare()).toBeEnabled(); expect(api.workspacesPrepareTaskGit).not.toHaveBeenCalled();
  });
  it('prepares only after explicit action with full original source pin, stable key and task/spec/project CAS', async () => {
    const user = userEvent.setup(), f = fixture(); setup(f);
    vi.mocked(api.workspacesPrepareTaskGit).mockImplementation(async (_, key) => { const view = f.view(key); vi.mocked(api.taskGet).mockResolvedValue({ ...f.task, revision: 3, code_workspace_binding: view.binding }); return view; });
    render(<TaskCodeWorkspace task={f.task} language="en" connected />); await expand(user); await user.click(prepare());
    await waitFor(() => expect(screen.getByText('Ready binding recorded')).toBeVisible());
    expect(api.workspacesPrepareTaskGit).toHaveBeenCalledExactlyOnceWith({ task_id: f.task.id, expected_task_revision: 1, expected_spec_revision: 2, expected_project_revision: 3, host_id: 'local', expected_source_pin: f.pin, preparation_key: f.preparation.preparation_key }, expect.any(String));
    expect(screen.getByText('Filesystem identity revalidated')).toBeVisible(); expect(screen.getByText(/not execution authorization/)).toBeVisible();
    expect(screen.queryByRole('button', { name: 'Prepare isolated Git workspace for this task' })).not.toBeInTheDocument();
    await user.click(screen.getByText('Binding and receipt details', { selector: 'summary' }));
    expect(screen.getByText('/managed/' + f.view('x').binding.receipt.workspace_id)).toBeVisible(); expect(screen.getByText('Original Task revision')).toBeVisible(); expect(screen.getByText('Current Task revision')).toBeVisible();
  });
  it('retains lost request across remount and requires a successful explicit task read before exact same-key replay', async () => {
    const user = userEvent.setup(), f = fixture(); setup(f); vi.mocked(api.workspacesPrepareTaskGit).mockRejectedValue(new Error('reply lost'));
    let page = render(<TaskCodeWorkspace task={f.task} language="en" connected />); await expand(user); await user.click(prepare()); await screen.findByText(/Request outcome unknown/);
    const original = vi.mocked(api.workspacesPrepareTaskGit).mock.calls[0]; expect(recover()).toBeDisabled(); page.unmount();
    vi.mocked(api.taskGet).mockRejectedValueOnce(new Error('read unavailable'));
    page = render(<TaskCodeWorkspace task={f.task} language="en" connected />); await expand(user); expect(recover()).toBeDisabled(); expect(api.workspacesPrepareTaskGit).toHaveBeenCalledOnce();
    await user.click(refresh()); await waitFor(() => expect(recover()).toBeEnabled());
    vi.mocked(api.workspacesPrepareTaskGit).mockImplementation(async (_, key) => { const view = f.view(key); vi.mocked(api.taskGet).mockResolvedValue({ ...f.task, revision: 3, code_workspace_binding: view.binding }); return view; });
    await user.click(recover()); await screen.findByText('Ready binding recorded');
    expect(vi.mocked(api.workspacesPrepareTaskGit).mock.calls[1]).toEqual(original); expect(api.workspacesPrepareTaskGit).toHaveBeenCalledTimes(2);
  });
  it('reads a committed lost reply without treating a stored ready binding as fresh filesystem evidence', async () => {
    const user = userEvent.setup(), f = fixture(); setup(f);
    vi.mocked(api.workspacesPrepareTaskGit).mockImplementation(async (_, key) => { const view = f.view(key); vi.mocked(api.taskGet).mockResolvedValue({ ...f.task, revision: 3, code_workspace_binding: view.binding }); throw new Error('lost'); });
    render(<TaskCodeWorkspace task={f.task} language="en" connected />); await expand(user); await user.click(prepare()); await screen.findByText(/Request outcome unknown/); await user.click(refresh());
    expect(screen.getByText('Ready binding recorded')).toBeVisible(); expect(screen.getByText(/Stored binding only/)).toBeVisible(); expect(recover()).toBeEnabled(); expect(api.workspacesPrepareTaskGit).toHaveBeenCalledOnce();
  });
  it('keeps prior uncertainty when exact-key recovery returns a not-started busy error', async () => {
    const user = userEvent.setup(), f = fixture(); setup(f);
    vi.mocked(api.workspacesPrepareTaskGit).mockRejectedValueOnce(new Error('lost')).mockRejectedValueOnce(new DaemonError('WORKSPACE_BUSY', 'busy', 'not_started'));
    render(<TaskCodeWorkspace task={f.task} language="en" connected />); await expand(user); await user.click(prepare()); await screen.findByText(/Request outcome unknown/); await user.click(refresh()); await waitFor(() => expect(recover()).toBeEnabled()); await user.click(recover());
    await waitFor(() => expect(api.workspacesPrepareTaskGit).toHaveBeenCalledTimes(2)); expect(recover()).toBeDisabled(); expect(prepare()).toBeDisabled(); expect(vi.mocked(api.workspacesPrepareTaskGit).mock.calls[1]).toEqual(vi.mocked(api.workspacesPrepareTaskGit).mock.calls[0]);
  });
  it.each(['pending', 'outcome_unknown'] as const)('shows persisted %s honestly without creating or claiming ready', async state => {
    const user = userEvent.setup(), f = fixture(), view = f.view('old-key'); setup(f);
    vi.mocked(api.taskGet).mockResolvedValue({ ...f.task, revision: 2, code_workspace_binding: { ...view.binding, state, revision: 1, managed_pin: null } });
    render(<TaskCodeWorkspace task={f.task} language="en" connected />); await expand(user);
    expect(screen.queryByText('Ready binding recorded')).toBeNull(); expect(screen.queryByRole('button', { name: /Prepare isolated/ })).toBeNull(); expect(api.workspacesPrepareTaskGit).not.toHaveBeenCalled(); expect(api.workspacesResolve).not.toHaveBeenCalled();
  });
  it('revalidates a previously stored binding through its original receipt/key rather than current CAS', async () => {
    const user = userEvent.setup(), f = fixture(), view = f.view('original-key'); setup(f);
    vi.mocked(api.taskGet).mockResolvedValue({ ...f.task, revision: 3, code_workspace_binding: view.binding }); vi.mocked(api.workspacesPrepareTaskGit).mockResolvedValue(view);
    render(<TaskCodeWorkspace task={f.task} language="en" connected />); await expand(user); await user.click(screen.getByRole('button', { name: 'Revalidate the original binding' })); await screen.findByText('Filesystem identity revalidated');
    expect(api.workspacesPrepareTaskGit).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ expected_task_revision: 1, expected_spec_revision: 2, expected_project_revision: 3 }), 'original-key');
  });
  it('does not silently promote existing project workspace candidates into task assignments', async () => {
    const user = userEvent.setup(), f = fixture(); setup(f); vi.mocked(api.workspacesResolve).mockResolvedValue({ ...f.preparation, candidates: [{ workspace_id: 'candidate', revision: 1, root: '/candidate', recorded_state: 'ready', has_unreconciled_lease: true, assigned_to_task: false, identity_revalidated: false }] });
    render(<TaskCodeWorkspace task={f.task} language="en" connected />); await expand(user); await user.click(screen.getByText('Existing project workspaces', { selector: 'summary' }));
    expect(screen.getByText(/not assignments to this task/)).toBeVisible(); expect(screen.getByText(/Unreconciled lease recorded/)).toBeVisible(); expect(screen.queryByText('Ready binding recorded')).toBeNull(); expect(api.workspacesPrepareTaskGit).not.toHaveBeenCalled();
  });
  it('does not copy or initialize a directory project and keeps selected-file guidance', async () => {
    const user = userEvent.setup(), f = fixture(); setup(f);
    vi.mocked(api.workspacesResolve).mockResolvedValue({ ...f.preparation, state: 'needs_dataset_selection', reason: 'selected_dataset_required', source: { ...f.preparation.source!, kind: { kind: 'directory' } }, source_head: null, preparation_key: null });
    render(<TaskCodeWorkspace task={f.task} language="en" connected />); await expand(user); expect(prepare()).toBeDisabled(); expect(screen.getByText(/no implicit copy or Git initialization/)).toBeVisible(); expect(api.workspacesPrepareTaskGit).not.toHaveBeenCalled();
  });
  it('requires a registered project instead of treating local as a source path or host', async () => {
    const user = userEvent.setup(), f = fixture(); f.task = { ...f.task, project_id: 'local' }; setup(f); vi.mocked(api.projectsList).mockResolvedValue([]);
    vi.mocked(api.workspacesResolve).mockResolvedValue({ ...f.preparation, project_id: 'local', project_revision: null, source: null, source_head: null, preparation_key: null, state: 'needs_registered_project', reason: 'registered_project_required' });
    render(<TaskCodeWorkspace task={f.task} language="en" connected />); await expand(user); expect(prepare()).toBeDisabled(); expect(screen.getByText(/personal task has no code source/)).toBeVisible();
  });
  it('keeps archived and non-draft records readable with new preparation disabled', async () => {
    const user = userEvent.setup(), f = fixture(); f.task = { ...f.task, state: 'cancelled' }; f.project.archived = true; setup(f); vi.mocked(api.workspacesResolve).mockResolvedValue({ ...f.preparation, state: 'blocked', reason: 'task_not_draft', preparation_key: null });
    render(<TaskCodeWorkspace task={f.task} language="en" connected />); await expand(user); expect(prepare()).toBeDisabled(); expect(screen.getByText(/project is archived/)).toBeVisible(); expect(api.workspacesResolve).not.toHaveBeenCalled(); expect(api.workspacesPrepareTaskGit).not.toHaveBeenCalled();
  });
  it('shows the actual blocked resolver result for an active project with an unknown task, without a prepare action', async () => {
    const user = userEvent.setup(), f = fixture(); f.task = { ...f.task, state: 'outcome_unknown' }; setup(f);
    vi.mocked(api.workspacesResolve).mockResolvedValue({ ...f.preparation, state: 'blocked', reason: 'task_not_draft', preparation_key: null });
    render(<TaskCodeWorkspace task={f.task} language="en" connected />); await expand(user);
    expect(screen.getByText(/Only an unbound draft task/)).toBeVisible(); expect(prepare()).toBeDisabled(); expect(api.workspacesPrepareTaskGit).not.toHaveBeenCalled();
  });
  it('refuses disconnected actions and ignores a slow read after task identity changes', async () => {
    const user = userEvent.setup(), f = fixture(), next = fixture(); setup(f); const gate = deferred<Task>(); vi.mocked(api.taskGet).mockReturnValueOnce(gate.promise);
    const page = render(<TaskCodeWorkspace task={f.task} language="en" connected />); await user.click(screen.getByText('Task code workspace', { selector: 'summary' }));
    page.rerender(<TaskCodeWorkspace task={next.task} language="en" connected={false} />); await act(async () => { gate.resolve(f.task); });
    expect(api.workspacesPrepareTaskGit).not.toHaveBeenCalled(); expect(refresh()).toBeDisabled(); expect(screen.queryByText('Ready binding recorded')).toBeNull();
  });
  it.each(['foreign-task', 'spec', 'authority', 'host'])('refuses %s drift in readonly preparation', async variant => {
    const user = userEvent.setup(), f = fixture(); setup(f);
    const raw = { ...f.preparation, ...(variant === 'foreign-task' ? { task_id: 'foreign' } : variant === 'spec' ? { spec_revision: 3 } : variant === 'authority' ? { execution_authorized: true } : { host_id: 'remote' }) };
    vi.mocked(api.workspacesResolve).mockResolvedValue(raw as CodeWorkspacePreparation);
    render(<TaskCodeWorkspace task={f.task} language="en" connected />); await expand(user); expect(await screen.findByRole('alert')).toBeVisible(); expect(api.workspacesPrepareTaskGit).not.toHaveBeenCalled();
  });
  it('preserves outcome unknown when the result falsely grants execution authority', async () => {
    const user = userEvent.setup(), f = fixture(); setup(f); vi.mocked(api.workspacesPrepareTaskGit).mockImplementation(async (_, key) => ({ ...f.view(key), execution_authorized: true } as unknown as TaskWorkspaceBindingView));
    render(<TaskCodeWorkspace task={f.task} language="en" connected />); await expand(user); await user.click(prepare()); await screen.findByText(/Request outcome unknown/); expect(prepare()).toBeDisabled(); expect(screen.queryByText('Filesystem identity revalidated')).toBeNull();
  });
  it('does not reuse a changed current task CAS/spec after a lost result', async () => {
    const user = userEvent.setup(), f = fixture(); setup(f); vi.mocked(api.workspacesPrepareTaskGit).mockRejectedValue(new Error('lost')); const page = render(<TaskCodeWorkspace task={f.task} language="en" connected />); await expand(user); await user.click(prepare()); await screen.findByText(/Request outcome unknown/);
    const changed = { ...f.task, revision: 2, spec_revision: 3 }; vi.mocked(api.taskGet).mockResolvedValue(changed); vi.mocked(api.workspacesResolve).mockResolvedValue({ ...f.preparation, task_revision: 2, spec_revision: 3 }); page.rerender(<TaskCodeWorkspace task={changed} language="en" connected />);
    await waitFor(() => expect(recover()).toBeEnabled()); await user.click(recover()); await waitFor(() => expect(api.workspacesPrepareTaskGit).toHaveBeenCalledTimes(2));
    expect(vi.mocked(api.workspacesPrepareTaskGit).mock.calls[1]).toEqual(vi.mocked(api.workspacesPrepareTaskGit).mock.calls[0]);
  });
  it('rejects rollback and a swapped immutable receipt on later readonly refresh', async () => {
    const user = userEvent.setup(), f = fixture(), view = f.view('key'); setup(f); vi.mocked(api.taskGet).mockResolvedValue({ ...f.task, revision: 3, code_workspace_binding: view.binding });
    render(<TaskCodeWorkspace task={f.task} language="en" connected />); await expand(user);
    const swapped = structuredClone(view.binding); swapped.receipt.request_key = 'foreign-key'; vi.mocked(api.taskGet).mockResolvedValue({ ...f.task, revision: 4, code_workspace_binding: swapped }); await user.click(refresh()); expect(await screen.findByRole('alert')).toHaveTextContent('does not match');
    expect(screen.getByRole('button', { name: 'Revalidate the original binding' })).toBeDisabled(); expect(api.workspacesPrepareTaskGit).not.toHaveBeenCalled();
    vi.mocked(api.taskGet).mockResolvedValue({ ...f.task, revision: 2, code_workspace_binding: view.binding }); await user.click(refresh());
    expect(await screen.findByRole('alert')).toHaveTextContent('does not match'); expect(api.workspacesPrepareTaskGit).not.toHaveBeenCalled();
  });
  it('does not reconcile an unknown preparation against a foreign project with matching numeric revisions', async () => {
    const user = userEvent.setup(), f = fixture(); setup(f); vi.mocked(api.workspacesPrepareTaskGit).mockRejectedValue(new Error('lost'));
    const page = render(<TaskCodeWorkspace task={f.task} language="en" connected />); await expand(user); await user.click(prepare()); await screen.findByText(/Request outcome unknown/);
    const foreign = { ...f.task, project_id: 'foreign', revision: 2 };
    vi.mocked(api.taskGet).mockResolvedValue(foreign); vi.mocked(api.projectsList).mockResolvedValue([{ ...f.project, id: 'foreign' }]);
    vi.mocked(api.workspacesResolve).mockResolvedValue({ ...f.preparation, task_revision: 2, project_id: 'foreign' });
    page.rerender(<TaskCodeWorkspace task={foreign} language="en" connected />);
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('does not match'));
    expect(recover()).toBeDisabled(); expect(api.workspacesPrepareTaskGit).toHaveBeenCalledOnce();
  });
  it('uses Chinese task-scoped labels and keyboard activation without an implicit write', async () => {
    const user = userEvent.setup(), f = fixture(); setup(f); render(<TaskCodeWorkspace task={f.task} language="zh" connected />); const summary = screen.getByText('任务代码工作区', { selector: 'summary' }); summary.focus(); await user.keyboard('{Enter}');
    // jsdom need not synthesize native details keyboard behavior; explicit toggle is a DOM event, not a backend success.
    if (!screen.getByTestId('task-code-workspace').hasAttribute('open')) { screen.getByTestId('task-code-workspace').setAttribute('open', ''); fireEvent(screen.getByTestId('task-code-workspace'), new Event('toggle')); }
    await waitFor(() => expect(screen.getByRole('button', { name: '检查工作区准备状态' })).toBeEnabled()); expect(screen.getByRole('button', { name: '为此任务准备隔离 Git 工作区' })).toBeEnabled(); expect(api.workspacesPrepareTaskGit).not.toHaveBeenCalled();
  });
});
