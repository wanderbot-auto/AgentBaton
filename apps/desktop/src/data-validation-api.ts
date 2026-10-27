import { request } from './api';
import type { ArtifactRequirement, ValidationContract, ValidationRecord } from './validation-api';

export const dataValidator = 'builtin.data-json-table.v1';
export interface JsonLimits { max_bytes: number; max_depth: number; max_values: number; max_string_bytes: number; max_rows: number }
export type Scalar = { type: 'number' | 'integer'; min: number; max: number } | { type: 'string'; max_bytes: number } | { type: 'boolean' };
export interface JsonColumn { name: string; required: boolean; nullable: boolean; scalar: Scalar }
export interface JsonTableContract {
  table_artifact_id: 'table'; dataset_rows: number; columns: JsonColumn[];
  analysis: { script_artifact_id: 'script'; environment_artifact_id: 'environment'; units: Record<string, string>; timezone: string; seed: number | null; scope: { kind: 'full' } | { kind: 'sample'; rows: number; limitation: string } };
  limits: JsonLimits;
}
export interface DataSelection { session_id: string; inputs: ArtifactRequirement[]; outputs: ArtifactRequirement[]; table: JsonTableContract }
export interface DataPolicy {
  id: string; task_id: string; project_id: string; session_id: string; attempt_id: string; workspace_id: string;
  approval_id: string; revision: number; revoked: boolean; created_at: string; scope: string; validator_id: string;
  file_contract: ValidationContract; table: JsonTableContract; contract_digest: string;
}
export interface DataRecord {
  id: string; policy_id: string; task_id: string; state: string; scope: string; correctness: string;
  task_acceptance: string; integration_authorized: boolean; created_at: string;
  evidence: { validator_id: string; approval_id: string; policy_revision: string;
    contract: { validator_id: string; artifact_policy: ValidationContract; table: JsonTableContract };
    contract_digest: string; file_integrity: ValidationRecord['evidence']; table_sha256: string | null;
    checked_rows: number | null; failure: { code: string; pointer: string; line: number | null; column: number | null } | null; passed: boolean; digest: string };
}
export interface ColumnForm { name: string; type: 'number' | 'integer' | 'string' | 'boolean'; required: boolean; nullable: boolean; min: string; max: string; maxBytes: string; unit: string }
export interface DataForm {
  sessionId: string; inputs: string; tablePath: string; scriptPath: string; environmentPath: string;
  datasetRows: string; scope: 'full' | 'sample'; sampleRows: string; limitation: string; timezone: string; seed: string;
  columns: ColumnForm[]; maxBytes: string; maxDepth: string; maxValues: string; maxStringBytes: string; maxRows: string;
}
export function newColumn(): ColumnForm { return { name: '', type: 'number', required: true, nullable: false, min: '', max: '', maxBytes: '256', unit: '' }; }
export function newDataForm(sessionId = ''): DataForm { return { sessionId, inputs: '', tablePath: '', scriptPath: '', environmentPath: '', datasetRows: '', scope: 'full', sampleRows: '', limitation: '', timezone: 'UTC', seed: '', columns: [newColumn()], maxBytes: '1048576', maxDepth: '16', maxValues: '100000', maxStringBytes: '4096', maxRows: '10000' }; }
export class DataFormError extends Error { constructor(public reason: 'path' | 'digest' | 'limit' | 'integer' | 'column' | 'range' | 'metadata' | 'sample' | 'empty' | 'snapshot') { super(reason); } }
const bytes = (value: string) => new TextEncoder().encode(value).length;
function integer(value: string, min = 0, max = Number.MAX_SAFE_INTEGER): number {
  if (!/^-?(0|[1-9]\d*)$/.test(value) || !Number.isSafeInteger(Number(value))) throw new DataFormError('integer');
  const number = Number(value); if (number < min || number > max) throw new DataFormError('limit'); return number;
}
function number(value: string): number {
  if (!/^-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?$/.test(value) || !Number.isFinite(Number(value))) throw new DataFormError('range');
  return Number(value);
}
function relative(value: string): string {
  const path = value.trim();
  if (!path || path.startsWith('/') || /^[A-Za-z]:/.test(path) || /[\\\u0000-\u001f\u007f]/.test(path) || bytes(path) > 240 || path.split('/').some(part => !part || part === '.' || part === '..')) throw new DataFormError('path');
  return path;
}
/** Usability checks only. Roots, subject, stop receipts and authority remain server-resolved. */
export function parseDataSelection(form: DataForm): DataSelection {
  if (!form.sessionId) throw new DataFormError('empty');
  const limits = { max_bytes: integer(form.maxBytes, 1, 4194304), max_depth: integer(form.maxDepth, 1, 32), max_values: integer(form.maxValues, 1, 200000), max_string_bytes: integer(form.maxStringBytes, 1, 16384), max_rows: integer(form.maxRows, 1, 20000) };
  const lines = form.inputs.split(/\r?\n/).map(line => line.trim()).filter(Boolean);
  if (!lines.length) throw new DataFormError('empty');
  if (lines.length > 16) throw new DataFormError('limit');
  const inputs = lines.map((line, index) => {
    const parts = line.split('|').map(part => part.trim());
    if (parts.length !== 2 || !/^[a-fA-F0-9]{64}$/.test(parts[1])) throw new DataFormError('digest');
    return { artifact_id: `input-${index + 1}`, relative_path: relative(parts[0]), expected_sha256: parts[1].toLowerCase(), max_bytes: limits.max_bytes };
  });
  const outputs = [form.tablePath, form.scriptPath, form.environmentPath].map((path, index) => ({ artifact_id: ['table', 'script', 'environment'][index], relative_path: relative(path), expected_sha256: null, max_bytes: limits.max_bytes }));
  if (new Set([...inputs, ...outputs].map(item => item.relative_path)).size !== inputs.length + 3) throw new DataFormError('path');
  if ((inputs.length + 3) * limits.max_bytes > 16777216) throw new DataFormError('limit');
  if (!form.columns.length || form.columns.length > 64) throw new DataFormError('column');
  const names = new Set<string>(), units: Record<string, string> = Object.create(null);
  const columns: JsonColumn[] = form.columns.map(column => {
    if (!/^[A-Za-z_][A-Za-z0-9_]{0,63}$/.test(column.name) || names.has(column.name)) throw new DataFormError('column');
    names.add(column.name);
    let scalar: Scalar;
    if (column.type === 'number' || column.type === 'integer') {
      const min = column.type === 'integer' ? integer(column.min, Number.MIN_SAFE_INTEGER) : number(column.min);
      const max = column.type === 'integer' ? integer(column.max, Number.MIN_SAFE_INTEGER) : number(column.max);
      if (min > max) throw new DataFormError('range');
      if (!column.unit.trim() || bytes(column.unit) > 128) throw new DataFormError('metadata');
      units[column.name] = column.unit;
      scalar = { type: column.type, min, max };
    } else if (column.type === 'string') scalar = { type: 'string', max_bytes: integer(column.maxBytes, 1, limits.max_string_bytes) };
    else if (column.type === 'boolean') scalar = { type: 'boolean' };
    else throw new DataFormError('column');
    return { name: column.name, required: column.required, nullable: column.nullable, scalar };
  });
  if (!Object.keys(units).length || !/^[A-Za-z0-9._:\-/]{1,256}$/.test(form.timezone)) throw new DataFormError('metadata');
  const dataset_rows = integer(form.datasetRows), seed = form.seed === '' ? null : integer(form.seed);
  const scope: JsonTableContract['analysis']['scope'] = form.scope === 'full' ? { kind: 'full' } : { kind: 'sample', rows: integer(form.sampleRows, 1), limitation: form.limitation };
  if ((scope.kind === 'sample' && (!scope.limitation.trim() || bytes(scope.limitation) > 4096 || scope.rows > dataset_rows)) || (scope.kind === 'sample' ? scope.rows : dataset_rows) > limits.max_rows) throw new DataFormError('sample');
  const selection: DataSelection = { session_id: form.sessionId, inputs, outputs, table: { table_artifact_id: 'table', dataset_rows, columns, analysis: { script_artifact_id: 'script', environment_artifact_id: 'environment', units, timezone: form.timezone, seed, scope }, limits } };
  // This is only the selection size. Rust separately checks its complete native snapshot.
  if (bytes(JSON.stringify(selection)) > 16384) throw new DataFormError('snapshot');
  return selection;
}
export const dataValidationApi = {
  policiesList: (taskId: string) => request<DataPolicy[]>('data_validation_policies_list', { taskId }),
  approve: (selection: DataSelection, idempotencyKey: string) => request<DataPolicy>('data_validation_policy_approve', { selection, idempotencyKey }),
  recordsList: (taskId: string) => request<DataRecord[]>('data_validations_list', { taskId }),
  run: (policyId: string, expectedRevision: number, idempotencyKey: string) => request<DataRecord>('data_validations_run', { policyId, expectedRevision, idempotencyKey }),
  revoke: (policyId: string, expectedRevision: number) => request<DataPolicy>('data_validation_policies_revoke', { policyId, expectedRevision }),
};
