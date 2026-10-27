import { request, type Task } from './api';
import type { ExecutionGrant } from './session-api';
import type { ArtifactRequirement } from './validation-api';
export interface GoalBudget { attempts: number; parallel: number; runtime_ms: number; output_bytes: number; children: number; reworks: number; tokens: number | null; cost_microunits: number | null }
export interface GoalBinding { project_id: string; root_task_id: string; task_id: string; spec_revision: number; input_revision: string }
export interface GoalExecutionBinding { connection_id: string; environment_id: string; identity_ref: string; host_id: string; workspace_id: string; config_revision: string; input_revision: string; capability_snapshot_id: string; policy_revision: string }
export interface GoalValidatorRef { kind: 'integrity' | 'data' | 'command'; policy_id: string; revision: number }
export interface GoalSelection {
  task_id: string; expected_task_revision: number; grant_id: string; expected_grant_revision: number; max_nodes: number;
  new_max_children: number; new_max_depth: number; root_budget: GoalBudget; project_budget: GoalBudget;
  inputs: ArtifactRequirement[]; outputs: { artifact: ArtifactRequirement; validators: GoalValidatorRef[] }[];
  writable_prefixes: string[]; owner_acceptance_criteria: string;
}
export interface GoalScope {
  id: string; revision: number; revoked: boolean; task_id: string; project_id: string; root_grant_id: string; root_grant_revision: number;
  authority_revision: number; scope_digest: string; goal: GoalBinding; root_budget: GoalBudget; project_budget: GoalBudget;
  outputs: Record<string, string>; required_acceptance_rules: string[]; contracts: Record<string, string[]>;
  inputs: Record<string, { contract_id: string; sha256: string }>; binding: GoalExecutionBinding; program_sha256: string;
  expires_at_ms: number; rules: Record<string, string>; workspace_id: string; input_revision: string; planning_only: boolean;
}
export interface GoalPlanNode {
  id: string; parent_id: string | null; kind: 'work' | 'validate' | 'owner_approval' | 'summary'; goal: string;
  dependencies: string[]; workspace_id: string;
  executor: { kind: 'connection'; binding: GoalExecutionBinding; program_sha256: string } | { kind: 'validator' | 'owner'; rule_id: string };
  recipients: string[]; required_inputs: { goal_task_id: string; artifact_id: string; contract_id: string; source: { kind: 'approved_external'; sha256: string } | { kind: 'node'; node_id: string; candidate_for_validation: boolean } }[];
  outputs: { artifact_id: string; contract_id: string }[]; required_validators: string[];
  side_effect: 'advisory_read_only' | 'workspace_write' | 'external_write' | 'production'; write_set: ({ kind: 'workspace'; path: string } | { kind: 'external'; resource_id: string })[];
  budget: GoalBudget; reservation: { runtime_ms: number; output_bytes: number; tokens: number | null; cost_microunits: number | null };
  retry: { max_attempts: number; max_reworks: number; retry_classes: string[] }; idempotency_key: string; external_action_id: string | null;
}
export interface GoalPlanProposal { schema_version: number; plan_id: string; goal: GoalBinding; scope_revision: number; revision: number; expected_revision: number; acceptance_rules: string[]; nodes: GoalPlanNode[] }
export interface GoalPlan { scope_id: string; proposal: GoalPlanProposal; digest: string; created_at: string; state: string; dispatched: boolean }
export interface GoalUsage { attempts: number; active: number; children: number; runtime_ms: number | null; output_bytes: number | null; tokens: number | null; cost_microunits: number | null }
export interface GoalPreview {
  scope_id: string; plan_id: string; plan_revision: number; journal_sequence: number; read_only: boolean; reservation_made: boolean; dispatched: boolean;
  budget_usage: { root: GoalUsage; project: GoalUsage }; unknown_dimensions: string[]; blocked_reasons: string[];
  decision: { plan_digest: string; expected_plan_revision: number; expected_ledger_revision: number; admissions: { node_id: string; next_attempt: number; [key: string]: unknown }[]; blocked: Record<string, string>; required_checks_satisfied: boolean; requires_atomic_store_commit: boolean } | null;
}
export interface BudgetForm { attempts: string; parallel: string; runtime_ms: string; output_bytes: string; children: string; reworks: string }
export interface GoalOutputForm { id: string; path: string; hash: string; validators: { kind: GoalValidatorRef['kind']; policyId: string; revision: string }[] }
export interface GoalForm { grantId: string; maxNodes: string; maxChildren: string; maxDepth: string; root: BudgetForm; project: BudgetForm; inputs: string; outputs: GoalOutputForm[]; cap: string; writable: string; criteria: string }
export function budgetForm(): BudgetForm { return { attempts: '1', parallel: '1', runtime_ms: '10000', output_bytes: '1048576', children: '3', reworks: '0' }; }
export function newGoalForm(): GoalForm { return { grantId: '', maxNodes: '4', maxChildren: '3', maxDepth: '1', root: budgetForm(), project: budgetForm(), inputs: '', outputs: [{ id: 'output-1', path: '', hash: '', validators: [] }], cap: '1048576', writable: '', criteria: '' }; }
export type GoalIssue = 'invalid' | 'integer' | 'path' | 'hash' | 'scope' | 'limit' | 'json' | 'duplicate' | 'unknownField' | 'unsupported';
export class GoalFormError extends Error { constructor(public reason: GoalIssue) { super(reason); } }
const fail = (reason: GoalIssue): never => { throw new GoalFormError(reason); };
export const utf8Bytes = (text: string) => new TextEncoder().encode(text).length;
export const safeInteger = (value: unknown, min = 0, max = Number.MAX_SAFE_INTEGER): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value >= min && value <= max;
function integer(text: string, min: number, max = Number.MAX_SAFE_INTEGER): number { if (!/^(0|[1-9]\d*)$/.test(text) || !safeInteger(Number(text), min, max)) return fail('integer'); return Number(text); }
const validId = (value: unknown): value is string => typeof value === 'string' && /^[A-Za-z0-9._:-]{1,256}$/.test(value);
function relative(value: string) { if (!value || value.startsWith('/') || /^[A-Za-z]:/.test(value) || /[\\\u0000-\u001f\u007f]/.test(value) || utf8Bytes(value) > 4096 || value.split('/').some(part => !part || part === '.' || part === '..')) return fail('path'); return value; }
function parseBudget(form: BudgetForm): GoalBudget { const b = { attempts: integer(form.attempts, 1, 100000), parallel: integer(form.parallel, 1, 100000), runtime_ms: integer(form.runtime_ms, 1, 604800000), output_bytes: integer(form.output_bytes, 1), children: integer(form.children, 0, 1000), reworks: integer(form.reworks, 0, 1000), tokens: null, cost_microunits: null }; if (b.parallel > b.attempts) return fail('limit'); return b; }
export function parseGoalSelection(form: GoalForm, task: Task, grant: ExecutionGrant): GoalSelection {
  if (!task.project_id || task.project_id !== grant.project_id || grant.id !== form.grantId || grant.parent_id !== null || grant.root_grant_id !== grant.id || grant.revoked || !safeInteger(task.revision, 1) || !safeInteger(grant.revision, 1)) return fail('scope');
  const root = parseBudget(form.root), project = parseBudget(form.project);
  const children = integer(form.maxChildren, 0, 1000), depth = integer(form.maxDepth, 0, 32), maxNodes = integer(form.maxNodes, 1, 256);
  if (children < grant.scope.max_children || depth < grant.scope.max_depth || maxNodes > children + 1 || root.attempts > grant.scope.max_attempts || root.parallel > grant.scope.max_concurrency || root.runtime_ms > grant.scope.max_duration_ms || root.children > children || root.attempts > project.attempts || root.parallel > project.parallel || root.runtime_ms > project.runtime_ms || root.output_bytes > project.output_bytes || root.children > project.children || root.reworks > project.reworks) return fail('limit');
  const cap = integer(form.cap, 1, 16777216), ids = new Set<string>(), paths = new Set<string>();
  const artifact = (id: string, path: string, hash: string): ArtifactRequirement => {
    if (!validId(id) || ids.has(id)) return fail('duplicate'); ids.add(id);
    const p = relative(path.trim()); if (paths.has(p)) return fail('duplicate'); paths.add(p);
    if (hash && !/^[a-fA-F0-9]{64}$/.test(hash)) return fail('hash');
    return { artifact_id: id, relative_path: p, max_bytes: cap, expected_sha256: hash ? hash.toLowerCase() : null };
  };
  const lines = form.inputs.split(/\r?\n/).map(line => line.trim()).filter(Boolean);
  if (lines.length > 16 || !form.outputs.length || form.outputs.length > 16 || lines.length * cap > 67108864 || form.outputs.length * cap > root.output_bytes) return fail('limit');
  const inputs = lines.map((line, i) => { const parts = line.split('|').map(part => part.trim()); if (parts.length !== 2 || !parts[1]) return fail('hash'); return artifact(`input-${i + 1}`, parts[0], parts[1]); });
  const outputs = form.outputs.map(item => ({ artifact: artifact(item.id, item.path, item.hash), validators: item.validators.map(ref => { if (!['integrity','data','command'].includes(ref.kind) || !validId(ref.policyId)) return fail('invalid'); return { kind: ref.kind, policy_id: ref.policyId, revision: integer(ref.revision, 1) }; }) }));
  if (outputs.some(item => item.validators.length > 8) || !form.criteria.trim() || utf8Bytes(form.criteria) > 2048) return fail('limit');
  const writable = form.writable.split(/\r?\n/).map(path => path.trim()).filter(Boolean).map(relative);
  if (!writable.length || writable.length > 32 || new Set(writable).size !== writable.length) return fail('path');
  const selection: GoalSelection = { task_id: task.id, expected_task_revision: task.revision, grant_id: grant.id, expected_grant_revision: grant.revision, max_nodes: maxNodes, new_max_children: children, new_max_depth: depth, root_budget: root, project_budget: project, inputs, outputs, writable_prefixes: writable, owner_acceptance_criteria: form.criteria };
  if (utf8Bytes(JSON.stringify(selection)) > 16384) return fail('limit'); return selection;
}

/** Bounded JSON proposal parser. Duplicate keys, unsafe numbers and excess depth
 * are rejected before JSON values lose that information; this is not authority. */
export function parseGoalJson(text: string): unknown {
  if (utf8Bytes(text) > 262144) return fail('limit');
  let index = 0, values = 0;
  const white = () => { while (/[ \t\r\n]/.test(text[index] ?? '') && index < text.length) ++index; };
  const string = (): string => { const start = index++; while (index < text.length) { const char = text[index++]; if (char === '\\') ++index; else if (char === '"') { try { return JSON.parse(text.slice(start,index)) as string; } catch { return fail('json'); } } } return fail('json'); };
  const read = (depth: number): unknown => {
    white(); if (depth > 48 || ++values > 100000) return fail('limit');
    const char = text[index];
    if (char === '"') return string();
    if (char === '{') { ++index; const object: Record<string, unknown> = Object.create(null), keys = new Set<string>(); white(); if (text[index] === '}') { ++index; return object; } while (index < text.length) { white(); if (text[index] !== '"') return fail('json'); const key = string(); if (keys.has(key)) return fail('duplicate'); keys.add(key); white(); if (text[index++] !== ':') return fail('json'); object[key] = read(depth+1); white(); const end = text[index++]; if (end === '}') return object; if (end !== ',') return fail('json'); } return fail('json'); }
    if (char === '[') { ++index; const array: unknown[] = []; white(); if (text[index] === ']') { ++index; return array; } while (index < text.length) { array.push(read(depth+1)); white(); const end = text[index++]; if (end === ']') return array; if (end !== ',') return fail('json'); } return fail('json'); }
    for (const [literal,value] of [['true',true],['false',false],['null',null]] as const) if (text.slice(index,index+literal.length) === literal) { index += literal.length; return value; }
    const token = /^-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?/.exec(text.slice(index))?.[0];
    if (!token) return fail('json'); index += token.length;
    if (!/^(0|[1-9]\d*)$/.test(token) || !safeInteger(Number(token))) return fail('integer'); return Number(token);
  };
  const value = read(0); white(); if (index !== text.length) return fail('json'); return value;
}
function object(value: unknown, keys: string[]): Record<string, unknown> { if (!value || typeof value !== 'object' || Array.isArray(value)) return fail('invalid'); const v = value as Record<string,unknown>; if (Object.keys(v).some(key => !keys.includes(key))) return fail('unknownField'); if (keys.some(key => !Object.hasOwn(v,key))) return fail('invalid'); return v; }
const list = (value: unknown): unknown[] => Array.isArray(value) && value.length <= 256 ? value : fail('limit');
function ids(value: unknown): string[] { const v = list(value); if (v.some(item => !validId(item)) || new Set(v).size !== v.length) return fail('invalid'); return v as string[]; }
function budget(value: unknown) { const v = object(value,['attempts','parallel','runtime_ms','output_bytes','children','reworks','tokens','cost_microunits']); for (const key of ['attempts','parallel','runtime_ms','output_bytes']) if (!safeInteger(v[key],1)) return fail('integer'); for (const key of ['children','reworks']) if (!safeInteger(v[key])) return fail('integer'); if (v.tokens !== null || v.cost_microunits !== null) return fail('unsupported'); return v; }
export function parseGoalPlan(text: string, scope: GoalScope, previous: GoalPlan | null): GoalPlanProposal {
  const p = object(parseGoalJson(text),['schema_version','plan_id','goal','scope_revision','revision','expected_revision','acceptance_rules','nodes']);
  const bindingKeys = ['connection_id','environment_id','identity_ref','host_id','workspace_id','config_revision','input_revision','capability_snapshot_id','policy_revision'];
  const goal = object(p.goal,['project_id','root_task_id','task_id','spec_revision','input_revision']);
  if (Object.keys(goal).some(key => goal[key] !== scope.goal[key as keyof GoalBinding]) || p.schema_version !== 1 || p.scope_revision !== scope.authority_revision || !validId(p.plan_id) || p.expected_revision !== (previous?.proposal.revision ?? 0) || p.revision !== (previous?.proposal.revision ?? 0)+1 || (previous && p.plan_id !== previous.proposal.plan_id)) return fail('scope');
  const acceptance = ids(p.acceptance_rules); if (acceptance.length !== scope.required_acceptance_rules.length || acceptance.some(id => !scope.required_acceptance_rules.includes(id))) return fail('scope');
  const nodes = list(p.nodes); if (!nodes.length) return fail('invalid'); const nodeIds = new Set<string>(), keys = new Set<string>();
  for (const node of nodes) {
    const n = object(node,['id','parent_id','kind','goal','dependencies','workspace_id','executor','recipients','required_inputs','outputs','required_validators','side_effect','write_set','budget','reservation','retry','idempotency_key','external_action_id']);
    if (!validId(n.id) || nodeIds.has(n.id) || !validId(n.idempotency_key) || keys.has(n.idempotency_key)) return fail('duplicate'); nodeIds.add(n.id); keys.add(n.idempotency_key);
    if (!['work','validate','owner_approval','summary'].includes(String(n.kind)) || typeof n.goal !== 'string' || !n.goal.trim() || utf8Bytes(n.goal)>16384 || (n.parent_id !== null && !validId(n.parent_id)) || n.workspace_id !== scope.workspace_id) return fail('scope');
    ids(n.dependencies); ids(n.recipients); if (ids(n.required_validators).some(rule => !Object.hasOwn(scope.rules,rule))) return fail('scope');
    if (!['advisory_read_only','workspace_write'].includes(String(n.side_effect)) || n.external_action_id !== null) return fail('unsupported');
    const e = n.executor as Record<string,unknown>;
    if (!e || typeof e !== 'object') return fail('invalid');
    if (e.kind === 'connection') { object(e,['kind','binding','program_sha256']); const b = object(e.binding,bindingKeys); if (scope.binding.host_id !== 'local' || bindingKeys.some(key => b[key] !== scope.binding[key as keyof GoalExecutionBinding]) || e.program_sha256 !== scope.program_sha256) return fail('unsupported'); }
    else if (e.kind === 'owner' || e.kind === 'validator') { object(e,['kind','rule_id']); if (!validId(e.rule_id) || !Object.hasOwn(scope.rules,e.rule_id)) return fail('scope'); } else return fail('unsupported');
    budget(n.budget); const reservation = object(n.reservation,['runtime_ms','output_bytes','tokens','cost_microunits']); if (!safeInteger(reservation.runtime_ms,1) || !safeInteger(reservation.output_bytes,1) || reservation.tokens !== null || reservation.cost_microunits !== null) return fail('unsupported');
    const retry = object(n.retry,['max_attempts','max_reworks','retry_classes']); if (!safeInteger(retry.max_attempts,1) || !safeInteger(retry.max_reworks) || list(retry.retry_classes).some(value => !['before_dispatch_transient','crashed_no_side_effect','validation_failed'].includes(String(value)))) return fail('unsupported');
    for (const item of list(n.outputs)) { const o = object(item,['artifact_id','contract_id']); if (!validId(o.artifact_id) || !validId(o.contract_id) || !Object.hasOwn(scope.contracts,o.contract_id)) return fail('scope'); }
    for (const item of list(n.required_inputs)) { const i = object(item,['goal_task_id','artifact_id','contract_id','source']); if (i.goal_task_id !== scope.goal.task_id || !validId(i.artifact_id) || !validId(i.contract_id)) return fail('scope'); const source = i.source as Record<string,unknown>; if (source?.kind === 'approved_external') { object(source,['kind','sha256']); if (typeof source.sha256 !== 'string' || !/^[a-f0-9]{64}$/.test(source.sha256)) return fail('hash'); } else if (source?.kind === 'node') { object(source,['kind','node_id','candidate_for_validation']); if (!validId(source.node_id) || typeof source.candidate_for_validation !== 'boolean') return fail('invalid'); } else return fail('invalid'); }
    for (const item of list(n.write_set)) { const write = object(item,['kind','path']); if (write.kind !== 'workspace' || typeof write.path !== 'string') return fail('unsupported'); relative(write.path); }
  }
  // The Core still validates the complete DAG, references, budgets and authority.
  return p as unknown as GoalPlanProposal;
}
export const goalApi = {
  grantsList: (projectId: string) => request<ExecutionGrant[]>('grants_list',{projectId}),
  scopesList: (taskId: string) => request<GoalScope[]>('goal_scopes_list',{taskId}),
  scopeGet: (scopeId: string) => request<GoalScope>('goal_scopes_get',{scopeId}),
  approve: (selection: GoalSelection,idempotencyKey: string) => request<GoalScope>('goal_scope_approve',{selection,idempotencyKey}),
  revoke: (scopeId: string,expectedRevision: number,idempotencyKey: string) => request<GoalScope>('goal_scopes_revoke',{scopeId,expectedRevision,idempotencyKey}),
  propose: (scopeId: string,planJson: string,expectedRevision: number,idempotencyKey: string) => request<GoalPlan>('goal_plans_propose',{scopeId,planJson,expectedRevision,idempotencyKey}),
  revise: (scopeId: string,planJson: string,expectedRevision: number,idempotencyKey: string) => request<GoalPlan>('goal_plans_revise',{scopeId,planJson,expectedRevision,idempotencyKey}),
  planGet: (scopeId: string) => request<GoalPlan>('goal_plans_get',{scopeId}),
  plansList: (scopeId: string) => request<GoalPlan[]>('goal_plans_list',{scopeId}),
  preview: (scopeId: string,expectedRevision: number) => request<GoalPreview>('goal_plans_ready_preview',{scopeId,expectedRevision}),
};
