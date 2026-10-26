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
