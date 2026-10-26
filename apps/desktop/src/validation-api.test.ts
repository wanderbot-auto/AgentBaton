import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { parseValidationSelection, validationApi } from './validation-api';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
beforeEach(() => { vi.resetAllMocks(); vi.mocked(invoke).mockResolvedValue({ result: {}, warnings: [] }); });
describe('Artifact integrity narrow transport (mocked invoke, not native evidence)', () => {
  it('approval sends selected relative contracts without roots, approval flags or resolved subject', async () => {
    const selection = parseValidationSelection('session-1', '', 'reports/result.txt', 8);
    await validationApi.policiesApprove(selection, 'approve-key');
    expect(invoke).toHaveBeenCalledWith('validation_policy_approve', { selection: { session_id: 'session-1', inputs: [], outputs: [{ artifact_id: 'output-1', relative_path: 'reports/result.txt', max_bytes: 8_388_608, expected_sha256: null }], analysis: null }, idempotencyKey: 'approve-key' });
    expect(JSON.stringify(vi.mocked(invoke).mock.calls)).not.toMatch(/canonical_root|approved|finalize|task_id|attempt_id|spec_revision/);
  });
  it('list/run/revoke use exact camelCase native arguments and no authority roundtrip', async () => {
    await validationApi.policiesList('task'); await validationApi.recordsList('task');
    await validationApi.run('policy', 7, 'run-key'); await validationApi.revoke('policy', 7);
    expect(invoke).toHaveBeenNthCalledWith(1, 'validation_policies_list', { taskId: 'task' });
    expect(invoke).toHaveBeenNthCalledWith(2, 'validations_list', { taskId: 'task' });
    expect(invoke).toHaveBeenNthCalledWith(3, 'validations_run', { policyId: 'policy', expectedRevision: 7, idempotencyKey: 'run-key' });
    expect(invoke).toHaveBeenNthCalledWith(4, 'validation_policies_revoke', { policyId: 'policy', expectedRevision: 7 });
    expect(Object.keys(validationApi).some(key => /finalize|accept|adopt/i.test(key))).toBe(false);
  });
  it('native denial and unknown outcomes propagate without retry', async () => {
    vi.mocked(invoke).mockResolvedValue({ error: { code: 'PERMISSION_DENIED', message: 'Owner declined', retryable: false, side_effect_state: 'not_started' }, warnings: [] });
    await expect(validationApi.policiesApprove(parseValidationSelection('s', '', 'report', 1), 'key')).rejects.toMatchObject({ code: 'PERMISSION_DENIED', sideEffectState: 'not_started' });
    expect(invoke).toHaveBeenCalledTimes(1);
    vi.mocked(invoke).mockRejectedValue(new Error('Unknown transport outcome'));
    await expect(validationApi.run('p', 1, 'key')).rejects.toThrow('Unknown transport outcome'); expect(invoke).toHaveBeenCalledTimes(2);
  });
});
describe('Bounded artifact form parsing (usability checks, not filesystem authorization)', () => {
  it('supports Unicode/space paths and fixed SHA inputs with separate IDs', () => {
    const hash = 'A'.repeat(64), selection = parseValidationSelection('session', `输入 数据.csv | ${hash}`, '结果 报告.md', 2);
    expect(selection.inputs[0]).toEqual({ artifact_id: 'input-1', relative_path: '输入 数据.csv', max_bytes: 2_097_152, expected_sha256: hash.toLowerCase() });
    expect(selection.outputs[0].artifact_id).toEqual('output-1'); expect(selection.outputs[0].expected_sha256).toBeNull();
  });
  it.each(['../escape', '/private/file', 'nested/../escape', './report', 'a//b', 'C:\\secret', 'a\\b', 'report\u0000.txt'])('refuses unsafe path %s before transport', path => {
    expect(() => parseValidationSelection('s', '', path, 1)).toThrow('path'); expect(invoke).not.toHaveBeenCalled();
  });
  it('refuses duplicate paths, missing input digest and malformed digest', () => {
    expect(() => parseValidationSelection('s', '', 'same\nsame', 1)).toThrow('duplicate');
    expect(() => parseValidationSelection('s', 'data.csv', 'report', 1)).toThrow('digest');
    expect(() => parseValidationSelection('s', '', 'report | short', 1)).toThrow('digest');
  });
  it('enforces combined count, UTF-8 path budget and integer per-file cap', () => {
    expect(() => parseValidationSelection('s', '', Array.from({ length: 17 }, (_, index) => `file${index}`).join('\n'), 1)).toThrow('limit');
    expect(() => parseValidationSelection('s', '', '中'.repeat(81), 1)).toThrow('path');
    for (const cap of [0, 1.5, 9, Number.NaN]) expect(() => parseValidationSelection('s', '', 'report', cap)).toThrow('limit');
  });
});
