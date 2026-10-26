import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ArtifactValidation } from './ArtifactValidation';
import { DaemonError, type Task } from './api';
import type { ManagedSession } from './session-api';
import { validationApi, type ValidationPolicy, type ValidationRecord } from './validation-api';
vi.mock('./validation-api', async original => ({ ...await original<typeof import('./validation-api')>(), validationApi: { policiesList: vi.fn(), policiesApprove: vi.fn(), recordsList: vi.fn(), run: vi.fn(), revoke: vi.fn() } }));
let serial = 0;
const date = '2026-10-27T01:02:03Z';
function fixture() {
  const task: Task = { id: `integrity-task-${++serial}`, project_id: 'project-1', goal: 'Inspect actual output integrity', state: 'failed', revision: 7, spec_revision: 1, created_at: date, updated_at: date };
  const session: ManagedSession = { id: `session-${serial}`, task_id: task.id, project_id: 'project-1', workspace_id: 'workspace-1', connection_id: 'connection-1', grant_id: 'grant-1', state: 'stopped', revision: 3, native_session_id: null, created_at: date, updated_at: date, last_error: null, turn_state: null };
  const subject = { task_id: task.id, attempt_id: 'attempt-1', spec_revision: 1, input_revision: 'base-sha', version: { kind: 'workspace_artifacts', workspace_id: session.workspace_id, base_revision: 'base-sha' } };
  const requirement = { artifact_id: 'output-1', relative_path: 'report.txt', max_bytes: 8_388_608, expected_sha256: null };
  const contract = { validator_id: 'builtin.artifact-integrity.v1', subject, inputs: [], outputs: [{ canonical_root: '/owner/managed/workspace', root_device: 1, root_inode: 2, requirement }], analysis: null };
  const policy: ValidationPolicy = { id: `policy-${serial}`, task_id: task.id, project_id: task.project_id!, session_id: session.id, attempt_id: subject.attempt_id, workspace_id: session.workspace_id, approval_id: 'native-owner-approval', revision: 1, revoked: false, scope: 'artifact_integrity_only', contract, created_at: date };
  const record: ValidationRecord = { id: `record-${serial}`, policy_id: policy.id, task_id: task.id, state: 'integrity_passed', scope: 'artifact_integrity_only', correctness: 'not_established', task_acceptance: 'not_established', integration_authorized: false, created_at: date, evidence: { schema_version: 1, digest: 'e'.repeat(64), validator_id: contract.validator_id, subject, approval_id: policy.approval_id, policy_revision: '1', policy_contract: contract, inputs: [], outputs: [{ artifact_id: 'output-1', relative_path: 'report.txt', root_device: 1, root_inode: 2, expected_sha256: null, observed_sha256: 'a'.repeat(64), size: 42, state: 'passed' }], analysis: null, passed: true } };
  return { task, session, policy, record };
}
beforeEach(() => { vi.resetAllMocks(); vi.mocked(validationApi.policiesList).mockResolvedValue([]); vi.mocked(validationApi.recordsList).mockResolvedValue([]); });
async function openForm(user: ReturnType<typeof userEvent.setup>) { await waitFor(() => expect(screen.getByRole('button', { name: 'Refresh integrity records' })).toBeEnabled()); await user.click(screen.getByText('Review a file-check policy', { selector: 'summary' })); }
async function openPolicy(user: ReturnType<typeof userEvent.setup>) { await user.click(await screen.findByText(/Approved for file integrity only/, { selector: 'summary' })); }
describe('Artifact integrity UI using mocked Tauri-facing transport (not native/real IPC evidence)', () => {
  it('approves only selected contracts, then separately runs and renders real returned evidence without task acceptance', async () => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(validationApi.policiesApprove).mockResolvedValue(f.policy); vi.mocked(validationApi.run).mockResolvedValue(f.record);
    render(<ArtifactValidation task={f.task} sessions={[f.session]} language="en" connected />); await openForm(user); await user.type(screen.getByLabelText('Output paths'), 'report.txt'); await user.click(screen.getByRole('button', { name: 'Review in native owner dialog' }));
    await waitFor(() => expect(validationApi.policiesApprove).toHaveBeenCalledWith({ session_id: f.session.id, inputs: [], outputs: [{ artifact_id: 'output-1', relative_path: 'report.txt', max_bytes: 8_388_608, expected_sha256: null }], analysis: null }, expect.any(String)));
    expect(validationApi.run).not.toHaveBeenCalled(); await openPolicy(user); await user.click(screen.getByRole('button', { name: 'Check selected files' }));
    await waitFor(() => expect(validationApi.run).toHaveBeenCalledWith(f.policy.id, 1, expect.any(String)));
    await user.click(await screen.findByText(/File integrity passed/, { selector: 'summary' }));
    expect(screen.getByText('Code correctness: not established')).toBeVisible(); expect(screen.getByText('Task acceptance and integration: not established')).toBeVisible();
    expect(screen.getByRole('table', { name: 'Outputs' })).toHaveTextContent('report.txt'); expect(screen.getByText('a'.repeat(64))).toBeVisible(); expect(screen.getByText('42')).toBeVisible(); expect(screen.getByText('e'.repeat(64))).toBeVisible();
    expect(f.task.state).toEqual('failed'); expect(screen.queryByText(/^Task accepted$/)).not.toBeInTheDocument();
  });
  it('retains selection on native decline and never retries, runs or fabricates a policy', async () => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(validationApi.policiesApprove).mockRejectedValue(new DaemonError('PERMISSION_DENIED', 'Native owner confirmation declined', 'not_started'));
    render(<ArtifactValidation task={f.task} sessions={[f.session]} language="en" connected />); await openForm(user); await user.type(screen.getByLabelText('Output paths'), 'report.txt'); await user.click(screen.getByRole('button', { name: 'Review in native owner dialog' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('declined'); expect(screen.getByLabelText('Output paths')).toHaveValue('report.txt'); expect(validationApi.policiesApprove).toHaveBeenCalledTimes(1); expect(validationApi.run).not.toHaveBeenCalled(); expect(screen.queryByText(/Approved for file integrity only/, { selector: 'summary' })).toBeNull();
  });
  it('holds unknown approval intent and exact key until an explicit same-request retry after refresh', async () => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(validationApi.policiesApprove).mockRejectedValueOnce(new Error('Transport disconnected')).mockResolvedValueOnce(f.policy);
    render(<ArtifactValidation task={f.task} sessions={[f.session]} language="en" connected />); await openForm(user); await user.type(screen.getByLabelText('Output paths'), 'report.txt'); await user.click(screen.getByRole('button', { name: 'Review in native owner dialog' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Outcome unknown'); expect(screen.getByLabelText('Output paths')).toBeDisabled(); expect(validationApi.policiesApprove).toHaveBeenCalledTimes(1); expect(screen.getByRole('button', { name: 'Review in native owner dialog' })).toBeDisabled();
    await user.click(screen.getByRole('button', { name: 'Refresh integrity records' })); await user.click(await screen.findByRole('button', { name: 'Review the same request again' }));
    await waitFor(() => expect(validationApi.policiesApprove).toHaveBeenCalledTimes(2)); const calls = vi.mocked(validationApi.policiesApprove).mock.calls; expect(calls[1]).toEqual(calls[0]); expect(validationApi.run).not.toHaveBeenCalled();
  });
  it.each(['read failure', 'foreign scope'] as const)('blocks same-request replay after a later %s until another successful reconciliation', async failure => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(validationApi.policiesApprove).mockRejectedValueOnce(new Error('Lost approval response')).mockResolvedValueOnce(f.policy);
    render(<ArtifactValidation task={f.task} sessions={[f.session]} language="en" connected />); await openForm(user); await user.type(screen.getByLabelText('Output paths'), 'report.txt'); await user.click(screen.getByRole('button', { name: 'Review in native owner dialog' })); await screen.findByRole('alert');
    const original = vi.mocked(validationApi.policiesApprove).mock.calls[0];
    await user.click(screen.getByRole('button', { name: 'Refresh integrity records' })); await waitFor(() => expect(screen.getByRole('button', { name: 'Review the same request again' })).toBeEnabled());
    if (failure === 'read failure') vi.mocked(validationApi.recordsList).mockRejectedValueOnce(new Error('Latest read failed'));
    else vi.mocked(validationApi.recordsList).mockResolvedValueOnce([{ ...f.record, task_id: 'foreign-task' }]);
    await user.click(screen.getByRole('button', { name: 'Refresh integrity records' }));
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent(failure === 'read failure' ? 'Latest read failed' : 'another task or unsupported scope'));
    expect(screen.getByRole('button', { name: 'Review in native owner dialog' })).toBeDisabled(); expect(screen.getByLabelText('Output paths')).toHaveValue('report.txt'); expect(validationApi.policiesApprove).toHaveBeenCalledTimes(1); expect(validationApi.run).not.toHaveBeenCalled(); expect(validationApi.revoke).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Refresh integrity records' })); await waitFor(() => expect(screen.getByRole('button', { name: 'Review the same request again' })).toBeEnabled()); expect(validationApi.policiesApprove).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole('button', { name: 'Review the same request again' })); await waitFor(() => expect(validationApi.policiesApprove).toHaveBeenCalledTimes(2)); expect(vi.mocked(validationApi.policiesApprove).mock.calls[1]).toEqual(original);
  });
  it('preserves unknown selection and idempotency key across tab-style unmount/remount', async () => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(validationApi.policiesApprove).mockRejectedValueOnce(new Error('Lost response')).mockResolvedValueOnce(f.policy);
    const first = render(<ArtifactValidation task={f.task} sessions={[f.session]} language="en" connected />); await openForm(user); await user.type(screen.getByLabelText('Output paths'), 'report.txt'); await user.click(screen.getByRole('button', { name: 'Review in native owner dialog' })); await screen.findByRole('alert'); first.unmount();
    render(<ArtifactValidation task={f.task} sessions={[f.session]} language="en" connected />); await openForm(user); expect(screen.getByLabelText('Output paths')).toHaveValue('report.txt'); await user.click(await screen.findByRole('button', { name: 'Review the same request again' })); await waitFor(() => expect(validationApi.policiesApprove).toHaveBeenCalledTimes(2)); expect(vi.mocked(validationApi.policiesApprove).mock.calls[1]).toEqual(vi.mocked(validationApi.policiesApprove).mock.calls[0]);
  });
  it('holds an unknown run key without automatic retry and reuses its original policy revision', async () => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(validationApi.policiesList).mockResolvedValue([f.policy]); vi.mocked(validationApi.run).mockRejectedValueOnce(new DaemonError('OUTCOME_UNKNOWN', 'Lost validation response', 'unknown')).mockResolvedValueOnce(f.record);
    render(<ArtifactValidation task={f.task} sessions={[f.session]} language="en" connected />); await openPolicy(user); await user.click(screen.getByRole('button', { name: 'Check selected files' })); expect(await screen.findByRole('alert')).toHaveTextContent('Outcome unknown'); expect(validationApi.run).toHaveBeenCalledTimes(1); await user.click(screen.getByRole('button', { name: 'Refresh integrity records' })); await user.click(await screen.findByRole('button', { name: 'Retry the same file check' })); await waitFor(() => expect(validationApi.run).toHaveBeenCalledTimes(2)); expect(vi.mocked(validationApi.run).mock.calls[1]).toEqual(vi.mocked(validationApi.run).mock.calls[0]);
  });
  it('excludes running/foreign-task/foreign-project sessions and refuses traversal before invoking native approval', async () => {
    const user = userEvent.setup(), f = fixture(); const sessions = [f.session, { ...f.session, id: 'running', state: 'running' as const }, { ...f.session, id: 'other-task', task_id: 'foreign' }, { ...f.session, id: 'other-project', project_id: 'foreign' }];
    render(<ArtifactValidation task={f.task} sessions={sessions} language="en" connected />); await openForm(user); expect(screen.getByRole('option', { name: new RegExp(f.session.id) })).toBeVisible(); for (const id of ['running', 'other-task', 'other-project']) expect(screen.queryByRole('option', { name: new RegExp(id) })).toBeNull();
    await user.type(screen.getByLabelText('Output paths'), '../private'); await user.click(screen.getByRole('button', { name: 'Review in native owner dialog' })); expect(await screen.findByRole('alert')).toHaveTextContent('relative paths'); expect(validationApi.policiesApprove).not.toHaveBeenCalled();
  });
  it('renders failed server evidence and malicious path text without interpreting markup or opening files', async () => {
    const user = userEvent.setup(), f = fixture(), path = '<img src=x onerror=alert(1)>';
    vi.mocked(validationApi.recordsList).mockResolvedValue([{ ...f.record, state: 'failed', evidence: { ...f.record.evidence, passed: false, outputs: [{ ...f.record.evidence.outputs[0], relative_path: path, state: 'missing', observed_sha256: null, size: null }] } }]);
    render(<ArtifactValidation task={f.task} sessions={[f.session]} language="en" connected />); await user.click(await screen.findByText(/File integrity check failed/, { selector: 'summary' })); expect(screen.getByText(path)).toBeVisible(); expect(screen.getByText('Missing')).toBeVisible(); expect(document.querySelector('img, script, a[href]')).toBeNull(); expect(validationApi.run).not.toHaveBeenCalled();
  });
  it('revokes future checks with the exact revision while preserving existing receipts', async () => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(validationApi.policiesList).mockResolvedValue([f.policy]); vi.mocked(validationApi.recordsList).mockResolvedValue([f.record]); vi.mocked(validationApi.revoke).mockResolvedValue({ ...f.policy, revision: 2, revoked: true });
    render(<ArtifactValidation task={f.task} sessions={[f.session]} language="en" connected />); await openPolicy(user); await user.click(screen.getByRole('button', { name: 'Revoke future checks' })); await waitFor(() => expect(validationApi.revoke).toHaveBeenCalledWith(f.policy.id, 1)); expect(screen.getByRole('button', { name: 'Check selected files' })).toBeDisabled(); expect(screen.getByText(/File integrity passed/, { selector: 'summary' })).toBeVisible();
  });
  it('localizes warnings, refuses stale policy runs and never mutates while disconnected', async () => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(validationApi.policiesList).mockResolvedValue([f.policy]); const view = render(<ArtifactValidation task={{ ...f.task, spec_revision: 2 }} sessions={[f.session]} language="zh" connected />); await user.click(await screen.findByText(/仅批准文件完整性检查/, { selector: 'summary' })); expect(screen.getByText('文件完整性检查，不代表代码正确、任务验收或合入授权。')).toBeVisible(); expect(screen.getByRole('button', { name: '检查选定文件' })).toBeDisabled(); expect(screen.getByText('任务规格已变化，请重新审阅策略。')).toBeVisible();
    view.rerender(<ArtifactValidation task={f.task} sessions={[f.session]} language="zh" connected={false} />); await user.click(screen.getByText('审阅文件检查策略', { selector: 'summary' })); expect(screen.getByRole('button', { name: '在原生负责人对话框中审阅' })).toBeDisabled(); expect(validationApi.policiesApprove).not.toHaveBeenCalled(); expect(validationApi.run).not.toHaveBeenCalled();
  });
  it('shows lease/stop denial without treating a failed session label as confirmed stop evidence', async () => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(validationApi.policiesApprove).mockRejectedValue(new DaemonError('WORKSPACE_BUSY', 'No matching confirmed stop receipt', 'not_started'));
    render(<ArtifactValidation task={f.task} sessions={[{ ...f.session, state: 'failed' }]} language="en" connected />); await openForm(user); await user.type(screen.getByLabelText('Output paths'), 'report.txt'); await user.click(screen.getByRole('button', { name: 'Review in native owner dialog' })); expect(await screen.findByRole('alert')).toHaveTextContent('confirmed stop receipt'); expect(screen.getByLabelText('Output paths')).toHaveValue('report.txt'); expect(validationApi.run).not.toHaveBeenCalled();
  });
});

describe('Integrity history scope checks with mocked transport', () => {
  it('rejects a foreign-task record instead of showing another task history', async () => {
    const f = fixture(); vi.mocked(validationApi.recordsList).mockResolvedValue([{ ...f.record, task_id: 'foreign-task' }]);
    render(<ArtifactValidation task={f.task} sessions={[f.session]} language="en" connected />);
    expect(await screen.findByRole('alert')).toHaveTextContent('another task or unsupported scope');
    expect(screen.queryByText(/File integrity passed/, { selector: 'summary' })).toBeNull(); expect(validationApi.run).not.toHaveBeenCalled();
  });
  it.each([{ integration_authorized: true }, { task_acceptance: 'accepted' }])('rejects same-task elevated authority %j independently', async elevated => {
    const f = fixture(); vi.mocked(validationApi.recordsList).mockResolvedValue([{ ...f.record, ...elevated }]);
    render(<ArtifactValidation task={f.task} sessions={[f.session]} language="en" connected />);
    expect(await screen.findByRole('alert')).toHaveTextContent('another task or unsupported scope'); expect(screen.queryByText(/File integrity passed/, { selector: 'summary' })).toBeNull(); expect(validationApi.run).not.toHaveBeenCalled();
  });
  it('labels unknown record and artifact states without presenting them as passing or elevating authority', async () => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(validationApi.recordsList).mockResolvedValue([{ ...f.record, state: 'future_record_state', evidence: { ...f.record.evidence, outputs: [{ ...f.record.evidence.outputs[0], state: 'future_artifact_state' }] } }]);
    render(<ArtifactValidation task={f.task} sessions={[f.session]} language="en" connected />);
    await user.click(await screen.findByText(/Unknown recorded state: future_record_state/, { selector: 'summary' })); expect(screen.getByText('Unknown recorded state: future_artifact_state')).toBeVisible(); expect(screen.queryByText(/File integrity passed/, { selector: 'summary' })).toBeNull(); expect(screen.queryByText(/^Passed$/)).toBeNull(); expect(screen.getByText('Task acceptance and integration: not established')).toBeVisible(); expect(validationApi.run).not.toHaveBeenCalled();
  });
});
