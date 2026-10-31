/** Directory-only handoff. IDs/revisions select existing records; paths never enter a request. */
export type WorkspaceOpenSelection =
  | { kind: 'project_source'; project_id: string; expected_project_revision: number }
  | { kind: 'managed_workspace'; project_id: string; expected_project_revision: number; workspace_id: string; expected_workspace_revision: number };
export interface WorkspaceOpenContext { selection: WorkspaceOpenSelection; canonical_path: string; directory_identity: { device: string; inode: string }[] }
export type WorkspaceOpenRequest = { selection: WorkspaceOpenSelection; terminal: 'macos_terminal' };
export type WorkspaceOpenCode = 'accepted' | 'unsupported_platform' | 'invalid_selection' | 'stale_context' | 'directory_changed' | 'native_window_invalid' | 'opener_unavailable' | 'opener_failed' | 'opener_timeout' | 'transport_unknown';
export interface WorkspaceOpenResult { status: 'handoff_accepted' | 'refused' | 'outcome_unknown'; code: WorkspaceOpenCode }
type Row = Record<string, unknown>;
const exact = (v: unknown, keys: string[]): v is Row => !!v && typeof v === 'object' && !Array.isArray(v) && Object.keys(v).length === keys.length && keys.every(k => Object.hasOwn(v, k));
const invalid = (): never => { throw new Error('Invalid workspace open contract'); };
const whole = (pattern: RegExp, value: string) => pattern.exec(value)?.[0] === value;
export const workspaceOpenId = (v: unknown): v is string => typeof v === 'string' && v.length === 36 && whole(/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/, v);
export const workspaceOpenRevision = (v: unknown): v is number => typeof v === 'number' && Number.isSafeInteger(v) && v > 0;
export const workspaceOpenPath = (v: unknown): v is string => typeof v === 'string' && v.length > 0 && v.length <= 32768 && v.startsWith('/') && !v.includes('\0') && (v === '/' || v.slice(1).split('/').every(p => p.length > 0 && p !== '.' && p !== '..')) && v.split('/').length <= 129;
const decimal = (v: unknown): v is string => typeof v === 'string' && whole(/(?:0|[1-9][0-9]{0,19})/, v) && (v.length < 20 || v <= '18446744073709551615');
export function validateWorkspaceOpenSelection(value: unknown): WorkspaceOpenSelection {
  if (!value || typeof value !== 'object') return invalid();
  const v = value as Row;
  if (!workspaceOpenId(v.project_id) || !workspaceOpenRevision(v.expected_project_revision)) return invalid();
  if (v.kind === 'project_source' && exact(v, ['kind', 'project_id', 'expected_project_revision'])) return { kind: v.kind, project_id: v.project_id, expected_project_revision: v.expected_project_revision };
  if (v.kind === 'managed_workspace' && exact(v, ['kind', 'project_id', 'expected_project_revision', 'workspace_id', 'expected_workspace_revision']) && workspaceOpenId(v.workspace_id) && workspaceOpenRevision(v.expected_workspace_revision)) return { kind: v.kind, project_id: v.project_id, expected_project_revision: v.expected_project_revision, workspace_id: v.workspace_id, expected_workspace_revision: v.expected_workspace_revision };
  return invalid();
}
export function validateWorkspaceOpenRequest(value: unknown): WorkspaceOpenRequest {
  if (!exact(value, ['selection', 'terminal']) || value.terminal !== 'macos_terminal') return invalid();
  return { selection: validateWorkspaceOpenSelection(value.selection), terminal: value.terminal };
}
export function parseWorkspaceOpenContext(value: unknown, selection: WorkspaceOpenSelection): WorkspaceOpenContext {
  const expected = validateWorkspaceOpenSelection(selection);
  if (!exact(value, ['selection', 'canonical_path', 'directory_identity']) || !workspaceOpenPath(value.canonical_path) || !Array.isArray(value.directory_identity)) return invalid();
  const selected = validateWorkspaceOpenSelection(value.selection);
  if (JSON.stringify(selected) !== JSON.stringify(expected)) return invalid();
  const count = value.canonical_path === '/' ? 1 : value.canonical_path.split('/').length;
  if (value.directory_identity.length !== count || count > 129 || value.directory_identity.some(v => !exact(v, ['device', 'inode']) || !decimal(v.device) || !decimal(v.inode))) return invalid();
  return { selection: selected, canonical_path: value.canonical_path, directory_identity: value.directory_identity.map(v => ({ device: v.device, inode: v.inode })) };
}
export function parseWorkspaceOpenResult(value: unknown): WorkspaceOpenResult {
  if (!exact(value, ['status', 'code'])) return invalid();
  const valid = value.status === 'handoff_accepted' && value.code === 'accepted' ||
    value.status === 'refused' && ['unsupported_platform', 'invalid_selection', 'stale_context', 'directory_changed', 'native_window_invalid', 'opener_unavailable'].includes(value.code as string) ||
    value.status === 'outcome_unknown' && ['opener_failed', 'opener_timeout', 'transport_unknown'].includes(value.code as string);
  if (!valid) return invalid();
  return { status: value.status as WorkspaceOpenResult['status'], code: value.code as WorkspaceOpenCode };
}
