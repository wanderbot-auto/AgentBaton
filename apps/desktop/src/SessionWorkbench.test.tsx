import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { api, DaemonError, type Project, type ManagedWorkspace, type Task } from './api';
import { Connections } from './Connections';
import { ManagedSessions, mergeSessions } from './ManagedSessions';
import { sessionApi, type ExecutionGrant, type NamedConnection, type ManagedSession } from './session-api';

// Only the browser/component transport is mocked. These tests never execute a
// selected program, copy credentials or call a vendor model.
vi.mock('./api', async original => ({ ...await original<typeof import('./api')>(), api: { workspacesList: vi.fn(), snapshot: vi.fn(), events: vi.fn() } }));
vi.mock('./session-api', () => ({ sessionApi: { connectionsList: vi.fn(), connectionsRegister: vi.fn(), grantsCreate: vi.fn(), grantsList: vi.fn(), grantsRevoke: vi.fn(), sessionsCreate: vi.fn(), sessionsList: vi.fn(), sessionsGet: vi.fn(), sessionsSend: vi.fn(), sessionsCancel: vi.fn() } }));
const created = '2026-10-27T00:00:00Z';
const project: Project = { id: 'project-1', name: 'Project', revision: 1, archived: false, created_at: created, updated_at: created, identity: { canonical_root: '/source', device: 1, inode: 2, kind: { kind: 'directory' } } };
const workspace: ManagedWorkspace = { id: 'workspace-1', project_id: project.id, root: '/managed/workspace', revision: 1, state: 'ready', identity: project.identity, base_revision: 'a'.repeat(40), created_at: created };
const task: Task = { id: 'task-1', project_id: project.id, goal: 'Selected task', revision: 1, state: 'draft', created_at: created, updated_at: created };
const connection: NamedConnection = { id: 'connection-1', name: 'Selected local agent', provider: 'custom_acp', executable: '/selected/program', arguments: ['--acp'], executable_sha256: 'b'.repeat(64), environment_id: 'env-1', identity_ref: 'identity-unverified', config_revision: 'config-1', host_id: 'local', approval_id: 'probe-approval', probe_state: 'ready', revision: 2, capability_snapshot_id: 'cap-1', identity_status: 'unknown', config_status: 'private_environment_not_sandbox', created_at: created, version_evidence: { source: 'selected_executable_version_probe', executable_sha256: 'b'.repeat(64), observed_at_unix_ms: 1793059200000, state: 'succeeded', version_line: 'Selected program 1.0', exit_code: 0, stdout_bytes: 20, stderr_bytes: 0, process_reaped: true } };
const grant: ExecutionGrant = { id: 'grant-1', project_id: project.id, root_grant_id: 'grant-1', parent_id: null, depth: 0, revision: 1, approval_id: 'approval-1', approved_at: created, expires_at_ms: Date.now() + 600_000, revoked: false, scope: { workspace_ids: [workspace.id], connection_ids: [connection.id], host_ids: ['local'], max_attempts: 1, max_concurrency: 1, max_duration_ms: 60_000, recipients: [], network: { mode: 'deny' }, max_depth: 0, max_children: 0 }, enforcement: { baton_mediated_scope: true, os_filesystem_sandbox: false, os_network_sandbox: false, native_identity_isolation: false } };
const session: ManagedSession = { id: 'session-1', task_id: task.id, project_id: project.id, workspace_id: workspace.id, connection_id: connection.id, grant_id: grant.id, state: 'ready', revision: 2, native_session_id: 'native-1', created_at: created, updated_at: created, last_error: null, turn_state: null };
const refreshTasks = vi.fn().mockResolvedValue(undefined);
beforeEach(() => {
  vi.resetAllMocks();
  refreshTasks.mockResolvedValue(undefined);
  vi.mocked(api.workspacesList).mockResolvedValue([workspace]);
  vi.mocked(api.snapshot).mockResolvedValue({ tasks: [task], api_version: 1, sequence: 0 });
  vi.mocked(api.events).mockResolvedValue({ events: [], next_cursor: 0, sequence: 0, gap: false });
  vi.mocked(sessionApi.connectionsList).mockResolvedValue([connection]);
  vi.mocked(sessionApi.grantsList).mockResolvedValue([grant]);
  vi.mocked(sessionApi.sessionsList).mockResolvedValue([session]);
  vi.mocked(sessionApi.sessionsGet).mockResolvedValue(session);
});
function showSessions() {
  return render(<ManagedSessions language="en" connected projects={[project]} tasks={[task]} projectId={project.id} onProjectChange={vi.fn()} refreshTasks={refreshTasks} />);
}
async function selectSession(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole('button', { name: /Selected local agent.*Ready.*session-1/ }));
  await waitFor(() => expect(screen.getByLabelText('Message to this connection')).toBeEnabled());
}
describe('Named connection and managed-session UI using mocked transport', () => {
  it('preserves the selection and does not register again when native owner confirmation is declined', async () => {
    const user = userEvent.setup();
    vi.mocked(sessionApi.connectionsRegister).mockRejectedValue(new DaemonError('PERMISSION_DENIED', 'Native owner confirmation was declined', 'not_started'));
    render(<Connections language="en" connected />);
    await user.type(screen.getByLabelText('Connection name'), 'Chosen');
    await user.type(screen.getByLabelText('Canonical executable path'), '/explicit/program');
    await user.click(screen.getByRole('button', { name: 'Register and run version probe' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Native owner confirmation was declined');
    expect(screen.getByLabelText('Canonical executable path')).toHaveValue('/explicit/program');
    expect(sessionApi.connectionsRegister).toHaveBeenCalledTimes(1);
  });
  it('explicitly registers only the user-selected program and keeps probe/account evidence distinct', async () => {
    const user = userEvent.setup();
    vi.mocked(sessionApi.connectionsRegister).mockResolvedValue(connection);
    render(<Connections language="en" connected />);
    await user.type(screen.getByLabelText('Connection name'), 'Chosen program');
    await user.type(screen.getByLabelText('Canonical executable path'), '/explicit/program');
    await user.type(screen.getByLabelText('Launch arguments (one per line)'), '--acp\n--mode=local');
    await user.click(screen.getByRole('button', { name: 'Register and run version probe' }));
    expect(sessionApi.connectionsRegister).toHaveBeenCalledWith('Chosen program', '/explicit/program', ['--acp', '--mode=local'], expect.any(String));
    expect(await screen.findByText('Selected program 1.0')).toBeVisible();
    expect(screen.getByText('identity-unverified (unknown)')).toBeVisible();
    expect(screen.getAllByText(/A version probe does not prove authentication/).length).toBeGreaterThan(0);
  });
  it('requires explicit acknowledgement after showing exact recipient and writable scope', async () => {
    const user = userEvent.setup();
    vi.mocked(sessionApi.grantsCreate).mockResolvedValue(grant);
    showSessions();
    await screen.findByRole('option', { name: '/managed/workspace (ready)' });
    await user.selectOptions(screen.getByLabelText('Managed workspace'), workspace.id);
    await user.selectOptions(screen.getByLabelText('Named connection'), connection.id);
    expect(screen.getByRole('button', { name: 'Create bounded execution grant' })).toBeDisabled();
    expect(screen.getByText(/Selected local agent · custom_acp · identity-unverified/)).toBeVisible();
    expect(screen.getAllByText(/do not enforce an operating system or network sandbox/).length).toBeGreaterThan(0);
    await user.click(screen.getByLabelText('I authorize this exact recipient, workspace and bounded execution scope.'));
    await user.click(screen.getByRole('button', { name: 'Create bounded execution grant' }));
    expect(sessionApi.grantsCreate).toHaveBeenCalledWith({ project_id: project.id, workspace_id: workspace.id, connection_id: connection.id, max_attempts: 1, max_concurrency: 1, max_duration_ms: 60_000 }, expect.any(String));
  });
  it('creates a session bound to an existing task and backend grant without sending a prompt', async () => {
    const user = userEvent.setup();
    vi.mocked(sessionApi.sessionsCreate).mockResolvedValue({ ...session, state: 'starting', revision: 1 });
    showSessions();
    await screen.findByRole('option', { name: '/managed/workspace (ready)' });
    await user.selectOptions(screen.getByLabelText('Task binding'), task.id);
    await user.selectOptions(screen.getByLabelText('Managed workspace'), workspace.id);
    await user.selectOptions(screen.getByLabelText('Named connection'), connection.id);
    await user.selectOptions(screen.getByLabelText('Execution grant'), grant.id);
    await user.click(screen.getByRole('button', { name: 'Create managed session' }));
    expect(sessionApi.sessionsCreate).toHaveBeenCalledWith(task.id, task.revision, workspace.id, connection.id, grant.id, expect.any(String));
    expect(sessionApi.sessionsSend).not.toHaveBeenCalled();
  });
  it('keeps sent prompts, completion and cancellation request semantics separate', async () => {
    const user = userEvent.setup();
    showSessions();
    await selectSession(user);
    await user.type(screen.getByLabelText('Message to this connection'), 'Only this chosen message');
    vi.mocked(sessionApi.sessionsSend).mockResolvedValue({ ...session, state: 'running', revision: 3 });
    await user.click(screen.getByRole('button', { name: 'Send message' }));
    expect(sessionApi.sessionsSend).toHaveBeenCalledWith(session, 'Only this chosen message', expect.any(String));
    expect(screen.getByRole('button', { name: 'Send message' })).toBeDisabled();
    vi.mocked(sessionApi.sessionsCancel).mockResolvedValue({ ...session, state: 'cancel_requested', revision: 4 });
    vi.mocked(sessionApi.sessionsList).mockResolvedValue([{ ...session, state: 'cancel_requested', revision: 4 }]);
    await user.click(screen.getByRole('button', { name: 'Request cancellation' }));
    expect(await screen.findAllByText('Cancellation requested')).toHaveLength(2);
    expect(screen.queryByText('Stopped (confirmed)')).not.toBeInTheDocument();
    expect(screen.queryByText('Accepted')).not.toBeInTheDocument();
  });
  it('blocks replay after an uncertain send and retains the unsent draft for review', async () => {
    const user = userEvent.setup();
    vi.mocked(sessionApi.sessionsSend).mockRejectedValue(new DaemonError('OUTCOME_UNKNOWN', 'Connection lost during write', 'unknown'));
    showSessions(); await selectSession(user);
    await user.type(screen.getByLabelText('Message to this connection'), 'Do not replay');
    await user.click(screen.getByRole('button', { name: 'Send message' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('side effects may be unknown');
    expect(screen.getByLabelText('Message to this connection')).toHaveValue('Do not replay');
    expect(screen.getByRole('button', { name: 'Send message' })).toBeDisabled();
    expect(sessionApi.sessionsSend).toHaveBeenCalledTimes(1);
  });
  it('does not offer expired, revoked or differently scoped grants for session creation', async () => {
    const user = userEvent.setup();
    vi.mocked(sessionApi.grantsList).mockResolvedValue([{ ...grant, expires_at_ms: 1 }, { ...grant, id: 'revoked', revoked: true }, { ...grant, id: 'wrong', scope: { ...grant.scope, connection_ids: ['other'] } }]);
    showSessions();
    await screen.findByRole('option', { name: '/managed/workspace (ready)' });
    await user.selectOptions(screen.getByLabelText('Managed workspace'), workspace.id);
    await user.selectOptions(screen.getByLabelText('Named connection'), connection.id);
    expect(screen.getByRole('button', { name: 'Create managed session' })).toBeDisabled();
    expect(screen.queryByRole('option', { name: grant.id })).not.toBeInTheDocument();
  });
  it('rejects stale session projections and deduplicates idempotent create responses', () => {
    const newer = { ...session, revision: 4, state: 'cancel_requested' as const };
    expect(mergeSessions([newer], [session, session])).toEqual([newer]);
  });
  it('keeps observing an idle ready process and disables sending after the daemon stops it', async () => {
    const user = userEvent.setup();
    showSessions(); await selectSession(user);
    vi.mocked(sessionApi.sessionsGet).mockResolvedValue({ ...session, state: 'stopped', revision: 3, turn_state: 'cancelled' });
    expect((await screen.findAllByText('Stopped (confirmed)', {}, { timeout: 2500 })).length).toBeGreaterThan(0);
    expect(screen.getByRole('button', { name: 'Send message' })).toBeDisabled();
  });
});
