/** Exact filesystem identity is opaque decimal text. It is never write authority. */
export interface CodeWorkspacePreparationRequest {
  task_id: string;
  expected_task_revision: number;
  expected_project_revision: number;
  host_id: 'local';
}
export interface CodeWorkspaceIdentity {
  canonical_root: string;
  device: string;
  inode: string;
  kind: { kind: 'directory' } | { kind: 'git'; common_dir: string; head: string; dirty: boolean };
}
export interface CodeWorkspaceCandidate {
  workspace_id: string;
  revision: number;
  root: string;
  recorded_state: 'ready' | 'creating' | 'outcome_unknown';
  has_unreconciled_lease: boolean;
  assigned_to_task: false;
  identity_revalidated: false;
}
export interface CodeWorkspacePreparation {
  task_id: string;
  task_revision: number;
  spec_revision: number;
  project_id: string;
  project_revision: number | null;
  host_id: 'local';
  source: CodeWorkspaceIdentity | null;
  source_head: string | null;
  preparation_key: string | null;
  state: 'needs_registered_project' | 'needs_git_workspace' | 'needs_dataset_selection' | 'blocked';
  reason: 'registered_project_required' | 'isolated_git_workspace_required' | 'selected_dataset_required' | 'task_not_draft';
  candidates: CodeWorkspaceCandidate[];
  execution_authorized: false;
  creates_workspace: false;
}

type ObjectValue = Record<string, unknown>;
const object = (v: unknown): v is ObjectValue => typeof v === 'object' && v !== null && !Array.isArray(v);
const exact = (v: unknown, keys: string[]): v is ObjectValue => object(v) && Object.keys(v).length === keys.length && keys.every(k => Object.hasOwn(v, k));
const text = (v: unknown, limit: number): v is string => typeof v === 'string' && v.length > 0 && v.length <= limit;
const positive = (v: unknown): v is number => typeof v === 'number' && Number.isSafeInteger(v) && v > 0;
const path = (v: unknown): v is string => text(v, 262144) && v.startsWith('/') && !v.includes('\0');
const head = (v: unknown): v is string => typeof v === 'string' && /^(?:[0-9a-f]{40}|[0-9a-f]{64})$/.test(v);
const decimalU64 = (v: unknown): v is string => typeof v === 'string' && /^(0|[1-9][0-9]{0,19})$/.test(v) && (v.length < 20 || v <= '18446744073709551615');
const invalid = (): never => { throw new Error('Invalid code workspace preparation contract'); };

export function validateCodeWorkspacePreparationRequest(value: unknown): CodeWorkspacePreparationRequest {
  if (!exact(value, ['task_id', 'expected_task_revision', 'expected_project_revision', 'host_id']) ||
      !text(value.task_id, 128) || !positive(value.expected_task_revision) ||
      typeof value.expected_project_revision !== 'number' || !Number.isSafeInteger(value.expected_project_revision) || value.expected_project_revision < 0 ||
      value.host_id !== 'local') return invalid();
  return { task_id: value.task_id, expected_task_revision: value.expected_task_revision, expected_project_revision: value.expected_project_revision, host_id: value.host_id };
}

function identity(value: unknown): value is CodeWorkspaceIdentity {
  if (!exact(value, ['canonical_root', 'device', 'inode', 'kind']) || !path(value.canonical_root) ||
      !decimalU64(value.device) || !decimalU64(value.inode)) return false;
  const kind = value.kind;
  return (exact(kind, ['kind']) && kind.kind === 'directory') ||
    (exact(kind, ['kind', 'common_dir', 'head', 'dirty']) && kind.kind === 'git' && path(kind.common_dir) && head(kind.head) && typeof kind.dirty === 'boolean');
}

export function parseCodeWorkspacePreparation(value: unknown, selection: CodeWorkspacePreparationRequest): CodeWorkspacePreparation {
  if (!exact(value, ['task_id', 'task_revision', 'spec_revision', 'project_id', 'project_revision', 'host_id', 'source', 'source_head', 'preparation_key', 'state', 'reason', 'candidates', 'execution_authorized', 'creates_workspace']) ||
      value.task_id !== selection.task_id || value.task_revision !== selection.expected_task_revision ||
      !positive(value.task_revision) || !positive(value.spec_revision) || !text(value.project_id, 128) || value.host_id !== 'local' ||
      value.execution_authorized !== false || value.creates_workspace !== false || !Array.isArray(value.candidates) || value.candidates.length > 1000) return invalid();
  const ids = new Set<string>();
  for (const candidate of value.candidates) {
    if (!exact(candidate, ['workspace_id', 'revision', 'root', 'recorded_state', 'has_unreconciled_lease', 'assigned_to_task', 'identity_revalidated']) ||
        !text(candidate.workspace_id, 128) || ids.has(candidate.workspace_id) || !positive(candidate.revision) || !path(candidate.root) ||
        !['ready', 'creating', 'outcome_unknown'].includes(candidate.recorded_state as string) || typeof candidate.has_unreconciled_lease !== 'boolean' ||
        candidate.assigned_to_task !== false || candidate.identity_revalidated !== false) return invalid();
    ids.add(candidate.workspace_id);
  }
  if (value.state === 'needs_registered_project') {
    if (value.reason !== 'registered_project_required' || value.project_id !== 'local' || value.project_revision !== null || value.source !== null ||
        value.source_head !== null || value.preparation_key !== null || value.candidates.length !== 0) return invalid();
  } else {
    if (value.project_id === 'local' || !positive(value.project_revision) || value.project_revision !== selection.expected_project_revision || !identity(value.source)) return invalid();
    const blocked = value.state === 'blocked' && value.reason === 'task_not_draft' && value.preparation_key === null;
    if (value.source.kind.kind === 'directory') {
      if (value.source_head !== null || value.preparation_key !== null || (!blocked && (value.state !== 'needs_dataset_selection' || value.reason !== 'selected_dataset_required'))) return invalid();
    } else {
      if (value.source_head !== value.source.kind.head || (!blocked &&
          (value.state !== 'needs_git_workspace' || value.reason !== 'isolated_git_workspace_required' ||
           typeof value.preparation_key !== 'string' || !/^code-prep:[0-9a-f]{64}$/.test(value.preparation_key)))) return invalid();
    }
  }
  return value as unknown as CodeWorkspacePreparation;
}
