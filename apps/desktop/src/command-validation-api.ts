import { request } from './api';
import type { ArtifactContract, ArtifactRequirement, ValidationContract, ValidationRecord } from './validation-api';
export const commandValidator = 'builtin.fixed-command.v1';
export const commandScope = 'approved_deterministic_command_only';
export const enforcementKeys = ['filesystem_isolation', 'network_isolation', 'process_tree_containment', 'execution_time_input_immutability', 'auxiliary_source_immutability', 'sealed_executable_image'] as const;
export type Enforcement = Record<typeof enforcementKeys[number], boolean>;
export interface CommandSelection {
  session_id: string; executable: string; mode: 'native_executable' | 'posix_sh_stdin'; script: ArtifactRequirement | null;
  inputs: ArtifactRequirement[]; outputs: ArtifactRequirement[]; arguments: string[]; environment: Record<string, string>;
  expected_exit_codes: number[]; wall_time_ms: number; cleanup_time_ms: number; stdout_limit: number; stderr_limit: number; require: Enforcement;
}
export interface CommandContract {
  validator_id: string; scope: string; file_contract: ValidationContract; program: ArtifactContract; script: ArtifactContract | null;
  mode: string; argv0: string; image_policy: string; execution_identity: { real_uid: number; effective_uid: number; real_gid: number; effective_gid: number };
  arguments: string[]; cwd: { canonical_root: string }; environment_sha256: Record<string, string>;
  expected_exit_codes: number[]; wall_time_ms: number; cleanup_time_ms: number; stdout_limit: number; stderr_limit: number; require: Enforcement;
}
export interface CommandPolicy {
  id: string; task_id: string; project_id: string; session_id: string; attempt_id: string; workspace_id: string;
  fencing_token: number; approval_id: string; revision: number; revoked: boolean; validator_id: string; scope: string;
  max_runs: 1; selection: CommandSelection; contract: CommandContract; contract_digest: string; created_at: string;
}
export interface CommandCapture { captured_bytes: number; observed_bytes_at_least: number; sha256_of_captured: string; truncated: boolean; eof_observed: boolean }
export interface CommandEvidence {
  validator_id: string; scope: string; approval_id: string; policy_revision: string; contract: CommandContract; contract_digest: string;
  process_started: boolean; exit_observed_within_budget: boolean; cleanup_completed_within_budget: boolean; cancellation_observed: boolean;
  reason: string; exit_code: number | null; exit_signal: number | null; stdout: CommandCapture; stderr: CommandCapture;
  cleanup: { owned_leader_reaped: boolean; owned_group_signal_sent: boolean; all_capture_pipes_observed_eof: boolean; descendant_containment: string; input_immutability: string };
  file_integrity: ValidationRecord['evidence'] | null; main_image: string; main_image_verified: boolean;
  native_configuration_integrity: string; auxiliary_validator_source_integrity: string; dynamic_dependency_integrity: string; transitive_execution_integrity: string;
  script_bytes_delivered: number; process_and_cleanup_elapsed_ms: number; task_acceptance: string; integration_authorized: boolean; passed: boolean; digest: string;
}
export interface CommandRecord {
  id: string; policy_id: string; task_id: string; session_id: string; workspace_id: string; attempt_id: string; revision: number;
  state: string; scope: string; correctness: string; task_acceptance: string; integration_authorized: boolean;
  evidence: CommandEvidence | null; last_error: { code: string; message: string; side_effect_state?: string } | null; created_at: string; updated_at: string;
}
export interface CommandForm {
  sessionId: string; executable: string; mode: CommandSelection['mode']; scriptPath: string; scriptHash: string;
  inputs: string; outputs: string; arguments: string; lang: string; lcAll: string; timezone: boolean;
  exits: string; wall: string; cleanup: string; stdout: string; stderr: string; cap: string; require: Enforcement;
}
export const emptyEnforcement = (): Enforcement => Object.fromEntries(enforcementKeys.map(key => [key, false])) as Enforcement;
export function newCommandForm(sessionId = ''): CommandForm { return { sessionId, executable: '', mode: 'native_executable', scriptPath: '', scriptHash: '', inputs: '', outputs: '', arguments: '', lang: '', lcAll: '', timezone: false, exits: '0', wall: '10000', cleanup: '1000', stdout: '16384', stderr: '16384', cap: '1048576', require: emptyEnforcement() }; }
export class CommandFormError extends Error { constructor(public reason: 'empty' | 'path' | 'hash' | 'limit' | 'environment' | 'arguments' | 'snapshot') { super(reason); } }
const bytes = (value: string) => new TextEncoder().encode(value).length;
function integer(text: string, min: number, max: number) {
  if (!/^(0|[1-9]\d*)$/.test(text) || !Number.isSafeInteger(Number(text)) || Number(text) < min || Number(text) > max) throw new CommandFormError('limit');
  return Number(text);
}
function path(text: string) {
  const value = text.trim();
  if (!value || value.startsWith('/') || /^[A-Za-z]:/.test(value) || /[\\\u0000-\u001f\u007f]/.test(value) || bytes(value) > 240 || value.split('/').some(part => !part || part === '.' || part === '..')) throw new CommandFormError('path');
  return value;
}
/** Proposal only. The daemon resolves executable hash, cwd, subject and stop/fence evidence. */
export function parseCommandSelection(form: CommandForm): CommandSelection {
  if (!form.sessionId) throw new CommandFormError('empty');
  const executable = form.executable.trim();
  if (!executable.startsWith('/') || bytes(executable) > 4096 || /[\u0000-\u001f\u007f]/.test(executable) || executable.split('/').some(part => part === '.' || part === '..')) throw new CommandFormError('path');
  const cap = integer(form.cap, 1, 4194304);
  const parse = (text: string, input: boolean) => text.split(/\r?\n/).map(line => line.trim()).filter(Boolean).map((line, index): ArtifactRequirement => {
    const parts = line.split('|').map(part => part.trim());
    if (parts.length > 2 || (input && !parts[1]) || (parts[1] && !/^[a-fA-F0-9]{64}$/.test(parts[1]))) throw new CommandFormError('hash');
    return { artifact_id: `${input ? 'input' : 'output'}-${index + 1}`, relative_path: path(parts[0]), max_bytes: cap, expected_sha256: parts[1]?.toLowerCase() || null };
  });
  const inputs = parse(form.inputs, true), outputs = parse(form.outputs, false);
  if (!inputs.length || !outputs.length) throw new CommandFormError('empty');
  if (inputs.length > 15 || outputs.length > 16 || (inputs.length + outputs.length) * cap > 16777216) throw new CommandFormError('limit');
  let script: ArtifactRequirement | null = null;
  if (form.mode === 'posix_sh_stdin') {
    if (!/^[a-fA-F0-9]{64}$/.test(form.scriptHash)) throw new CommandFormError('hash');
    script = { artifact_id: 'script', relative_path: path(form.scriptPath), expected_sha256: form.scriptHash.toLowerCase(), max_bytes: 65536 };
  } else if (form.mode !== 'native_executable') throw new CommandFormError('limit');
  const files = [...inputs, ...outputs, ...(script ? [script] : [])];
  if (new Set(files.map(item => item.relative_path)).size !== files.length) throw new CommandFormError('path');
  const args = form.arguments === '' ? [] : form.arguments.split('\n').map(line => line.endsWith('\r') ? line.slice(0, -1) : line);
  if (args.length > 64 || args.some(arg => arg.includes('\0') || bytes(arg) > 4096) || args.reduce((total, arg) => total + bytes(arg), 0) > 16384) throw new CommandFormError('arguments');
  const environment: Record<string, string> = {};
  for (const [key, value] of [['LANG', form.lang], ['LC_ALL', form.lcAll]]) {
    if (value && !['C', 'C.UTF-8', 'en_US.UTF-8'].includes(value)) throw new CommandFormError('environment');
    if (value) environment[key] = value;
  }
  if (form.timezone) environment.TZ = 'UTC';
  const exits = form.exits.split(',').map(value => integer(value.trim(), 0, 255));
  if (!exits.length || exits.length > 16 || new Set(exits).size !== exits.length) throw new CommandFormError('limit');
  const selection: CommandSelection = { session_id: form.sessionId, executable, mode: form.mode, script, inputs, outputs, arguments: args, environment, expected_exit_codes: exits, wall_time_ms: integer(form.wall, 1, 60000), cleanup_time_ms: integer(form.cleanup, 1, 1000), stdout_limit: integer(form.stdout, 0, 65536), stderr_limit: integer(form.stderr, 0, 65536), require: { ...form.require } };
  if (bytes(JSON.stringify(selection)) > 16384) throw new CommandFormError('snapshot');
  return selection;
}
export const commandValidationApi = {
  policiesList: (taskId: string) => request<CommandPolicy[]>('command_validation_policies_list', { taskId }),
  approve: (selection: CommandSelection, idempotencyKey: string) => request<CommandPolicy>('command_validation_policy_approve', { selection, idempotencyKey }),
  revoke: (policyId: string, expectedRevision: number, idempotencyKey: string) => request<CommandPolicy>('command_validation_policies_revoke', { policyId, expectedRevision, idempotencyKey }),
  run: (policyId: string, expectedRevision: number, idempotencyKey: string) => request<CommandRecord>('command_validations_run', { policyId, expectedRevision, idempotencyKey }),
  recordsList: (taskId: string) => request<CommandRecord[]>('command_validations_list', { taskId }),
  get: (runId: string) => request<CommandRecord>('command_validations_get', { runId }),
  cancel: (runId: string, expectedRevision: number, idempotencyKey: string) => request<CommandRecord>('command_validations_cancel', { runId, expectedRevision, idempotencyKey }),
};
