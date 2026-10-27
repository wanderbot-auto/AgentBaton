import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { commandValidationApi as api, enforcementKeys, newCommandForm, parseCommandSelection } from './command-validation-api';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
beforeEach(() => { vi.resetAllMocks(); vi.mocked(invoke).mockResolvedValue({ result: {}, warnings: [] }); });
const form = () => ({ ...newCommandForm('session'), executable: '/usr/bin/printf', inputs: `input.txt | ${'a'.repeat(64)}`, outputs: 'result.txt' });
describe('Command native contracts with mocked invoke, not execution authority', () => {
  it('sends only the proposal selection, never a program hash/root/subject or renderer approval claim', async () => {
    const selection = parseCommandSelection(form()); await api.approve(selection, 'approval-key');
    expect(invoke).toHaveBeenCalledWith('command_validation_policy_approve', { selection, idempotencyKey: 'approval-key' });
    expect(selection.mode).toBe('native_executable'); expect(selection.script).toBeNull(); expect(selection.environment).toEqual({});
    expect(JSON.stringify(vi.mocked(invoke).mock.calls)).not.toMatch(/canonical_root|program_sha256|subject|approved|finalize|attempt_id/);
  });
  it('uses seven dedicated native commands and exact policy/run revision and fixed keys', async () => {
    await api.policiesList('task'); await api.recordsList('task'); await api.get('run'); await api.run('policy', 2, 'run-key'); await api.cancel('run', 3, 'cancel-key'); await api.revoke('policy', 2, 'revoke-key');
    expect(vi.mocked(invoke).mock.calls).toEqual([
      ['command_validation_policies_list', { taskId: 'task' }], ['command_validations_list', { taskId: 'task' }], ['command_validations_get', { runId: 'run' }], ['command_validations_run', { policyId: 'policy', expectedRevision: 2, idempotencyKey: 'run-key' }], ['command_validations_cancel', { runId: 'run', expectedRevision: 3, idempotencyKey: 'cancel-key' }], ['command_validation_policies_revoke', { policyId: 'policy', expectedRevision: 2, idempotencyKey: 'revoke-key' }],
    ]);
    vi.mocked(invoke).mockResolvedValue({ error: { code: 'INVALID_APPROVAL_PROPOSAL', message: 'Snapshot too large', side_effect_state: 'not_started' } }); await expect(api.approve(parseCommandSelection(form()), 'key')).rejects.toMatchObject({ sideEffectState: 'not_started' }); expect(invoke).toHaveBeenCalledTimes(7);
    vi.mocked(invoke).mockRejectedValue(new Error('Lost run response')); await expect(api.run('p', 1, 'same-key')).rejects.toThrow('Lost run response'); expect(invoke).toHaveBeenCalledTimes(8);
  });
  it('pins script bytes separately while preserving literal argv, empty arguments and constant-only environment', () => {
    const f = { ...form(), mode: 'posix_sh_stdin' as const, executable: '/bin/sh', scriptPath: 'checks/test.sh', scriptHash: 'B'.repeat(64), arguments: '--value=$HOME\n"literal spaces"\n', lang: 'C.UTF-8', lcAll: 'C', timezone: true };
    const selection = parseCommandSelection(f); expect(selection.arguments).toEqual(['--value=$HOME', '"literal spaces"', '']); expect(selection.script).toEqual({ artifact_id: 'script', relative_path: 'checks/test.sh', max_bytes: 65536, expected_sha256: 'b'.repeat(64) }); expect(selection.environment).toEqual({ LANG: 'C.UTF-8', LC_ALL: 'C', TZ: 'UTC' });
    for (const key of enforcementKeys) { f.require[key] = true; expect(parseCommandSelection(f).require[key]).toBe(true); }
  });
  it.each(['API_KEY=secret', 'secret', 'en_US.UTF-8\nPRIVATE_KEY=anything'])('refuses unsupported environment value %s', value => { expect(() => parseCommandSelection({ ...form(), lang: value })).toThrow('environment'); expect(() => parseCommandSelection({ ...form(), lcAll: value })).toThrow('environment'); expect(invoke).not.toHaveBeenCalled(); });
  it.each(['../escape', '/private/file', 'a/../b', './b', 'a//b', 'C:\\file', 'a\\b', 'name\u0000.txt', '中'.repeat(81)])('rejects unsafe path %s and duplicate roles before native approval', outputs => { expect(() => parseCommandSelection({ ...form(), outputs })).toThrow('path'); });
  it('requires fixed hashes, bounded input/output counts and unique roles', () => {
    expect(() => parseCommandSelection({ ...form(), inputs: 'input.txt' })).toThrow('hash'); expect(() => parseCommandSelection({ ...form(), outputs: 'input.txt' })).toThrow('path');
    expect(() => parseCommandSelection({ ...form(), inputs: Array.from({ length: 16 }, (_, i) => `i${i}|${'a'.repeat(64)}`).join('\n') })).toThrow('limit');
    expect(() => parseCommandSelection({ ...form(), outputs: Array.from({ length: 17 }, (_, i) => `o${i}`).join('\n') })).toThrow('limit');
    expect(() => parseCommandSelection({ ...form(), executable: 'relative-program' })).toThrow('path'); expect(() => parseCommandSelection({ ...form(), mode: 'posix_sh_stdin', scriptPath: 'script', scriptHash: '' })).toThrow('hash');
  });
  it.each([['wall','0'],['wall','60001'],['cleanup','1001'],['stdout','65537'],['stderr','-1'],['cap','4194305'],['wall','9007199254740993'],['exits','256'],['exits','0,0'],['wall','1e3']] as const)('rejects numeric bound %s=%s without rounding', (field, value) => { expect(() => parseCommandSelection({ ...form(), [field]: value })).toThrow('limit'); });
  it('enforces UTF8 argument/total-file/selection budgets without truncation', () => {
    expect(() => parseCommandSelection({ ...form(), arguments: '中'.repeat(1366) })).toThrow('arguments');
    expect(() => parseCommandSelection({ ...form(), cap: '4194304', outputs: 'a\nb\nc\nd' })).toThrow('limit');
    expect(() => parseCommandSelection({ ...form(), arguments: Array.from({ length: 65 }, () => 'a').join('\n') })).toThrow('arguments');
    expect(() => parseCommandSelection({ ...form(), arguments: Array.from({ length: 4 }, () => 'x'.repeat(4096)).join('\n') })).toThrow('snapshot');
  });
});
