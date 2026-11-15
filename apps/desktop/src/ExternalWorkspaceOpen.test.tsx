import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Task } from './api';
import { ExternalWorkspaceOpen } from './ExternalWorkspaceOpen';
import { workspaceOpenApi } from './workspace-open-api';
import { workspaceOpenMessages as messages } from './workspace-open-i18n';
import type { WorkspaceOpenResult } from './workspace-open-wire';
vi.mock('./workspace-open-api', () => ({ workspaceOpenApi: { choices: vi.fn(), open: vi.fn() } }));
let serial = 0;
const project = '11111111-1111-4111-8111-111111111111', isolated = '22222222-2222-4222-8222-222222222222';
function fixture() {
  const task: Task = { id: `open-task-${++serial}`, project_id: project, revision: 3, spec_revision: 2, state: 'running', goal: 'Existing task', created_at: 'now', updated_at: 'now' };
  const choices = [{ label: 'source' as const, recordedPath: '/source dirty', selection: { kind: 'project_source' as const, project_id: project, expected_project_revision: 4 } }, { label: 'isolated' as const, recordedPath: '/managed/目录', selection: { kind: 'managed_workspace' as const, project_id: project, expected_project_revision: 4, workspace_id: isolated, expected_workspace_revision: 5 } }];
  vi.mocked(workspaceOpenApi.choices).mockResolvedValue(choices); vi.mocked(workspaceOpenApi.open).mockResolvedValue({ status: 'handoff_accepted', code: 'accepted' });
  return { task, choices };
}
function deferred<T>() { let resolve!: (v: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; }
const button = () => screen.getByRole('button', { name: messages.en.open });
const refresh = () => screen.getByRole('button', { name: messages.en.refresh });
async function choose(f: ReturnType<typeof fixture>, which = 0) { const select = await screen.findByRole('combobox', { name: messages.en.target }); await waitFor(() => expect(select).toBeEnabled()); await userEvent.selectOptions(select, JSON.stringify(f.choices[which].selection)); }
beforeEach(() => vi.resetAllMocks());
describe('Directory handoff component / mocked Native, never actual Terminal evidence', () => {
  it('does nothing while folded, disconnected, blocked or without a registered project', () => {
    const f = fixture(); const view = render(<ExternalWorkspaceOpen task={f.task} language="en" connected open={false} />);
    expect(workspaceOpenApi.choices).not.toHaveBeenCalled(); expect(workspaceOpenApi.open).not.toHaveBeenCalled(); expect(button()).toBeDisabled();
    view.rerender(<ExternalWorkspaceOpen task={f.task} language="en" connected={false} open />);
    view.rerender(<ExternalWorkspaceOpen task={f.task} language="en" connected open blocked />);
    view.rerender(<ExternalWorkspaceOpen task={{ ...f.task, project_id: 'local' }} language="en" connected open />);
    expect(workspaceOpenApi.choices).not.toHaveBeenCalled(); expect(button()).toBeDisabled();
  });
  it('requires an explicit source/isolated choice and sends only exact selection plus terminal enum', async () => {
    const f = fixture(); render(<ExternalWorkspaceOpen task={f.task} language="en" connected open />);
    await waitFor(() => expect(refresh()).toBeEnabled()); expect(button()).toBeDisabled(); expect(workspaceOpenApi.open).not.toHaveBeenCalled();
    await choose(f, 1); await userEvent.click(button()); await screen.findByText(messages.en.accepted);
    expect(workspaceOpenApi.open).toHaveBeenCalledExactlyOnceWith({ selection: f.choices[1].selection, terminal: 'macos_terminal' }); expect(button()).toBeDisabled(); expect(screen.getByText(messages.en.boundary)).toBeVisible();
  });
  it('opens the dirty source explicitly without changing task/session/approval state', async () => {
    const f = fixture(); render(<ExternalWorkspaceOpen task={f.task} language="en" connected open />); await choose(f); await userEvent.click(button()); await screen.findByText(messages.en.accepted);
    expect(workspaceOpenApi.open).toHaveBeenCalledExactlyOnceWith({ selection: f.choices[0].selection, terminal: 'macos_terminal' }); expect(f.task.state).toBe('running');
  });
  it('keeps one in-flight call, disables target switching and never repeats automatically', async () => {
    const f = fixture(), pending = deferred<WorkspaceOpenResult>(); vi.mocked(workspaceOpenApi.open).mockReturnValue(pending.promise);
    render(<ExternalWorkspaceOpen task={f.task} language="en" connected open />); await choose(f); fireEvent.click(button()); fireEvent.click(button());
    expect(workspaceOpenApi.open).toHaveBeenCalledTimes(1); expect(await screen.findByText(messages.en.pending)).toBeVisible(); expect(screen.getByRole('combobox', { name: messages.en.target })).toBeDisabled(); expect(refresh()).toBeDisabled();
    await act(async () => pending.resolve({ status: 'outcome_unknown', code: 'opener_timeout' })); expect(button()).toBeDisabled(); expect(workspaceOpenApi.open).toHaveBeenCalledTimes(1);
  });
  it('keeps unknown visible and needs a manual fresh read plus another explicit click', async () => {
    const f = fixture(); vi.mocked(workspaceOpenApi.open).mockRejectedValueOnce(new Error('private transport')).mockResolvedValueOnce({ status: 'handoff_accepted', code: 'accepted' });
    render(<ExternalWorkspaceOpen task={f.task} language="en" connected open />); await choose(f); await userEvent.click(button()); await screen.findByText(messages.en.unknown);
    expect(button()).toBeDisabled(); expect(screen.queryByText('private transport')).toBeNull(); expect(workspaceOpenApi.open).toHaveBeenCalledTimes(1);
    await userEvent.click(refresh()); await waitFor(() => expect(button()).toBeEnabled()); expect(screen.getByText(messages.en.unknown)).toBeVisible(); expect(workspaceOpenApi.open).toHaveBeenCalledTimes(1);
    await userEvent.click(button()); await screen.findByText(messages.en.accepted); expect(workspaceOpenApi.open).toHaveBeenCalledTimes(2);
  });
  it('does not erase earlier handoff uncertainty when a later explicit attempt is refused', async () => {
    const f = fixture(); vi.mocked(workspaceOpenApi.open).mockRejectedValueOnce(new Error('lost')).mockResolvedValueOnce({ status: 'refused', code: 'stale_context' });
    render(<ExternalWorkspaceOpen task={f.task} language="en" connected open />); await choose(f); await userEvent.click(button()); await screen.findByText(messages.en.unknown);
    await userEvent.click(refresh()); await waitFor(() => expect(button()).toBeEnabled()); await userEvent.click(button());
    await screen.findByRole('alert'); expect(screen.getByText(messages.en.unknown)).toBeVisible(); expect(button()).toBeDisabled(); expect(workspaceOpenApi.open).toHaveBeenCalledTimes(2);
  });
  it.each(['disconnect', 'task', 'project', 'revision', 'close', 'blocked'] as const)('rejects an old accepted reply after %s', async change => {
    const f = fixture(), pending = deferred<WorkspaceOpenResult>(); vi.mocked(workspaceOpenApi.open).mockReturnValue(pending.promise);
    const view = render(<ExternalWorkspaceOpen task={f.task} language="en" connected open />); await choose(f); await userEvent.click(button());
    const task = change === 'task' ? { ...f.task, id: f.task.id + '-changed' } : change === 'project' ? { ...f.task, project_id: isolated } : change === 'revision' ? { ...f.task, revision: 4 } : f.task;
    view.rerender(<ExternalWorkspaceOpen task={task} language="en" connected={change !== 'disconnect'} open={change !== 'close'} blocked={change === 'blocked'} />);
    await act(async () => pending.resolve({ status: 'handoff_accepted', code: 'accepted' })); expect(screen.queryByText(messages.en.accepted)).toBeNull(); expect(workspaceOpenApi.open).toHaveBeenCalledTimes(1);
  });
  it('rejects old directory choices after parent changes and does not silently choose the old target', async () => {
    const f = fixture(), pending = deferred<typeof f.choices>(); vi.mocked(workspaceOpenApi.choices).mockReturnValueOnce(pending.promise).mockResolvedValueOnce(f.choices);
    const view = render(<ExternalWorkspaceOpen task={f.task} language="en" connected open />); view.rerender(<ExternalWorkspaceOpen task={{ ...f.task, revision: 4 }} language="en" connected open />);
    await act(async () => pending.resolve(f.choices)); await waitFor(() => expect(refresh()).toBeEnabled()); expect(button()).toBeDisabled(); expect(workspaceOpenApi.open).not.toHaveBeenCalled();
  });
  it('retains uncertainty after unmount and refuses an automatic remount retry', async () => {
    const f = fixture(), pending = deferred<WorkspaceOpenResult>(); vi.mocked(workspaceOpenApi.open).mockReturnValue(pending.promise);
    const view = render(<ExternalWorkspaceOpen task={f.task} language="en" connected open />); await choose(f); await userEvent.click(button()); view.unmount();
    await act(async () => pending.resolve({ status: 'handoff_accepted', code: 'accepted' }));
    render(<ExternalWorkspaceOpen task={f.task} language="en" connected open />); await choose(f); expect(screen.getByText(messages.en.unknown)).toBeVisible(); expect(button()).toBeDisabled(); expect(workspaceOpenApi.open).toHaveBeenCalledTimes(1);
  });
  it('renders exact finite refusal and Chinese labels without claiming terminal cwd or a session', async () => {
    const f = fixture(); vi.mocked(workspaceOpenApi.open).mockResolvedValue({ status: 'refused', code: 'unsupported_platform' });
    render(<ExternalWorkspaceOpen task={f.task} language="zh" connected open />);
    const select = screen.getByRole('combobox', { name: messages.zh.target }); await waitFor(() => expect(select).toBeEnabled()); await userEvent.selectOptions(select, JSON.stringify(f.choices[0].selection));
    await userEvent.click(screen.getByRole('button', { name: messages.zh.open })); expect(await screen.findByRole('alert')).toHaveTextContent(messages.zh.unsupported_platform); expect(screen.queryByText(messages.zh.accepted)).toBeNull();
  });
  it('keeps read failures closed and permits only explicit refresh', async () => {
    const f = fixture(); vi.mocked(workspaceOpenApi.choices).mockRejectedValueOnce(new Error('private path'));
    render(<ExternalWorkspaceOpen task={f.task} language="en" connected open />); await screen.findByText(messages.en.readFailed); expect(button()).toBeDisabled(); expect(screen.queryByText('private path')).toBeNull(); expect(workspaceOpenApi.choices).toHaveBeenCalledTimes(1);
    await userEvent.click(refresh()); await choose(f); expect(button()).toBeEnabled(); expect(workspaceOpenApi.open).not.toHaveBeenCalled();
  });
});
