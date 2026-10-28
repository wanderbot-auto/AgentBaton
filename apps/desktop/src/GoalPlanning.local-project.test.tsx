import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { GoalPlanning } from './GoalPlanning';
import { DaemonError, type Task } from './api';
import { goalApi as api } from './goal-api';
import type { ExecutionGrant } from './session-api';

vi.mock('./goal-api', async original => ({
  ...await original<typeof import('./goal-api')>(),
  goalApi: { grantsList: vi.fn(), scopesList: vi.fn(), scopeGet: vi.fn(), approve: vi.fn(), revoke: vi.fn(), propose: vi.fn(), revise: vi.fn(), planGet: vi.fn(), plansList: vi.fn(), preview: vi.fn() },
}));

let serial = 0;
const guidance = {
  en: 'Goal planning needs a registered project and an approved root grant. Create a task in a registered project and prepare its root grant first; this personal task has no project execution scope.',
  zh: 'Goal 规划需要已注册项目及已批准的根授权。请先在已注册项目中创建任务并准备其根授权；此个人任务没有项目执行范围。',
};
function task(projectId = 'local'): Task & { project_id: string } {
  return { id: `personal-readiness-${++serial}`, project_id: projectId, goal: 'Prepare a report', state: 'draft', revision: 1, spec_revision: 1, created_at: '2026-10-27T00:00:00Z', updated_at: '2026-10-27T00:00:00Z' };
}
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(api.grantsList).mockResolvedValue([]);
  vi.mocked(api.scopesList).mockResolvedValue([]);
  vi.mocked(api.plansList).mockResolvedValue([]);
});
const writes = () => [api.approve, api.revoke, api.propose, api.revise, api.preview];
async function open(user: ReturnType<typeof userEvent.setup>, language: 'en' | 'zh' = 'en') {
  await user.click(screen.getByRole('heading', { name: language === 'en' ? 'Goal scope and plan' : 'Goal 范围与计划' }));
}

describe('reserved personal project Goal prerequisites, mocked transport', () => {
  it.each(['en', 'zh'] as const)('explains the personal prerequisite in %s without impossible reads or Goal actions', async language => {
    const user = userEvent.setup();
    vi.mocked(api.grantsList).mockRejectedValue(new DaemonError('NOT_FOUND', 'Project does not exist', 'not_started'));
    render(<GoalPlanning task={task()} language={language} connected />);
    expect(screen.getByTestId('goal-planning').querySelector('details')).not.toHaveAttribute('open');
    await open(user, language);
    expect(screen.getByText(guidance[language])).toBeVisible();
    expect(screen.queryByRole('alert')).toBeNull();
    expect(screen.getByTestId('goal-planning').querySelector('form')).toBeNull();
    expect(screen.queryAllByRole('button')).toHaveLength(0);
    for (const method of Object.values(api)) expect(method).not.toHaveBeenCalled();
  });

  it('keeps personal prerequisites non-failing across connection changes and reopening', async () => {
    const user = userEvent.setup(), personal = task();
    const view = render(<GoalPlanning task={personal} language="en" connected={false} />);
    await open(user);
    expect(screen.getByText(guidance.en)).toBeVisible();
    view.rerender(<GoalPlanning task={personal} language="en" connected />);
    await open(user); await open(user);
    expect(screen.getByText(guidance.en)).toBeVisible();
    expect(screen.queryByRole('alert')).toBeNull();
    for (const method of Object.values(api)) expect(method).not.toHaveBeenCalled();
  });

  it.each([
    new DaemonError('NOT_FOUND', 'Registered project no longer exists', 'not_started'),
    new Error('Registered project connection unavailable'),
  ])('preserves genuine registered-project read failure: %s', async failure => {
    const user = userEvent.setup(), registered = task('local-registered-project');
    vi.mocked(api.grantsList).mockRejectedValue(failure);
    render(<GoalPlanning task={registered} language="en" connected />);
    await open(user);
    expect(await screen.findByRole('alert')).toHaveTextContent(failure.message);
    expect(api.grantsList).toHaveBeenCalledWith(registered.project_id);
    expect(api.scopesList).toHaveBeenCalledWith(registered.id);
    expect(screen.queryByText(guidance.en)).toBeNull();
    expect(screen.getByRole('button', { name: 'Review Goal scope in native dialog' })).toBeDisabled();
    for (const method of writes()) expect(method).not.toHaveBeenCalled();
  });

  it('keeps a successful empty registered-project read distinct from personal readiness', async () => {
    const user = userEvent.setup(), registered = task('registered-project');
    render(<GoalPlanning task={registered} language="en" connected />);
    await open(user);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Refresh Goal records' })).toBeEnabled());
    expect(screen.getByText('A live existing local root grant is required. Register and approve its exact execution scope first.')).toBeVisible();
    expect(screen.queryByText(guidance.en)).toBeNull();
    expect(screen.queryByRole('alert')).toBeNull();
    expect(api.grantsList).toHaveBeenCalledWith(registered.project_id);
    expect(api.scopesList).toHaveBeenCalledWith(registered.id);
    for (const method of writes()) expect(method).not.toHaveBeenCalled();
  });

  it('preserves an exact registered pending approval across navigation through a personal task', async () => {
    const user = userEvent.setup(), registered = task('registered-recovery-project'), personal = task();
    const grant: ExecutionGrant = { id: 'registered-recovery-grant', project_id: registered.project_id, root_grant_id: 'registered-recovery-grant', parent_id: null, depth: 0, revision: 1, approval_id: 'native', approved_at: registered.created_at, expires_at_ms: Date.now() + 60000, revoked: false, scope: { workspace_ids: ['workspace'], connection_ids: ['connection'], host_ids: ['local'], max_attempts: 8, max_concurrency: 2, max_duration_ms: 60000, max_children: 0, max_depth: 0, recipients: [], network: { mode: 'inherited' } }, enforcement: { baton_mediated_scope: true, os_filesystem_sandbox: false, os_network_sandbox: false, native_identity_isolation: false } };
    vi.mocked(api.grantsList).mockResolvedValue([grant]);
    vi.mocked(api.approve).mockRejectedValueOnce(new Error('Approval response lost')).mockRejectedValueOnce(new DaemonError('PERMISSION_DENIED', 'No new approval was finalized', 'not_started'));
    let view = render(<GoalPlanning task={registered} language="en" connected />);
    await open(user);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Refresh Goal records' })).toBeEnabled());
    await user.selectOptions(screen.getByLabelText('Existing root grant'), grant.id);
    fireEvent.change(screen.getByLabelText('Output relative path 1'), { target: { value: 'outputs/report.txt' } });
    fireEvent.change(screen.getByLabelText('Writable relative prefixes (one per line)'), { target: { value: 'outputs' } });
    fireEvent.change(screen.getByLabelText('Owner acceptance criteria'), { target: { value: 'Review exact report' } });
    await user.click(screen.getByRole('button', { name: 'Review Goal scope in native dialog' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Request outcome unknown');
    const original = vi.mocked(api.approve).mock.calls[0], readCount = vi.mocked(api.grantsList).mock.calls.length;
    view.unmount();
    view = render(<GoalPlanning task={personal} language="en" connected />);
    await open(user);
    expect(screen.getByText(guidance.en)).toBeVisible();
    expect(api.grantsList).toHaveBeenCalledTimes(readCount);
    expect(api.approve).toHaveBeenCalledOnce();
    view.unmount();
    render(<GoalPlanning task={registered} language="en" connected />);
    await open(user);
    const recover = await screen.findByRole('button', { name: 'Review the same Goal request' });
    await waitFor(() => expect(recover).toBeEnabled());
    expect(api.approve).toHaveBeenCalledOnce();
    await user.click(recover);
    await waitFor(() => expect(api.approve).toHaveBeenCalledTimes(2));
    expect(vi.mocked(api.approve).mock.calls[1]).toEqual(original);
    for (const method of [api.propose, api.revise, api.preview, api.revoke]) expect(method).not.toHaveBeenCalled();
  });
});
