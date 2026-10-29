import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { api, type Project } from './api';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const project: Project = { id: 'project-1', name: 'Name', identity: { canonical_root: '/source', device: 1, inode: 2, kind: { kind: 'directory' } }, archived: false, revision: 4, created_at: 'now', updated_at: 'now' };
beforeEach(() => { vi.resetAllMocks(); vi.mocked(invoke).mockResolvedValue({ result: {}, warnings: [] }); });
describe('Narrow desktop command transport', () => {
  it('sends task project binding only when selected', async () => {
    await api.create('Goal', 'key');
    expect(invoke).toHaveBeenLastCalledWith('tasks_create', { goal: 'Goal', idempotencyKey: 'key' });
    await api.create('Goal', 'key2', project.id);
    expect(invoke).toHaveBeenLastCalledWith('tasks_create', { goal: 'Goal', idempotencyKey: 'key2', projectId: project.id });
  });
  it('never roundtrips source identity or supplies a managed destination', async () => {
    await api.projectsArchive(project, 'key');
    expect(invoke).toHaveBeenLastCalledWith('projects_archive', { projectId: project.id, expectedRevision: 4, idempotencyKey: 'key' });
    await api.workspacesCreate(project, 'a'.repeat(40), 'workspace-key');
    expect(invoke).toHaveBeenLastCalledWith('workspaces_create', { projectId: project.id, expectedHead: 'a'.repeat(40), expectedRevision: 4, idempotencyKey: 'workspace-key' });
  });
  it('preserves uncertain side effects instead of treating transport failure as no execution', async () => {
    vi.mocked(invoke).mockResolvedValue({ error: { code: 'OUTCOME_UNKNOWN', message: 'Reconcile', retryable: false, side_effect_state: 'unknown' }, warnings: [] });
    await expect(api.projectsImport('/source', 'Name', 'key')).rejects.toMatchObject({ code: 'OUTCOME_UNKNOWN', sideEffectState: 'unknown' });
  });
});

describe('Read-only code workspace preparation', () => {
  const selection = { task_id: 'task-1', expected_task_revision: 3, expected_project_revision: 4, host_id: 'local' as const };
  const directory = () => ({
    task_id: 'task-1', task_revision: 3, spec_revision: 2, project_id: 'project-1', project_revision: 4, host_id: 'local',
    source: { canonical_root: '/source', device: '9007199254740993', inode: '18446744073709551615', kind: { kind: 'directory' } },
    source_head: null, preparation_key: null, state: 'needs_dataset_selection', reason: 'selected_dataset_required',
    candidates: [], execution_authorized: false, creates_workspace: false,
  });
  const respond = (value: unknown) => vi.mocked(invoke).mockResolvedValue({ result: value, warnings: [] });

  it('uses only the fixed read command and preserves exact opaque filesystem decimal strings', async () => {
    const original = directory(); respond(original);
    const actual = await api.workspacesResolve(selection);
    expect(invoke).toHaveBeenCalledExactlyOnceWith('workspaces_resolve', { selection });
    expect(actual.source?.device).toBe('9007199254740993');
    expect(actual.source?.inode).toBe('18446744073709551615');
    expect(actual).toEqual(original);
    expect(actual.execution_authorized).toBe(false);
    expect(actual.creates_workspace).toBe(false);
  });

  it('handles personal projects, exact Git versions, blocked tasks and busy candidates as metadata', async () => {
    const personal = { ...directory(), project_id: 'local', project_revision: null, source: null,
      state: 'needs_registered_project', reason: 'registered_project_required' };
    respond(personal); expect(await api.workspacesResolve({ ...selection, expected_project_revision: 0 })).toEqual(personal);
    const git = { ...directory(), source: { ...directory().source, device: '0', inode: '0', kind: { kind: 'git', common_dir: '/source/.git', head: 'a'.repeat(40), dirty: true } },
      source_head: 'a'.repeat(40), preparation_key: 'code-prep:' + 'b'.repeat(64), state: 'needs_git_workspace', reason: 'isolated_git_workspace_required',
      candidates: [{ workspace_id: 'ws', revision: 1, root: '/managed/ws', recorded_state: 'ready', has_unreconciled_lease: true, assigned_to_task: false, identity_revalidated: false }] };
    respond(git); expect(await api.workspacesResolve(selection)).toEqual(git);
    const blocked = { ...git, state: 'blocked', reason: 'task_not_draft', preparation_key: null };
    respond(blocked); expect(await api.workspacesResolve(selection)).toEqual(blocked);
  });

  it.each([0, -1, 1.5, Number.MAX_SAFE_INTEGER + 1, NaN, Infinity])('rejects unsafe task revisions before invoke (%s)', async revision => {
    await expect(api.workspacesResolve({ ...selection, expected_task_revision: revision })).rejects.toThrow('contract');
    expect(invoke).not.toHaveBeenCalled();
  });
  it('rejects extra request authority, foreign host and unsafe project revision before invoke', async () => {
    for (const raw of [{ ...selection, path: '/source' }, { ...selection, host_id: 'remote' }, { ...selection, expected_project_revision: Number.MAX_SAFE_INTEGER + 1 }]) {
      await expect(api.workspacesResolve(raw as typeof selection)).rejects.toThrow('contract');
    }
    expect(invoke).not.toHaveBeenCalled();
  });

  it.each([1, '00', '01', '-1', '1.0', '1e3', '', '18446744073709551616', '999999999999999999999'])('rejects a lossy or noncanonical filesystem identity (%s)', async device => {
    respond({ ...directory(), source: { ...directory().source, device } });
    await expect(api.workspacesResolve(selection)).rejects.toThrow('contract');
  });

  it('rejects mismatched selection, unsafe revisions, extra keys, false authority and impossible states', async () => {
    const bad: unknown[] = [
      { ...directory(), task_id: 'other-task' }, { ...directory(), task_revision: 4 }, { ...directory(), project_revision: 5 },
      { ...directory(), spec_revision: Number.MAX_SAFE_INTEGER + 1 }, { ...directory(), execution_authorized: true },
      { ...directory(), creates_workspace: true }, { ...directory(), host_id: 'remote' }, { ...directory(), grant_id: 'not-authority' },
      { ...directory(), state: 'ready' }, { ...directory(), reason: 'task_not_draft' }, { ...directory(), source_head: 'a'.repeat(40) },
      { ...directory(), source: { ...directory().source, kind: { kind: 'directory', head: 'extra' } } },
      { ...directory(), source: { ...directory().source, canonical_root: 'relative' } },
      { ...directory(), source: { ...directory().source, canonical_root: '/source\0bad' } },
      { ...directory(), source: { ...directory().source, inode: 1 } },
      { ...directory(), source: { ...directory().source, device: '1', inode: '01' } },
      { ...directory(), candidates: Array(1001).fill({}) },
    ];
    for (const value of bad) { respond(value); await expect(api.workspacesResolve(selection)).rejects.toThrow('contract'); }
  });

  it('never accepts candidate assignment, identity revalidation, unknown status or duplicate IDs', async () => {
    const candidate = { workspace_id: 'ws', revision: 1, root: '/managed/ws', recorded_state: 'ready', has_unreconciled_lease: true, assigned_to_task: false, identity_revalidated: false };
    for (const candidates of [
      [{ ...candidate, assigned_to_task: true }], [{ ...candidate, identity_revalidated: true }],
      [{ ...candidate, recorded_state: 'assigned' }], [{ ...candidate, revision: Number.MAX_SAFE_INTEGER + 1 }],
      [{ ...candidate, root: 'relative' }], [candidate, candidate],
    ]) { respond({ ...directory(), candidates }); await expect(api.workspacesResolve(selection)).rejects.toThrow('contract'); }
  });

  it('preserves Core errors and does not create or finalize on refusal', async () => {
    vi.mocked(invoke).mockResolvedValue({ error: { code: 'STALE_REVISION', message: 'Refresh', retryable: false, side_effect_state: 'not_started' }, warnings: [] });
    await expect(api.workspacesResolve(selection)).rejects.toMatchObject({ code: 'STALE_REVISION', sideEffectState: 'not_started' });
    expect(invoke).toHaveBeenCalledExactlyOnceWith('workspaces_resolve', { selection });
  });
});
