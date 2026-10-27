import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DataWorkspaces } from './DataWorkspaces';
import { dataCreationShape, dataWorkspaceApi as api, type DataCreation } from './data-workspace-api';
import { copyFixture, dataWorkspaceFixture } from './data-workspace-test-fixtures';

vi.mock('./data-workspace-api', async original => ({ ...await original<typeof import('./data-workspace-api')>(), dataWorkspaceApi: { previewStart: vi.fn(), previewsList: vi.fn(), previewGet: vi.fn(), previewCancel: vi.fn(), approve: vi.fn(), creationsList: vi.fn(), creationGet: vi.fn(), creationCancel: vi.fn() } }));
type Fixture = ReturnType<typeof dataWorkspaceFixture>;
function stopped(f: Fixture, patch: Record<string, unknown> = {}): DataCreation {
  return { ...copyFixture(f.creation), revision: 4, state: 'cancelled', cancel_requested: true, io_completion: 'worker_joined', contract: null, observation: null, error: { code: 'CANCEL_REQUESTED', message: 'Owned copy worker joined after cancellation', retryable: false, side_effect_state: 'confirmed' }, ...patch } as DataCreation;
}
const stoppedLabel = 'Owned copy worker stopped; retained files need review';
const retainedHint = 'The owned copy worker joined after cancellation. Files may have been written and partial output is retained; stopping does not mean deletion, a ready workspace or accepted artifacts.';
beforeEach(() => { vi.resetAllMocks(); vi.mocked(api.previewsList).mockResolvedValue([]); vi.mocked(api.creationsList).mockResolvedValue([]); });
async function open(user: ReturnType<typeof userEvent.setup>) { await user.click(screen.getByRole('heading', { name: 'Selected-file data workspace' })); await waitFor(() => expect(screen.getByRole('button', { name: 'Refresh data workspace records' })).toBeEnabled()); }
const refresh = () => screen.getByRole('button', { name: 'Refresh data workspace records' });
async function detail(user: ReturnType<typeof userEvent.setup>, id: string) { const summary = screen.getByText(new RegExp(` · ${id}$`), { selector: 'summary' }); if (!summary.parentElement!.hasAttribute('open')) await user.click(summary); return within(summary.parentElement!); }
async function loseCancellation(user: ReturnType<typeof userEvent.setup>, f: Fixture, onReady = vi.fn()) {
  vi.mocked(api.previewsList).mockResolvedValue([f.preview]);
  vi.mocked(api.creationsList).mockResolvedValue([{ ...f.creation, revision: 3 }]);
  vi.mocked(api.creationCancel).mockRejectedValue(new Error('Cancellation response lost'));
  const view = render(<DataWorkspaces project={f.project} language="en" connected onReady={onReady} />);
  await open(user); await user.click((await detail(user, f.creation.id)).getByRole('button', { name: 'Request copy cancellation' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('Request outcome unknown');
  return view;
}

describe('readonly owned-copy completion DTO, mocked transport', () => {
  it('accepts a joined cancellation before reserve or after a failed partial-copy observation', () => {
    const f = dataWorkspaceFixture(), partial = { ...copyFixture(f.ready.observation!), state: 'failed', error: 'cancel_requested', copied_files: 0 };
    expect(dataCreationShape(stopped(f), f.project.id)).toBe(true);
    expect(dataCreationShape(stopped(f, { observation: partial }), f.project.id)).toBe(true);
  });
  it.each(['missing', 'null', 'not_scheduled', 'interrupted_no_join_receipt', 'unknown', 'object', 'no-cancel', 'ready-contract', 'no-error', 'wrong-error', 'uncertain-error', 'ready-observation'] as const)('refuses contradictory cancelled receipt %s', kind => {
    const f = dataWorkspaceFixture();
    const patch: Record<string, unknown> = {};
    if (kind === 'missing') patch.io_completion = undefined;
    else if (kind === 'null') patch.io_completion = null;
    else if (['not_scheduled', 'interrupted_no_join_receipt', 'unknown'].includes(kind)) patch.io_completion = kind;
    else if (kind === 'object') patch.io_completion = { joined: true };
    else if (kind === 'no-cancel') patch.cancel_requested = false;
    else if (kind === 'ready-contract') patch.contract = f.ready.contract;
    else if (kind === 'no-error') patch.error = null;
    else if (kind === 'wrong-error') patch.error = { code: 'DEADLINE_EXCEEDED', message: 'deadline', retryable: false, side_effect_state: 'confirmed' };
    else if (kind === 'uncertain-error') patch.error = { code: 'CANCEL_REQUESTED', message: 'unjoined', retryable: false, side_effect_state: 'unknown' };
    else patch.observation = f.ready.observation;
    expect(dataCreationShape(stopped(f, patch), f.project.id)).toBe(false);
  });
  it.each(['legacy-ready', 'null-ready', 'joined-ready', 'joined-failed', 'joined-unknown', 'unscheduled-failed', 'interrupted-unknown', 'legacy-cancel-failed'] as const)('preserves valid %s without inferring cancellation or private witness', kind => {
    const f = dataWorkspaceFixture(); let row: DataCreation;
    if (kind === 'legacy-ready') row = f.ready;
    else if (kind === 'null-ready') row = { ...f.ready, io_completion: null } as DataCreation;
    else if (kind === 'joined-ready') row = { ...f.ready, cancel_requested: true, io_completion: 'worker_joined' } as DataCreation;
    else if (kind === 'joined-failed') row = { ...f.creation, state: 'failed', io_completion: 'worker_joined', error: { code: 'DEADLINE_EXCEEDED', message: 'deadline', retryable: false, side_effect_state: 'confirmed' } } as DataCreation;
    else if (kind === 'joined-unknown') row = { ...f.creation, state: 'outcome_unknown', io_completion: 'worker_joined' } as DataCreation;
    else if (kind === 'unscheduled-failed') row = { ...f.creation, state: 'failed', io_completion: 'not_scheduled' } as DataCreation;
    else if (kind === 'interrupted-unknown') row = { ...f.creation, state: 'outcome_unknown', io_completion: 'interrupted_no_join_receipt' } as DataCreation;
    else row = { ...f.creation, state: 'failed', cancel_requested: true, error: { code: 'CANCEL_REQUESTED', message: 'Legacy cancellation failure', retryable: false, side_effect_state: 'not_started' } };
    expect(dataCreationShape(row, f.project.id)).toBe(true);
  });
  it.each(['unknown-io', 'active-joined', 'ready-unscheduled', 'ready-interrupted', 'observed-unscheduled'] as const)('refuses malformed completion %s instead of advertising ready', kind => {
    const f = dataWorkspaceFixture(); let row: DataCreation;
    if (kind === 'unknown-io') row = { ...f.ready, io_completion: 'future_receipt' } as unknown as DataCreation;
    else if (kind === 'active-joined') row = { ...f.creation, io_completion: 'worker_joined' } as DataCreation;
    else if (kind === 'ready-unscheduled') row = { ...f.ready, io_completion: 'not_scheduled' } as DataCreation;
    else if (kind === 'ready-interrupted') row = { ...f.ready, io_completion: 'interrupted_no_join_receipt' } as DataCreation;
    else row = { ...f.creation, state: 'failed', observation: { ...f.ready.observation!, state: 'failed' }, io_completion: 'not_scheduled' } as DataCreation;
    expect(dataCreationShape(row, f.project.id)).toBe(false);
  });
});

describe('copy cancellation evidence and reconciliation, not real I/O stop proof', () => {
  it('settles a fresh joined cancellation, stops polling, retains partial output and requires new explicit preview/approval', async () => {
    const user = userEvent.setup(), f = dataWorkspaceFixture(), onReady = vi.fn();
    await loseCancellation(user, f, onReady);
    vi.mocked(api.creationsList).mockResolvedValue([stopped(f)]); await user.click(refresh());
    expect(screen.queryByRole('button', { name: 'Recover the same request' })).toBeNull();
    expect(screen.getByText(`${stoppedLabel} · ${f.creation.id}`, { selector: 'summary' })).toBeVisible();
    expect((await detail(user, f.creation.id)).getByText(retainedHint)).toBeVisible();
    expect(onReady).not.toHaveBeenCalled(); expect(screen.queryByText(/^Copy ready/)).toBeNull();
    const reads = vi.mocked(api.creationsList).mock.calls.length;
    vi.useFakeTimers(); try { await act(async () => { await vi.advanceTimersByTimeAsync(2000); }); expect(api.creationsList).toHaveBeenCalledTimes(reads); } finally { vi.useRealTimers(); }
    expect(api.previewStart).not.toHaveBeenCalled(); expect(api.approve).not.toHaveBeenCalled(); expect(api.creationCancel).toHaveBeenCalledOnce();
    expect((await detail(user, f.preview.id)).getByRole('button', { name: 'Review this data copy in native dialog' })).toBeDisabled();
    const nextId = f.preview.id + '-next', next = { ...copyFixture(f.preview), id: nextId, snapshot: { ...copyFixture(f.snapshot), contract: { ...copyFixture(f.snapshot.contract), approval_reference: nextId } } };
    vi.mocked(api.previewStart).mockImplementation(async () => { vi.mocked(api.previewsList).mockResolvedValue([f.preview, next]); return next; });
    fireEvent.change(screen.getByLabelText('Relative files (one path per line; optional | SHA-256)'), { target: { value: 'data.csv' } });
    await user.click(screen.getByRole('button', { name: 'Preview selected files' }));
    await waitFor(() => expect(api.previewStart).toHaveBeenCalledWith(f.selection, f.project.revision, expect.any(String)));
    expect((await detail(user, nextId)).getByRole('button', { name: 'Review this data copy in native dialog' })).toBeEnabled();
    expect(api.approve).not.toHaveBeenCalled();
  });
  it('retains an exact cancel intent for a legacy failed cancellation with no join receipt', async () => {
    const user = userEvent.setup(), f = dataWorkspaceFixture(), onReady = vi.fn(); await loseCancellation(user, f, onReady);
    const legacy = { ...f.creation, revision: 4, state: 'failed', cancel_requested: true, error: { code: 'CANCEL_REQUESTED', message: 'Old failure receipt', retryable: false, side_effect_state: 'not_started' } };
    vi.mocked(api.creationsList).mockResolvedValue([legacy]); await user.click(refresh());
    expect(screen.getByRole('button', { name: 'Recover the same request' })).toBeInTheDocument();
    expect(screen.getByText(`Cancellation unconfirmed; no stopped receipt · ${f.creation.id}`, { selector: 'summary' })).toBeVisible();
    expect(screen.queryByText(new RegExp(stoppedLabel))).toBeNull(); expect(onReady).not.toHaveBeenCalled();
    expect(api.creationCancel).toHaveBeenCalledOnce(); expect(api.previewStart).not.toHaveBeenCalled(); expect(api.approve).not.toHaveBeenCalled();
    const original = vi.mocked(api.creationCancel).mock.calls[0];
    expect(original).toEqual([f.creation.id, 3, expect.any(String)]);
    vi.mocked(api.creationCancel).mockResolvedValue(legacy);
    await user.click(screen.getByRole('button', { name: 'Recover the same request' }));
    await waitFor(() => expect(api.creationCancel).toHaveBeenCalledTimes(2));
    expect(vi.mocked(api.creationCancel).mock.calls[1]).toEqual(original);
    expect(screen.getByRole('button', { name: 'Recover the same request' })).toBeInTheDocument();
    expect(onReady).not.toHaveBeenCalled();
  });
  it.each(['stale', 'foreign-project', 'wrong-project-revision', 'malformed-io', 'contradictory-ready'] as const)('does not clear pending floors or notify ready on %s', async kind => {
    const user = userEvent.setup(), f = dataWorkspaceFixture(), onReady = vi.fn(); await loseCancellation(user, f, onReady);
    let row = stopped(f);
    if (kind === 'stale') row = { ...row, revision: 2 };
    else if (kind === 'foreign-project') row = { ...row, project_id: 'foreign' };
    else if (kind === 'wrong-project-revision') row = { ...row, project_revision: 5 };
    else if (kind === 'malformed-io') row = stopped(f, { io_completion: { joined: true } });
    else row = { ...f.ready, revision: 4, io_completion: 'not_scheduled' } as DataCreation;
    vi.mocked(api.creationsList).mockResolvedValue([row]); await user.click(refresh());
    expect(await screen.findByRole('alert')).toHaveTextContent('Foreign, stale or malformed');
    expect(screen.getByRole('button', { name: 'Recover the same request' })).toBeDisabled(); expect(onReady).not.toHaveBeenCalled();
    expect(api.creationCancel).toHaveBeenCalledOnce(); expect(api.approve).not.toHaveBeenCalled();
    vi.mocked(api.creationsList).mockResolvedValue([stopped(f)]); await user.click(refresh());
    expect(screen.queryByRole('button', { name: 'Recover the same request' })).toBeNull();
    expect(onReady).not.toHaveBeenCalled();
  });
  it('keeps a ready cancellation race ready without reporting cancellation success', async () => {
    const user = userEvent.setup(), f = dataWorkspaceFixture(), onReady = vi.fn(); await loseCancellation(user, f, onReady);
    vi.mocked(api.creationsList).mockResolvedValue([{ ...f.ready, revision: 4, cancel_requested: true, io_completion: 'worker_joined' } as DataCreation]); await user.click(refresh());
    expect(onReady).toHaveBeenCalledOnce(); expect(screen.queryByRole('button', { name: 'Recover the same request' })).toBeNull();
    expect((await detail(user, f.creation.id)).getByText('Copy completed before cancellation took effect. The ready copy is retained; this is not cancellation success.')).toBeVisible();
    expect(screen.queryByText(new RegExp(stoppedLabel))).toBeNull(); expect(api.previewStart).not.toHaveBeenCalled(); expect(api.approve).not.toHaveBeenCalled();
  });
  it('retains the distinct preview-cancelled wording', async () => {
    const user = userEvent.setup(), f = dataWorkspaceFixture(); vi.mocked(api.previewsList).mockResolvedValue([{ ...f.preview, state: 'cancelled', snapshot: null }]);
    render(<DataWorkspaces project={f.project} language="en" connected />); await open(user);
    expect(screen.getByText(`Preview cancelled; no copy approved · ${f.preview.id}`, { selector: 'summary' })).toBeVisible();
    expect(api.approve).not.toHaveBeenCalled(); expect(api.previewStart).not.toHaveBeenCalled();
  });
});
