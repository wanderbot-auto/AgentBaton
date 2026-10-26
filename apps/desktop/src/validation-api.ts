import { request } from './api';

export interface ArtifactRequirement {
  artifact_id: string;
  relative_path: string;
  max_bytes: number;
  expected_sha256: string | null;
}
export interface ValidationSelection { session_id: string; inputs: ArtifactRequirement[]; outputs: ArtifactRequirement[]; analysis: null }
export interface ArtifactContract {
  canonical_root: string;
  // Read-only server identities. Never compare or roundtrip u64 directory IDs.
  root_device: number;
  root_inode: number;
  requirement: ArtifactRequirement;
}
export interface ValidationSubject {
  task_id: string; attempt_id: string; spec_revision: number; input_revision: string;
  version: { kind: string; workspace_id?: string; base_revision?: string; [field: string]: unknown };
}
export interface ValidationContract { validator_id: string; subject: ValidationSubject; inputs: ArtifactContract[]; outputs: ArtifactContract[]; analysis: unknown }
export interface ValidationPolicy {
  id: string; task_id: string; project_id: string; session_id: string; attempt_id: string; workspace_id: string;
  approval_id: string; revision: number; revoked: boolean; scope: string; contract: ValidationContract; created_at: string;
}
export interface ArtifactEvidence {
  artifact_id: string; relative_path: string; root_device: number; root_inode: number;
  expected_sha256: string | null; observed_sha256: string | null; size: number | null; state: string;
}
export interface ValidationRecord {
  id: string; policy_id: string; task_id: string; state: string; scope: string;
  correctness: string; task_acceptance: string; integration_authorized: boolean; created_at: string;
  evidence: { schema_version: number; digest: string; validator_id: string; subject: ValidationSubject;
    approval_id: string; policy_revision: string; policy_contract: ValidationContract;
    inputs: ArtifactEvidence[]; outputs: ArtifactEvidence[]; analysis: unknown; passed: boolean };
}
export class ArtifactSelectionError extends Error {
  constructor(public reason: 'path' | 'format' | 'digest' | 'limit' | 'empty' | 'duplicate') { super(reason); }
}
const maxFiles = 16;
export const maxArtifactMiB = 8;
function parseLines(text: string, input: boolean, cap: number): ArtifactRequirement[] {
  const lines = text.split(/\r?\n/).map(line => line.trim()).filter(Boolean);
  if (lines.length > maxFiles) throw new ArtifactSelectionError('limit');
  const paths = new Set<string>();
  return lines.map((line, index) => {
    const parts = line.split('|').map(part => part.trim());
    if (parts.length > 2) throw new ArtifactSelectionError('format');
    const [path, hash = ''] = parts;
    if (!path || path.startsWith('/') || /^[A-Za-z]:/.test(path) || path.includes('\\') || /[\u0000-\u001f\u007f]/.test(path)
      || new TextEncoder().encode(path).length > 240 || path.split('/').some(part => !part || part === '.' || part === '..')) throw new ArtifactSelectionError('path');
    if (paths.has(path)) throw new ArtifactSelectionError('duplicate');
    paths.add(path);
    if ((input && !hash) || (hash && !/^[a-fA-F0-9]{64}$/.test(hash))) throw new ArtifactSelectionError('digest');
    return { artifact_id: `${input ? 'input' : 'output'}-${index + 1}`, relative_path: path, max_bytes: cap, expected_sha256: hash ? hash.toLowerCase() : null };
  });
}
/** Form checks are usability hints. The daemon independently resolves roots,
 * confirmed-stop evidence, scope, paths and byte limits before any file read. */
export function parseValidationSelection(sessionId: string, inputsText: string, outputsText: string, maxMiB: number): ValidationSelection {
  if (!sessionId) throw new ArtifactSelectionError('empty');
  if (!Number.isInteger(maxMiB) || maxMiB < 1 || maxMiB > maxArtifactMiB) throw new ArtifactSelectionError('limit');
  const cap = maxMiB * 1_048_576;
  const inputs = parseLines(inputsText, true, cap), outputs = parseLines(outputsText, false, cap);
  if (!outputs.length) throw new ArtifactSelectionError('empty');
  if (inputs.length + outputs.length > maxFiles) throw new ArtifactSelectionError('limit');
  return { session_id: sessionId, inputs, outputs, analysis: null };
}
export const validationApi = {
  policiesList: (taskId: string) => request<ValidationPolicy[]>('validation_policies_list', { taskId }),
  policiesApprove: (selection: ValidationSelection, idempotencyKey: string) => request<ValidationPolicy>('validation_policy_approve', { selection, idempotencyKey }),
  recordsList: (taskId: string) => request<ValidationRecord[]>('validations_list', { taskId }),
  run: (policyId: string, expectedRevision: number, idempotencyKey: string) => request<ValidationRecord>('validations_run', { policyId, expectedRevision, idempotencyKey }),
  revoke: (policyId: string, expectedRevision: number) => request<ValidationPolicy>('validation_policies_revoke', { policyId, expectedRevision }),
};
