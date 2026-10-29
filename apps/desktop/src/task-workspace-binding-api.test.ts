import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { api } from './api';
import type { TaskGitWorkspacePreparation } from './task-workspace-binding-wire';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const pin = () => ({ canonical_root: '/source', device: '9007199254740993', inode: '18446744073709551615', common_dir: '/source/.git', head: 'a'.repeat(40) });
const selection = (): TaskGitWorkspacePreparation => ({ task_id: 'task-1', expected_task_revision: 1, expected_spec_revision: 2, expected_project_revision: 3, host_id: 'local', expected_source_pin: pin(), preparation_key: 'code-prep:' + 'b'.repeat(64) });
const receipt = () => ({ version: 1, binding_id: 'binding-1', workspace_id: 'workspace-1', task_id: 'task-1', project_id: 'project-1', host_id: 'local', original_task_revision: 1, reserved_task_revision: 2, spec_revision: 2, project_revision: 3, source_pin: pin(), managed_root: '/managed', preparation_key: selection().preparation_key, request_key: 'stable-key', request_digest: 'c'.repeat(64), created_at: '2026-10-29T00:00:00Z' });
const managed = () => ({ ...pin(), canonical_root: '/managed/workspace-1', inode: '9007199254740995' });
const binding = () => ({ receipt: receipt(), state: 'ready', revision: 2, managed_pin: managed() });
const view = () => ({ original_receipt: receipt(), binding: binding(), task_revision: 3, observed_managed_pin: managed(), identity_revalidated: true, execution_authorized: false });
const task = () => ({ id: 'task-1', project_id: 'project-1', goal: 'Generated Task', state: 'draft', revision: 3, spec_revision: 2, created_at: '2026-10-29T00:00:00Z', updated_at: '2026-10-29T00:00:01Z', code_workspace_binding: binding() });
const respond = (value: unknown) => vi.mocked(invoke).mockResolvedValue({ result: value, warnings: [] });
beforeEach(() => vi.resetAllMocks());

describe('Task Git workspace fixed Owner transport', () => {
  it('sends exact original selection/key and retains large FS integers as opaque strings', async () => {
    const selected = selection(), reply = view(); respond(reply);
    const actual = await api.workspacesPrepareTaskGit(selected, 'stable-key');
    expect(invoke).toHaveBeenCalledExactlyOnceWith('workspaces_prepare_task_git', { selection: selected, idempotencyKey: 'stable-key' });
    expect(actual).toEqual(reply); expect(actual.original_receipt.source_pin.device).toBe('9007199254740993'); expect(actual.original_receipt.source_pin.inode).toBe('18446744073709551615');
    expect(actual.execution_authorized).toBe(false); expect(actual.identity_revalidated).toBe(true);
  });

  it('accepts later Task revision and legitimate output HEAD without altering the immutable receipt', async () => {
    const reply = { ...view(), task_revision: 9, observed_managed_pin: { ...managed(), head: 'd'.repeat(40) } }; respond(reply);
    expect(await api.workspacesPrepareTaskGit(selection(), 'stable-key')).toEqual(reply);
  });

  it('does not silently retry a lost response or rebuild original payload with the current revision', async () => {
    const selected = selection(); vi.mocked(invoke).mockRejectedValueOnce(new Error('generated transport disconnect'));
    await expect(api.workspacesPrepareTaskGit(selected, 'stable-key')).rejects.toThrow('transport disconnect'); expect(invoke).toHaveBeenCalledTimes(1);
    respond(view()); expect((await api.workspacesPrepareTaskGit(selected, 'stable-key')).original_receipt.original_task_revision).toBe(1);
    expect(invoke).toHaveBeenNthCalledWith(2, 'workspaces_prepare_task_git', { selection: selected, idempotencyKey: 'stable-key' });
  });

  it('retains Core side-effect uncertainty and never falls back to unbound creation or approval', async () => {
    vi.mocked(invoke).mockResolvedValue({ error: { code: 'OUTCOME_UNKNOWN', message: 'Reconcile original intent', retryable: false, side_effect_state: 'unknown' }, warnings: [] });
    await expect(api.workspacesPrepareTaskGit(selection(), 'stable-key')).rejects.toMatchObject({ code: 'OUTCOME_UNKNOWN', sideEffectState: 'unknown' });
    expect(invoke).toHaveBeenCalledExactlyOnceWith('workspaces_prepare_task_git', { selection: selection(), idempotencyKey: 'stable-key' });
  });

  it('rejects unsafe revisions, extra path/Grant fields, foreign host and invalid keys before invoke', async () => {
    const inputs = [
      { ...selection(), expected_task_revision: 0 }, { ...selection(), expected_task_revision: Number.MAX_SAFE_INTEGER + 1 },
      { ...selection(), expected_spec_revision: 1.5 }, { ...selection(), expected_project_revision: false },
      { ...selection(), host_id: 'remote' }, { ...selection(), managed_root: '/injected' }, { ...selection(), grant_id: 'not-authority' },
    ];
    for (const input of inputs) await expect(api.workspacesPrepareTaskGit(input as TaskGitWorkspacePreparation, 'stable-key')).rejects.toThrow('contract');
    for (const key of ['', 'x'.repeat(257), '界'.repeat(100), null]) await expect(api.workspacesPrepareTaskGit(selection(), key as string)).rejects.toThrow('contract');
    expect(invoke).not.toHaveBeenCalled();
  });

  it.each([1, '01', '-1', '1.0', '18446744073709551616', '1\n', '1\r', '1\u2028', '1\u2029'])('rejects noncanonical or lossy source identity %s before invoke', async value => {
    const selected = { ...selection(), expected_source_pin: { ...pin(), device: value } };
    await expect(api.workspacesPrepareTaskGit(selected as TaskGitWorkspacePreparation, 'stable-key')).rejects.toThrow('contract'); expect(invoke).not.toHaveBeenCalled();
  });

  it('rejects false-ready, missing managed identity, role/authority flags and malformed receipt bindings', async () => {
    const bad: unknown[] = [
      { ...view(), identity_revalidated: false }, { ...view(), execution_authorized: true }, { ...view(), execution_authorized: 0 },
      { ...view(), binding: { ...binding(), state: 'pending', managed_pin: null } },
      { ...view(), binding: { ...binding(), state: 'outcome_unknown' } }, { ...view(), binding: { ...binding(), managed_pin: null } },
      { ...view(), observed_managed_pin: null }, { ...view(), observed_managed_pin: { ...managed(), inode: '1' } },
      { ...view(), original_receipt: { ...receipt(), task_id: 'other-task' } },
      { ...view(), binding: { ...binding(), receipt: { ...receipt(), request_digest: 'd'.repeat(64) } } },
      { ...view(), task_revision: Number.MAX_SAFE_INTEGER + 1 }, { ...view(), task_revision: 1 },
      { ...view(), grant_id: 'not-authority' },
    ];
    for (const reply of bad) { respond(reply); await expect(api.workspacesPrepareTaskGit(selection(), 'stable-key')).rejects.toThrow('contract'); }
  });

  it('rejects semantically equal-looking altered original receipt/key/CAS/paths even when both receipt copies agree', async () => {
    const alterations = [{ original_task_revision: 2, reserved_task_revision: 3 }, { request_key: 'new-key' }, { spec_revision: 3 }, { project_revision: 4 }, { preparation_key: 'code-prep:' + 'd'.repeat(64) }, { workspace_id: '../escape' }];
    for (const change of alterations) {
      const r = { ...receipt(), ...change }; respond({ ...view(), original_receipt: r, binding: { ...binding(), receipt: r } });
      await expect(api.workspacesPrepareTaskGit(selection(), 'stable-key')).rejects.toThrow('contract');
    }
  });
});

describe('Single stored Task read is not fresh workspace or execution authority', () => {
  it('uses fixed tasks_get without a key or auto-write and keeps pending/unknown metadata inspectable', async () => {
    for (const state of ['pending', 'outcome_unknown']) {
      const reply = { ...task(), spec_revision: 4, code_workspace_binding: { ...binding(), state, managed_pin: null } }; respond(reply);
      const actual = await api.taskGet('task-1'); expect(actual).toEqual(reply); expect(actual).not.toHaveProperty('identity_revalidated'); expect(actual).not.toHaveProperty('execution_authorized');
    }
    expect(invoke).toHaveBeenLastCalledWith('tasks_get', { taskId: 'task-1' });
  });

  it('preserves legacy absent/null binding and validates the current Task identity', async () => {
    const { code_workspace_binding: _binding, ...legacy } = task(); respond(legacy); expect(await api.taskGet('task-1')).toEqual(legacy);
    respond({ ...legacy, code_workspace_binding: null }); expect((await api.taskGet('task-1')).code_workspace_binding).toBeNull();
    respond({ ...legacy, id: 'other-task' }); await expect(api.taskGet('task-1')).rejects.toThrow('contract');
    await expect(api.taskGet('')).rejects.toThrow('contract');
  });

  it('rejects malformed stored projection and cannot promote a binding into TaskAccepted', async () => {
    for (const reply of [
      { ...task(), revision: Number.MAX_SAFE_INTEGER + 1 }, { ...task(), state: 'accepted' },
      { ...task(), code_workspace_binding: { ...binding(), receipt: { ...receipt(), task_id: 'other-task' } } },
      { ...task(), code_workspace_binding: { ...binding(), managed_pin: { ...managed(), device: 1 } } },
      { ...task(), execution_authorized: true },
    ]) { respond(reply); await expect(api.taskGet('task-1')).rejects.toThrow('contract'); }
  });
});
