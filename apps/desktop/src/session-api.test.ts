import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { sessionApi, type ManagedSession } from './session-api';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
beforeEach(() => { vi.resetAllMocks(); vi.mocked(invoke).mockResolvedValue({ result: {}, warnings: [] }); });
describe('Session transport sends only typed user intent to narrow native commands', () => {
  it('registration does not supply a trusted approval or resolved execution binding', async () => {
    await sessionApi.connectionsRegister('Name', '/explicit/program', ['--acp'], 'key');
    expect(invoke).toHaveBeenCalledWith('connections_register', { name: 'Name', executable: '/explicit/program', arguments: ['--acp'], provider: 'custom_acp', idempotencyKey: 'key' });
  });
  it('native selection passes only the explicit Provider choice, without inventing adapter flags', async () => {
    await sessionApi.connectionsRegister('Native', '/selected/codex', [], 'native-key', 'codex');
    expect(invoke).toHaveBeenCalledWith('connections_register', { name: 'Native', executable: '/selected/codex', arguments: [], provider: 'codex', idempotencyKey: 'native-key' });
  });
  it('grant creation uses camelCase Tauri arguments and omits renderer authority claims', async () => {
    await sessionApi.grantsCreate({ project_id: 'p', workspace_id: 'w', connection_id: 'c', max_attempts: 2, max_concurrency: 1, max_duration_ms: 9000 }, 'key');
    expect(invoke).toHaveBeenCalledWith('grants_create', { projectId: 'p', workspaceId: 'w', connectionId: 'c', maxAttempts: 2, maxConcurrency: 1, maxDurationMs: 9000, idempotencyKey: 'key' });
  });
  it('session creation sends selected references rather than resolved bindings', async () => {
    await sessionApi.sessionsCreate('task', 3, 'workspace', 'connection', 'grant', 'key');
    expect(invoke).toHaveBeenCalledWith('sessions_create', { taskId: 'task', expectedRevision: 3, workspaceId: 'workspace', connectionId: 'connection', grantId: 'grant', idempotencyKey: 'key' });
  });
  it('native denial propagates without automatic retries or finalize exposed to JS', async () => {
    vi.mocked(invoke).mockResolvedValue({ error: { code: 'PERMISSION_DENIED', message: 'Native owner confirmation was declined', retryable: false, side_effect_state: 'not_started' }, warnings: [] });
    await expect(sessionApi.connectionsRegister('Name', '/explicit/program', [], 'key')).rejects.toMatchObject({ code: 'PERMISSION_DENIED', sideEffectState: 'not_started' });
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(Object.keys(sessionApi).some(key => key.includes('finalize') || key.includes('approve'))).toBe(false);
  });
});
