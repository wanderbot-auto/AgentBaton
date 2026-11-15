import type { Task } from './api';

/** Filesystem integers stay canonical decimal text, including values above 2^53. */
export interface TaskGitWorkspacePin {
  canonical_root: string;
  device: string;
  inode: string;
  common_dir: string;
  head: string;
}
export interface TaskGitWorkspacePreparation {
  task_id: string;
  expected_task_revision: number;
  expected_spec_revision: number;
  expected_project_revision: number;
  host_id: 'local';
  expected_source_pin: TaskGitWorkspacePin;
  preparation_key: string;
}
export interface TaskWorkspaceReservationReceipt {
  version: 1;
  binding_id: string;
  workspace_id: string;
  task_id: string;
  project_id: string;
  host_id: 'local';
  original_task_revision: number;
  reserved_task_revision: number;
  spec_revision: number;
  project_revision: number;
  source_pin: TaskGitWorkspacePin;
  managed_root: string;
  preparation_key: string;
  request_key: string;
  request_digest: string;
  created_at: string;
}
export interface TaskWorkspaceBinding {
  receipt: TaskWorkspaceReservationReceipt;
  state: 'pending' | 'ready' | 'outcome_unknown';
  revision: number;
  managed_pin: TaskGitWorkspacePin | null;
}
export interface TaskWorkspaceBindingView {
  original_receipt: TaskWorkspaceReservationReceipt;
  binding: TaskWorkspaceBinding;
  task_revision: number;
  observed_managed_pin: TaskGitWorkspacePin;
  identity_revalidated: true;
  execution_authorized: false;
}

type ObjectValue = Record<string, unknown>;
const object = (v: unknown): v is ObjectValue => typeof v === 'object' && v !== null && !Array.isArray(v);
const exact = (v: unknown, keys: readonly string[]): v is ObjectValue => object(v) && Object.keys(v).length === keys.length && keys.every(k => Object.hasOwn(v, k));
const bytes = (v: string) => new TextEncoder().encode(v).length;
const text = (v: unknown, max: number): v is string => typeof v === 'string' && v.length > 0 && v.length <= max && bytes(v) <= max;
const positive = (v: unknown): v is number => typeof v === 'number' && Number.isSafeInteger(v) && v > 0;
const component = (v: unknown): v is string => text(v, 128) && /^[A-Za-z0-9_-]+$/.test(v);
const absolute = (v: unknown): v is string => text(v, 262144) && v.startsWith('/') && !v.includes('\0');
const hex = (v: unknown, lengths: readonly number[]): v is string => typeof v === 'string' && lengths.includes(v.length) && /^[0-9a-f]+$/.test(v);
const decimal = (v: unknown): v is string => typeof v === 'string' && /^(0|[1-9][0-9]{0,19})$/.test(v) && (v.length < 20 || v <= '18446744073709551615');
const preparationKey = (v: unknown): v is string => typeof v === 'string' && v.startsWith('code-prep:') && v.length === 74 && hex(v.slice(10), [64]);
const invalid = (): never => { throw new Error('Invalid Task workspace binding contract'); };
const pinKeys = ['canonical_root', 'device', 'inode', 'common_dir', 'head'] as const;
function pin(v: unknown): v is TaskGitWorkspacePin {
  return exact(v, pinKeys) && absolute(v.canonical_root) && decimal(v.device) && decimal(v.inode) && absolute(v.common_dir) && hex(v.head, [40, 64]);
}
function physical(a: TaskGitWorkspacePin, b: TaskGitWorkspacePin): boolean {
  return a.canonical_root === b.canonical_root && a.device === b.device && a.inode === b.inode && a.common_dir === b.common_dir;
}
function samePin(a: TaskGitWorkspacePin, b: TaskGitWorkspacePin): boolean { return physical(a, b) && a.head === b.head; }
const receiptKeys = ['version', 'binding_id', 'workspace_id', 'task_id', 'project_id', 'host_id', 'original_task_revision', 'reserved_task_revision', 'spec_revision', 'project_revision', 'source_pin', 'managed_root', 'preparation_key', 'request_key', 'request_digest', 'created_at'] as const;
const receiptScalarKeys = ['version', 'binding_id', 'workspace_id', 'task_id', 'project_id', 'host_id', 'original_task_revision', 'reserved_task_revision', 'spec_revision', 'project_revision', 'managed_root', 'preparation_key', 'request_key', 'request_digest', 'created_at'] as const;
function receipt(v: unknown): v is TaskWorkspaceReservationReceipt {
  return exact(v, receiptKeys) && v.version === 1 && component(v.binding_id) && component(v.workspace_id) && text(v.task_id, 128) && text(v.project_id, 128) && v.project_id !== 'local' && v.host_id === 'local' &&
    positive(v.original_task_revision) && positive(v.reserved_task_revision) && v.reserved_task_revision === v.original_task_revision + 1 && positive(v.spec_revision) && positive(v.project_revision) && pin(v.source_pin) && absolute(v.managed_root) && preparationKey(v.preparation_key) && text(v.request_key, 256) && hex(v.request_digest, [64]) && text(v.created_at, 128);
}
/** Read-only shape reuse; Core alone verifies receipt provenance and authority. */
export const taskWorkspaceReceiptShape = receipt;
function sameReceipt(a: TaskWorkspaceReservationReceipt, b: TaskWorkspaceReservationReceipt): boolean {
  // Core verifies the digest against original bytes. Here it remains an opaque
  // reference and is bound to the identical receipt, never re-encoded as authority.
  return receiptScalarKeys.every(k => a[k] === b[k]) && samePin(a.source_pin, b.source_pin);
}
function expectedRoot(r: TaskWorkspaceReservationReceipt): string { return r.managed_root.replace(/\/$/, '') + '/' + r.workspace_id; }
function binding(v: unknown): v is TaskWorkspaceBinding {
  if (!exact(v, ['receipt', 'state', 'revision', 'managed_pin']) || !receipt(v.receipt) || !positive(v.revision) || !['pending', 'ready', 'outcome_unknown'].includes(v.state as string)) return false;
  if (v.managed_pin !== null && (!pin(v.managed_pin) || v.managed_pin.canonical_root !== expectedRoot(v.receipt) || v.managed_pin.common_dir !== v.receipt.source_pin.common_dir)) return false;
  if (v.state === 'ready' && v.managed_pin === null) return false;
  return v.state !== 'pending' || v.managed_pin === null;
}

export function validateTaskGitWorkspacePreparation(v: unknown, key: unknown): TaskGitWorkspacePreparation {
  if (!exact(v, ['task_id', 'expected_task_revision', 'expected_spec_revision', 'expected_project_revision', 'host_id', 'expected_source_pin', 'preparation_key']) ||
      !text(v.task_id, 128) || !positive(v.expected_task_revision) || !positive(v.expected_spec_revision) || !positive(v.expected_project_revision) || v.host_id !== 'local' || !pin(v.expected_source_pin) || !preparationKey(v.preparation_key) || !text(key, 256)) return invalid();
  return { task_id: v.task_id, expected_task_revision: v.expected_task_revision, expected_spec_revision: v.expected_spec_revision, expected_project_revision: v.expected_project_revision, host_id: 'local', expected_source_pin: { ...v.expected_source_pin }, preparation_key: v.preparation_key };
}

export function parseTaskWorkspaceBindingView(v: unknown, selected: TaskGitWorkspacePreparation, key: string): TaskWorkspaceBindingView {
  if (!exact(v, ['original_receipt', 'binding', 'task_revision', 'observed_managed_pin', 'identity_revalidated', 'execution_authorized']) || !receipt(v.original_receipt) || !binding(v.binding) ||
      !sameReceipt(v.original_receipt, v.binding.receipt) || v.binding.state !== 'ready' || !positive(v.task_revision) || v.task_revision < v.original_receipt.reserved_task_revision || !pin(v.observed_managed_pin) || v.binding.managed_pin === null ||
      !physical(v.binding.managed_pin, v.observed_managed_pin) || v.identity_revalidated !== true || v.execution_authorized !== false) return invalid();
  const r = v.original_receipt;
  if (r.task_id !== selected.task_id || r.original_task_revision !== selected.expected_task_revision || r.spec_revision !== selected.expected_spec_revision || r.project_revision !== selected.expected_project_revision || r.request_key !== key || r.preparation_key !== selected.preparation_key || !samePin(r.source_pin, selected.expected_source_pin)) return invalid();
  return v as unknown as TaskWorkspaceBindingView;
}

export function validateTaskGetId(taskId: unknown): string { return text(taskId, 128) ? taskId : invalid(); }
export function parseTaskWithWorkspaceBinding(v: unknown, taskId: string): Task {
  const required = ['id', 'project_id', 'goal', 'spec_revision', 'state', 'revision', 'created_at', 'updated_at'];
  const optional = ['acceptance_ref', 'acceptance_sequence', 'code_workspace_binding'];
  if (!object(v) || !required.every(k => Object.hasOwn(v, k)) || Object.keys(v).some(k => !required.includes(k) && !optional.includes(k)) || v.id !== taskId || !text(v.id, 128) || !text(v.project_id, 128) || !text(v.goal, 16384) || !positive(v.spec_revision) || !positive(v.revision) || !['draft', 'running', 'cancel_requested', 'cancelled', 'outcome_unknown', 'accepted'].includes(v.state as string) || !text(v.created_at, 128) || !text(v.updated_at, 128)) return invalid();
  if (v.state === 'accepted') {
    if (!text(v.acceptance_ref, 256) || !/^[A-Za-z0-9._:-]{1,256}$/.test(v.acceptance_ref) || !positive(v.acceptance_sequence)) return invalid();
  } else if (v.acceptance_ref != null || v.acceptance_sequence != null) return invalid();
  if (v.code_workspace_binding != null) {
    if (!binding(v.code_workspace_binding) || v.code_workspace_binding.receipt.task_id !== taskId || v.revision < v.code_workspace_binding.receipt.reserved_task_revision) return invalid();
    // Stored binding may be pending/unknown/stale relative to current spec/project.
    // Reading it must not promote identity_revalidated or execution authorization.
  }
  return v as unknown as Task;
}
