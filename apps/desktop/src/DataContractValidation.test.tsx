import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DataContractValidation } from './DataContractValidation';
import { DaemonError, type Task } from './api';
import type { ManagedSession } from './session-api';
import { dataValidationApi, dataValidator, newDataForm, parseDataSelection, type DataPolicy, type DataRecord } from './data-validation-api';
vi.mock('./data-validation-api', async original => ({ ...await original<typeof import('./data-validation-api')>(), dataValidationApi: { policiesList: vi.fn(), recordsList: vi.fn(), approve: vi.fn(), run: vi.fn(), revoke: vi.fn() } }));
let serial = 0;
const date = '2026-10-27T02:03:04Z';
function fixture() {
  const task: Task = { id: `data-task-${++serial}`, project_id: 'project', goal: 'Check a data contract', state: 'failed', revision: 4, spec_revision: 1, created_at: date, updated_at: date };
  const session: ManagedSession = { id: `session-${serial}`, task_id: task.id, project_id: 'project', workspace_id: 'workspace', connection_id: 'connection', grant_id: 'grant', state: 'stopped', revision: 2, native_session_id: null, created_at: date, updated_at: date, last_error: null, turn_state: null };
  const form = newDataForm(session.id); Object.assign(form, { inputs: `input.csv | ${'a'.repeat(64)}`, tablePath: 'table.json', scriptPath: 'reproduce.py', environmentPath: 'environment.json', datasetRows: '2' }); Object.assign(form.columns[0], { name: 'value', min: '0', max: '10', unit: 'm' });
  const selection = parseDataSelection(form);
  const subject = { task_id: task.id, attempt_id: 'attempt', spec_revision: 1, input_revision: 'input-v1', version: { kind: 'data', dataset_revision: 'input-v1' } };
  const fileContract = { validator_id: 'builtin.artifact-integrity.v1', subject, inputs: selection.inputs.map(requirement => ({ canonical_root: '/managed', root_device: 1, root_inode: 2, requirement })), outputs: selection.outputs.map(requirement => ({ canonical_root: '/managed', root_device: 1, root_inode: 2, requirement })), analysis: selection.table.analysis };
  const policy: DataPolicy = { id: `policy-${serial}`, task_id: task.id, project_id: 'project', session_id: session.id, attempt_id: 'attempt', workspace_id: 'workspace', approval_id: 'native-owner', revision: 1, revoked: false, created_at: date, scope: 'data_contract_only', validator_id: dataValidator, file_contract: fileContract, table: selection.table, contract_digest: 'c'.repeat(64) };
  const artifact = (requirement: typeof selection.inputs[number]) => ({ ...requirement, root_device: 1, root_inode: 2, observed_sha256: 'a'.repeat(64), size: 100, state: 'passed' });
  const record: DataRecord = { id: `record-${serial}`, policy_id: policy.id, task_id: task.id, state: 'contract_passed', scope: 'data_contract_only', correctness: 'approved_schema_ranges_metadata_only', task_acceptance: 'not_established', integration_authorized: false, created_at: date, evidence: { validator_id: dataValidator, approval_id: 'native-owner', policy_revision: '1', contract: { validator_id: dataValidator, artifact_policy: fileContract, table: selection.table }, contract_digest: policy.contract_digest, file_integrity: { schema_version: 1, digest: 'f'.repeat(64), validator_id: fileContract.validator_id, subject, approval_id: 'native-owner', policy_revision: '1', policy_contract: fileContract, inputs: selection.inputs.map(artifact), outputs: selection.outputs.map(artifact), analysis: selection.table.analysis, passed: true }, table_sha256: 'b'.repeat(64), checked_rows: 2, failure: null, passed: true, digest: 'e'.repeat(64) } };
  return { task, session, policy, record, selection };
}
beforeEach(() => { vi.resetAllMocks(); vi.mocked(dataValidationApi.policiesList).mockResolvedValue([]); vi.mocked(dataValidationApi.recordsList).mockResolvedValue([]); });
async function open(user: ReturnType<typeof userEvent.setup>) { await user.click(screen.getByRole('heading', { name: 'Data contract validation' })); await waitFor(() => expect(screen.getByRole('button', { name: 'Refresh data contract records' })).toBeEnabled()); }
function fill() { for (const [label, value] of [['Fixed input paths and SHA-256', `input.csv | ${'a'.repeat(64)}`], ['JSON table path', 'table.json'], ['Reproduction script path', 'reproduce.py'], ['Environment description path', 'environment.json'], ['Declared dataset rows', '2'], ['Column name 1', 'value'], ['Minimum 1', '0'], ['Maximum 1', '10'], ['Unit metadata 1', 'm']]) fireEvent.change(screen.getByLabelText(label), { target: { value } }); }
const approve = () => screen.getByRole('button', { name: 'Review data contract in native dialog' });
async function openPolicy(user: ReturnType<typeof userEvent.setup>) { await user.click(await screen.findByText(/Approved for data contract only/, { selector: 'summary' })); }
describe('Data contract UI with mocked native transport; no real Provider/native owner proof', () => {
  it('is folded by default and only reads dedicated namespaces before explicit native approval and separate run', async () => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(dataValidationApi.approve).mockResolvedValue(f.policy); vi.mocked(dataValidationApi.run).mockResolvedValue(f.record);
    render(<DataContractValidation task={f.task} sessions={[f.session]} language="en" connected />);
    expect(screen.getByTestId('data-contract-validation').querySelector('details')).not.toHaveAttribute('open'); expect(dataValidationApi.approve).not.toHaveBeenCalled();
    await open(user); fill(); await user.click(approve()); await waitFor(() => expect(dataValidationApi.approve).toHaveBeenCalledWith(f.selection, expect.any(String))); expect(dataValidationApi.run).not.toHaveBeenCalled();
    await openPolicy(user); await user.click(screen.getByRole('button', { name: 'Validate data contract' })); await user.click(await screen.findByText(/Data contract passed/, { selector: 'summary' }));
    expect(dataValidationApi.run).toHaveBeenCalledWith(f.policy.id, 1, expect.any(String)); expect(screen.getByText('Correctness: approved schema, ranges and metadata only')).toBeVisible(); expect(screen.getByText('Whole-task acceptance and integration: not established')).toBeVisible(); expect(screen.getByText('b'.repeat(64))).toBeVisible(); expect(screen.getByText('2', { selector: 'dd' })).toBeVisible(); expect(f.task.state).toBe('failed');
  });
  it('retains form on native denial, and rejects unsafe integer form values without a second invoke', async () => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(dataValidationApi.approve).mockRejectedValue(new DaemonError('PERMISSION_DENIED', 'Native owner declined', 'not_started'));
    render(<DataContractValidation task={f.task} sessions={[f.session]} language="en" connected />); await open(user); fill(); await user.click(approve()); expect(await screen.findByRole('alert')).toHaveTextContent('declined'); expect(screen.getByLabelText('JSON table path')).toHaveValue('table.json');
    fireEvent.change(screen.getByLabelText('Scalar type 1'), { target: { value: 'integer' } }); fireEvent.change(screen.getByLabelText('Minimum 1'), { target: { value: '9007199254740993' } }); await user.click(approve()); expect(screen.getByRole('alert')).toHaveTextContent('safe integers'); expect(dataValidationApi.approve).toHaveBeenCalledTimes(1); expect(dataValidationApi.run).not.toHaveBeenCalled();
  });
  it('rejects traversal and missing metadata before proposing any authority', async () => {
    const user = userEvent.setup(), f = fixture(); render(<DataContractValidation task={f.task} sessions={[f.session]} language="en" connected />); await open(user); fill();
    fireEvent.change(screen.getByLabelText('JSON table path'), { target: { value: '../outside' } }); await user.click(approve()); expect(screen.getByRole('alert')).toHaveTextContent('relative paths');
    fireEvent.change(screen.getByLabelText('JSON table path'), { target: { value: 'table.json' } }); fireEvent.change(screen.getByLabelText('Unit metadata 1'), { target: { value: '' } }); await user.click(approve()); expect(screen.getByRole('alert')).toHaveTextContent('unit metadata'); expect(dataValidationApi.approve).not.toHaveBeenCalled();
  });
  it('retains exact unknown approval payload/key across unmount and does not mutate on reconciliation', async () => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(dataValidationApi.approve).mockRejectedValueOnce(new Error('Lost response')).mockResolvedValueOnce(f.policy);
    const view = render(<DataContractValidation task={f.task} sessions={[f.session]} language="en" connected />); await open(user); fill(); await user.click(approve()); expect(await screen.findByRole('alert')).toHaveTextContent('Outcome unknown'); expect(screen.getByLabelText('JSON table path')).toBeDisabled(); const first = vi.mocked(dataValidationApi.approve).mock.calls[0]; view.unmount();
    render(<DataContractValidation task={f.task} sessions={[f.session]} language="en" connected />); await open(user); expect(screen.getByLabelText('JSON table path')).toHaveValue('table.json'); expect(dataValidationApi.approve).toHaveBeenCalledTimes(1); await user.click(screen.getByRole('button', { name: 'Review the same data request again' })); await waitFor(() => expect(dataValidationApi.approve).toHaveBeenCalledTimes(2)); expect(vi.mocked(dataValidationApi.approve).mock.calls[1]).toEqual(first);
  });
  it.each(['read failure', 'foreign scope'] as const)('invalidates earlier retry freshness after a later %s', async kind => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(dataValidationApi.approve).mockRejectedValue(new Error('Lost response'));
    render(<DataContractValidation task={f.task} sessions={[f.session]} language="en" connected />); await open(user); fill(); await user.click(approve()); await screen.findByRole('alert');
    await user.click(screen.getByRole('button', { name: 'Refresh data contract records' })); await waitFor(() => expect(screen.getByRole('button', { name: 'Review the same data request again' })).toBeEnabled());
    if (kind === 'read failure') vi.mocked(dataValidationApi.recordsList).mockRejectedValueOnce(new Error('Latest read failed')); else vi.mocked(dataValidationApi.recordsList).mockResolvedValueOnce([{ ...f.record, task_id: 'foreign' }]);
    await user.click(screen.getByRole('button', { name: 'Refresh data contract records' })); expect(await screen.findByRole('alert')).toHaveTextContent(kind === 'read failure' ? 'Latest read failed' : 'foreign or unsupported'); expect(approve()).toBeDisabled(); expect(dataValidationApi.approve).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole('button', { name: 'Refresh data contract records' })); await waitFor(() => expect(screen.getByRole('button', { name: 'Review the same data request again' })).toBeEnabled()); expect(dataValidationApi.approve).toHaveBeenCalledTimes(1);
  });
  it('keeps an unknown run key and original revision after reconciliation returns a newer policy', async () => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(dataValidationApi.policiesList).mockResolvedValue([f.policy]); vi.mocked(dataValidationApi.run).mockRejectedValueOnce(new DaemonError('OUTCOME_UNKNOWN', 'Lost run', 'unknown')).mockResolvedValueOnce(f.record);
    render(<DataContractValidation task={f.task} sessions={[f.session]} language="en" connected />); await open(user); await openPolicy(user); await user.click(screen.getByRole('button', { name: 'Validate data contract' })); await screen.findByRole('alert'); const original = vi.mocked(dataValidationApi.run).mock.calls[0];
    vi.mocked(dataValidationApi.policiesList).mockResolvedValue([{ ...f.policy, revision: 2 }]); await user.click(screen.getByRole('button', { name: 'Refresh data contract records' })); await user.click(await screen.findByRole('button', { name: 'Retry the same data check' })); await waitFor(() => expect(dataValidationApi.run).toHaveBeenCalledTimes(2)); expect(vi.mocked(dataValidationApi.run).mock.calls[1]).toEqual(original);
  });
  it('never carries pending intent into another task, then restores its original payload when returning', async () => {
    const user = userEvent.setup(), f = fixture(), other = fixture(); vi.mocked(dataValidationApi.approve).mockRejectedValue(new Error('Lost response'));
    const view = render(<DataContractValidation task={f.task} sessions={[f.session]} language="en" connected />); await open(user); fill(); await user.click(approve()); await screen.findByRole('alert');
    view.rerender(<DataContractValidation task={other.task} sessions={[other.session]} language="en" connected />); await waitFor(() => expect(approve()).toBeEnabled()); expect(screen.getByLabelText('JSON table path')).toHaveValue(''); expect(dataValidationApi.approve).toHaveBeenCalledTimes(1);
    view.rerender(<DataContractValidation task={f.task} sessions={[f.session]} language="en" connected />); await waitFor(() => expect(screen.getByRole('button', { name: 'Review the same data request again' })).toBeEnabled()); expect(screen.getByLabelText('JSON table path')).toHaveValue('table.json'); expect(dataValidationApi.approve).toHaveBeenCalledTimes(1);
  });
  it.each([{ task_id: 'foreign' }, { integration_authorized: true }, { correctness: 'algorithm_correct' }, { task_acceptance: 'accepted' }])('rejects foreign/elevated record %j without showing success', async patch => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(dataValidationApi.recordsList).mockResolvedValue([{ ...f.record, ...patch }]); render(<DataContractValidation task={f.task} sessions={[f.session]} language="en" connected />); await user.click(screen.getByRole('heading', { name: 'Data contract validation' })); expect(await screen.findByRole('alert')).toHaveTextContent('foreign or unsupported'); expect(screen.queryByText(/Data contract passed/, { selector: 'summary' })).toBeNull(); expect(approve()).toBeDisabled();
  });
  it('renders unknown states and failure pointers as text without executing HTML or interpreting future states as success', async () => {
    const user = userEvent.setup(), f = fixture(), pointer = '<img src=x onerror=alert(1)>';
    vi.mocked(dataValidationApi.recordsList).mockResolvedValue([{ ...f.record, state: 'constructor', evidence: { ...f.record.evidence, passed: false, failure: { code: 'future_failure', pointer, line: 1, column: 2 } } }]);
    render(<DataContractValidation task={f.task} sessions={[f.session]} language="en" connected />); await open(user); await user.click(await screen.findByText(/Unknown recorded state: constructor/, { selector: 'summary' })); expect(screen.getByText(pointer)).toBeVisible(); expect(screen.getByText('future_failure')).toBeVisible(); expect(document.querySelector('img,script,a[href]')).toBeNull(); expect(screen.queryByText(/Data contract passed/, { selector: 'summary' })).toBeNull(); expect(dataValidationApi.run).not.toHaveBeenCalled();
  });
  it('selects only stopped/failed matching task/project sessions and keeps stale/foreign policy runs disabled', async () => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(dataValidationApi.policiesList).mockResolvedValue([f.policy]);
    render(<DataContractValidation task={{ ...f.task, spec_revision: 2 }} sessions={[f.session, { ...f.session, id: 'running-session', state: 'running' }, { ...f.session, id: 'foreign-project', project_id: 'foreign' }, { ...f.session, id: 'foreign-task', task_id: 'foreign' }]} language="en" connected />); await open(user); await openPolicy(user);
    expect(screen.getByRole('button', { name: 'Validate data contract' })).toBeDisabled(); expect(screen.getByText('Task specification changed; review a fresh policy.')).toBeVisible(); for (const name of ['running-session', 'foreign-project', 'foreign-task']) expect(screen.queryByRole('option', { name: new RegExp(name) })).toBeNull(); expect(dataValidationApi.run).not.toHaveBeenCalled();
  });
  it('revokes with exact revision and preserves historical receipts; unknown revoke reconciles without auto-replay', async () => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(dataValidationApi.policiesList).mockResolvedValue([f.policy]); vi.mocked(dataValidationApi.recordsList).mockResolvedValue([f.record]); vi.mocked(dataValidationApi.revoke).mockRejectedValueOnce(new Error('Lost revoke'));
    render(<DataContractValidation task={f.task} sessions={[f.session]} language="en" connected />); await open(user); await openPolicy(user); await user.click(screen.getByRole('button', { name: 'Revoke future checks' })); await screen.findByRole('alert'); expect(dataValidationApi.revoke).toHaveBeenCalledWith(f.policy.id, 1);
    vi.mocked(dataValidationApi.policiesList).mockResolvedValue([{ ...f.policy, revoked: true, revision: 2 }]); await user.click(screen.getByRole('button', { name: 'Refresh data contract records' })); expect(await screen.findByText(/Data policy revoked/, { selector: 'summary' })).toBeVisible(); expect(dataValidationApi.revoke).toHaveBeenCalledTimes(1); expect(screen.getByRole('button', { name: 'Validate data contract' })).toBeDisabled(); expect(screen.getByText(/Data contract passed/, { selector: 'summary' })).toBeVisible();
  });
  it.each([{ task_id: 'other-task' }, { project_id: 'other-project' }, { validator_id: 'future.validator' }])('rejects foreign/unsupported policy %j and does not reuse an earlier good read', async patch => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(dataValidationApi.policiesList).mockResolvedValueOnce([f.policy]).mockResolvedValueOnce([{ ...f.policy, ...patch }]);
    render(<DataContractValidation task={f.task} sessions={[f.session]} language="en" connected />); await open(user); await openPolicy(user); expect(screen.getByRole('button', { name: 'Validate data contract' })).toBeEnabled();
    await user.click(screen.getByRole('button', { name: 'Refresh data contract records' })); expect(await screen.findByRole('alert')).toHaveTextContent('foreign or unsupported'); expect(screen.getByRole('button', { name: 'Validate data contract' })).toBeDisabled(); expect(approve()).toBeDisabled(); expect(dataValidationApi.run).not.toHaveBeenCalled();
  });
  it('does not turn a foreign approval response into success or discard its uncertain intent', async () => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(dataValidationApi.approve).mockResolvedValue({ ...f.policy, project_id: 'foreign' });
    render(<DataContractValidation task={f.task} sessions={[f.session]} language="en" connected />); await open(user); fill(); await user.click(approve()); expect(await screen.findByRole('alert')).toHaveTextContent('Outcome unknown'); expect(screen.queryByText(/Approved for data contract only/, { selector: 'summary' })).toBeNull(); expect(screen.getByLabelText('JSON table path')).toBeDisabled(); expect(dataValidationApi.run).not.toHaveBeenCalled();
  });
  it('allows labelled column editing and keeps sample scope and limitation explicit in the submitted contract', async () => {
    const user = userEvent.setup(), f = fixture(); vi.mocked(dataValidationApi.approve).mockRejectedValue(new DaemonError('PERMISSION_DENIED', 'Not approved', 'not_started'));
    render(<DataContractValidation task={f.task} sessions={[f.session]} language="en" connected />); await open(user); fill();
    await user.selectOptions(screen.getByLabelText('Check scope'), 'sample'); fireEvent.change(screen.getByLabelText('Declared dataset rows'), { target: { value: '100' } }); fireEvent.change(screen.getByLabelText('Sample rows'), { target: { value: '2' } }); fireEvent.change(screen.getByLabelText('Sample limitation'), { target: { value: 'Selected subset only' } });
    await user.click(screen.getByRole('button', { name: 'Add column' })); fireEvent.change(screen.getByLabelText('Column name 2'), { target: { value: 'label' } }); await user.selectOptions(screen.getByLabelText('Scalar type 2'), 'string'); fireEvent.change(screen.getByLabelText('String bytes 2'), { target: { value: '32' } }); await user.click(screen.getByLabelText('Nullable 2')); await user.click(approve());
    await waitFor(() => expect(dataValidationApi.approve).toHaveBeenCalledOnce()); const selection = vi.mocked(dataValidationApi.approve).mock.calls[0][0]; expect(selection.table.dataset_rows).toBe(100); expect(selection.table.analysis.scope).toEqual({ kind: 'sample', rows: 2, limitation: 'Selected subset only' }); expect(selection.table.columns[1]).toEqual({ name: 'label', required: true, nullable: true, scalar: { type: 'string', max_bytes: 32 } }); expect(dataValidationApi.run).not.toHaveBeenCalled();
  });
  it('disables all mutations while disconnected and localizes the default-folded warnings', async () => {
    const user = userEvent.setup(), f = fixture(); render(<DataContractValidation task={f.task} sessions={[f.session]} language="zh" connected={false} />); await user.click(screen.getByRole('heading', { name: '数据合同验证' })); expect(screen.getByRole('button', { name: '在原生对话框中审阅数据合同' })).toBeDisabled(); expect(screen.getByText(/不运行脚本、分析引擎或 Provider/)).toBeVisible(); expect(dataValidationApi.approve).not.toHaveBeenCalled(); expect(dataValidationApi.run).not.toHaveBeenCalled(); expect(dataValidationApi.policiesList).not.toHaveBeenCalled();
  });
});
