import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { dataValidationApi, newDataForm, parseDataSelection, type DataForm } from './data-validation-api';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
beforeEach(() => { vi.resetAllMocks(); vi.mocked(invoke).mockResolvedValue({ result: {}, warnings: [] }); });
function form(): DataForm { return { ...newDataForm('session'), inputs: `source.csv | ${'A'.repeat(64)}`, tablePath: 'table.json', scriptPath: 'reproduce.py', environmentPath: 'environment.json', datasetRows: '2', columns: [{ name: 'value', type: 'number', required: true, nullable: false, min: '-1.5', max: '2e3', unit: 'm', maxBytes: '32' }] }; }
describe('Data contract transport and form bounds; mocked Tauri, not native authority', () => {
  it('sends only an explicit bounded selection and fixed IDs to the dedicated namespace', async () => {
    const selection = parseDataSelection(form()); await dataValidationApi.approve(selection, 'approve-key');
    expect(invoke).toHaveBeenCalledWith('data_validation_policy_approve', { selection, idempotencyKey: 'approve-key' });
    expect(selection.outputs.map(item => item.artifact_id)).toEqual(['table', 'script', 'environment']); expect(selection.inputs[0].expected_sha256).toBe('a'.repeat(64));
    expect(selection.table.analysis).toEqual({ script_artifact_id: 'script', environment_artifact_id: 'environment', timezone: 'UTC', units: { value: 'm' }, seed: null, scope: { kind: 'full' } });
    expect(JSON.stringify(vi.mocked(invoke).mock.calls)).not.toMatch(/canonical_root|subject|approval_id|task_id|attempt_id|finalize|accepted/);
  });
  it('keeps reads/run/revoke separate, with exact policy revision and no auto-retry', async () => {
    await dataValidationApi.policiesList('t'); await dataValidationApi.recordsList('t'); await dataValidationApi.run('p', 3, 'key'); await dataValidationApi.revoke('p', 3);
    expect(vi.mocked(invoke).mock.calls).toEqual([['data_validation_policies_list', { taskId: 't' }], ['data_validations_list', { taskId: 't' }], ['data_validations_run', { policyId: 'p', expectedRevision: 3, idempotencyKey: 'key' }], ['data_validation_policies_revoke', { policyId: 'p', expectedRevision: 3 }]]);
    vi.mocked(invoke).mockResolvedValue({ error: { code: 'PERMISSION_DENIED', message: 'Native declined', side_effect_state: 'not_started' } });
    await expect(dataValidationApi.approve(parseDataSelection(form()), 'declined')).rejects.toMatchObject({ code: 'PERMISSION_DENIED' }); expect(invoke).toHaveBeenCalledTimes(5);
    vi.mocked(invoke).mockRejectedValue(new Error('Unknown')); await expect(dataValidationApi.run('p', 3, 'same')).rejects.toThrow('Unknown'); expect(invoke).toHaveBeenCalledTimes(6);
  });
  it.each(['../escape', '/secret', 'a/../b', './b', 'a//b', 'C:\\a', 'a\\b', 'bad\u0000name', '中'.repeat(81)])('rejects unsafe or oversized path %s', path => { expect(() => parseDataSelection({ ...form(), tablePath: path })).toThrow('path'); expect(invoke).not.toHaveBeenCalled(); });
  it('requires fixed SHA inputs and unique input/output paths across all roles', () => {
    for (const inputs of ['', 'file', 'file|bad', `file|${'a'.repeat(64)}|extra`]) expect(() => parseDataSelection({ ...form(), inputs })).toThrow();
    expect(() => parseDataSelection({ ...form(), tablePath: 'source.csv' })).toThrow('path');
    expect(() => parseDataSelection({ ...form(), scriptPath: 'table.json' })).toThrow('path');
  });
  it.each(['9007199254740992', '9007199254740993', '9007199254740991.1', '1e3', '-1', 'NaN', ''])('never silently rounds count or seed %s', value => {
    expect(() => parseDataSelection({ ...form(), datasetRows: value })).toThrow();
    if (value !== '') expect(() => parseDataSelection({ ...form(), seed: value })).toThrow();
  });
  it('supports exact safe signed integer bounds and maximum safe seed, refusing unsafe integer min', () => {
    const f = form(); f.columns[0] = { ...f.columns[0], type: 'integer', min: '-9007199254740991', max: '9007199254740991' }; f.seed = '9007199254740991';
    const selection = parseDataSelection(f); expect(selection.table.columns[0].scalar).toEqual({ type: 'integer', min: Number.MIN_SAFE_INTEGER, max: Number.MAX_SAFE_INTEGER }); expect(selection.table.analysis.seed).toBe(Number.MAX_SAFE_INTEGER);
    f.columns[0].min = '-9007199254740992'; expect(() => parseDataSelection(f)).toThrow('integer');
  });
  it.each(['', 'NaN', 'Infinity', '1e999', '0xFF'])('refuses non-finite or non-JSON number bound %s', min => { const f = form(); f.columns[0].min = min; expect(() => parseDataSelection(f)).toThrow('range'); });
  it('enforces column names, uniqueness, numeric units and string bounds', () => {
    for (const name of ['x y', '2x', '中文', 'x'.repeat(65)]) { const f = form(); f.columns[0].name = name; expect(() => parseDataSelection(f)).toThrow('column'); }
    const f = form(); f.columns.push({ ...f.columns[0] }); expect(() => parseDataSelection(f)).toThrow('column');
    f.columns.pop(); f.columns[0].unit = ''; expect(() => parseDataSelection(f)).toThrow('metadata');
    f.columns[0].unit = '中'.repeat(43); expect(() => parseDataSelection(f)).toThrow('metadata');
    f.columns[0].unit = 'm'; f.columns.push({ ...f.columns[0], name: 'label', type: 'string', maxBytes: '4097' }); expect(() => parseDataSelection(f)).toThrow('limit');
    f.columns = [{ ...f.columns[0], type: 'boolean' }]; expect(() => parseDataSelection(f)).toThrow('metadata');
  });
  it('preserves prototype-named numeric units as own data and finite binary64 semantics', () => {
    const f = form(); f.columns[0].name = '__proto__'; f.columns[0].min = '0.1';
    expect(JSON.parse(JSON.stringify(parseDataSelection(f))).table.analysis.units).toEqual({ ['__proto__']: 'm' });
  });
  it('enforces full/sample population limits without silently upgrading sample scope', () => {
    const f = { ...form(), scope: 'sample' as const, datasetRows: '100000', sampleRows: '2', limitation: 'Selected subset only' };
    expect(parseDataSelection(f).table.analysis.scope).toEqual({ kind: 'sample', rows: 2, limitation: f.limitation });
    for (const patch of [{ limitation: '' }, { sampleRows: '0' }, { sampleRows: '100001' }, { sampleRows: '10001' }]) expect(() => parseDataSelection({ ...f, ...patch })).toThrow();
    expect(() => parseDataSelection({ ...f, scope: 'full' })).toThrow('sample');
  });
  it('enforces every read bound, input count, total bytes and explicit selection snapshot', () => {
    const f = form();
    for (const [field, value] of [['maxBytes', '4194305'], ['maxDepth', '33'], ['maxValues', '200001'], ['maxStringBytes', '16385'], ['maxRows', '20001']] as const) expect(() => parseDataSelection({ ...f, [field]: value })).toThrow('limit');
    const inputs = Array.from({ length: 17 }, (_, i) => `i${i}|${'a'.repeat(64)}`).join('\n'); expect(() => parseDataSelection({ ...f, inputs })).toThrow('limit');
    expect(() => parseDataSelection({ ...f, maxBytes: '4194304', inputs: `a|${'a'.repeat(64)}\nb|${'b'.repeat(64)}` })).toThrow('limit');
    const large = form(); large.columns = Array.from({ length: 64 }, (_, i) => ({ ...large.columns[0], name: `column_${i}`, unit: 'm'.repeat(128) })); expect(() => parseDataSelection(large)).toThrow('snapshot');
  });
});
