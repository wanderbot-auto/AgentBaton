import { parseTaskWorkspaceBindingView, parseTaskWithWorkspaceBinding, validateTaskGetId, validateTaskGitWorkspacePreparation, type TaskGitWorkspacePreparation, type TaskWorkspaceBinding } from './task-workspace-binding-wire';
import { parseCodeWorkspacePreparation, validateCodeWorkspacePreparationRequest, type CodeWorkspacePreparationRequest } from './workspace-preparation-wire';
import { invoke } from '@tauri-apps/api/core';

export interface Task {
  id: string;
  project_id?: string;
  goal: string;
  state: string;
  revision: number;
  readonly spec_revision?: number;
  readonly acceptance_ref?: string | null;
  readonly acceptance_sequence?: number | null;
  readonly code_workspace_binding?: TaskWorkspaceBinding | null;
  created_at: string;
  updated_at: string;
}
/** Trusted Core Task projection, independent of a graph status boolean. */
export function taskHasAcceptanceProjection(task:Task):boolean {
  return task.state==='accepted'&&typeof task.project_id==='string'&&task.project_id.length>0&&typeof task.acceptance_ref==='string'&&/^[A-Za-z0-9._:-]{1,256}$/.test(task.acceptance_ref)&&Number.isSafeInteger(task.acceptance_sequence)&&task.acceptance_sequence!>0&&Number.isSafeInteger(task.spec_revision)&&task.spec_revision!>0&&Number.isSafeInteger(task.revision)&&task.revision>0;
}
export const taskAllowsExecution=(task:Task)=>['draft','running'].includes(task.state)&&task.acceptance_ref==null&&task.acceptance_sequence==null;
export interface WorkspaceIdentity {
  canonical_root: string;
  // Opaque server identity only; never compare or roundtrip these u64 fields.
  device: number;
  inode: number;
  kind: { kind: 'directory' } | { kind: 'git'; common_dir: string; head: string; dirty: boolean };
}
export interface Project {
  id: string;
  name: string;
  identity: WorkspaceIdentity;
  revision: number;
  archived: boolean;
  created_at: string;
  updated_at: string;
}
export interface ManagedWorkspace {
  id: string;
  project_id: string;
  identity: WorkspaceIdentity | null;
  root: string;
  revision: number;
  base_revision: string;
  created_at: string;
  state: 'creating' | 'ready' | 'outcome_unknown';
}
export interface JournalEvent {
  sequence: number;
  event_id: string;
  aggregate_id: string;
  revision: number;
  kind: string;
  occurred_at: string;
  payload: unknown;
  source: string;
}
export interface Snapshot { tasks: Task[]; sequence: number; api_version: number; database_schema_version: number }
export interface EventBatch { events: JournalEvent[]; next_cursor: number; sequence: number; gap: boolean; page_bytes?: number; byte_limited?: boolean }
interface Envelope<T> {
  result?: T;
  error?: { code: string; message: string; retryable: boolean; side_effect_state: string };
  warnings: string[];
}
export class DaemonError extends Error {
  constructor(public code: string, message: string, public sideEffectState: string) {
    super(message);
  }
}
export async function request<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const envelope = await invoke<Envelope<T>>(command, args);
  if (envelope.error) {
    throw new DaemonError(envelope.error.code, envelope.error.message, envelope.error.side_effect_state);
  }
  if (envelope.result === undefined) throw new Error('Daemon returned no result');
  return envelope.result;
}
export const api = {
  taskGet: async (taskId: string) => {
    const selected = validateTaskGetId(taskId);
    return parseTaskWithWorkspaceBinding(await request<unknown>('tasks_get', { taskId: selected }), selected);
  },
  workspacesPrepareTaskGit: async (selection: TaskGitWorkspacePreparation, idempotencyKey: string) => {
    const selected = validateTaskGitWorkspacePreparation(selection, idempotencyKey);
    return parseTaskWorkspaceBindingView(await request<unknown>('workspaces_prepare_task_git', { selection: selected, idempotencyKey }), selected, idempotencyKey);
  },
  snapshot: async () => {const value=await request<Snapshot>('system_snapshot');if(value.api_version!==1||value.database_schema_version!==17||!Array.isArray(value.tasks)||!Number.isSafeInteger(value.sequence)||value.sequence<0)throw new Error('Unsupported database schema or snapshot contract');if(value.tasks.some(task=>task.state==='accepted'&&(!taskHasAcceptanceProjection(task)||!Number.isSafeInteger(value.sequence)||task.acceptance_sequence!>value.sequence)))throw new Error('Invalid accepted Task projection');return value;},
  create: (goal: string, idempotencyKey: string, projectId?: string) => request<Task>('tasks_create', { goal, idempotencyKey, ...(projectId ? { projectId } : {}) }),
  cancel: (task: Task, idempotencyKey: string) => request<Task>('tasks_cancel', { taskId: task.id, expectedRevision: task.revision, idempotencyKey }),
  events: (cursor: number) => request<EventBatch>('events_subscribe', { cursor, limit: 100 }),
  projectsList: () => request<Project[]>('projects_list'),
  projectsImport: (path: string, name: string, idempotencyKey: string) => request<Project>('projects_import', { path, name, idempotencyKey }),
  projectsArchive: (project: Project, idempotencyKey: string) => request<Project>('projects_archive', { projectId: project.id, expectedRevision: project.revision, idempotencyKey }),
  projectsReassociate: (project: Project, path: string, idempotencyKey: string) => request<Project>('projects_reassociate', { projectId: project.id, path, expectedRevision: project.revision, idempotencyKey }),
  workspacesCreate: (project: Project, expectedHead: string, idempotencyKey: string) => request<ManagedWorkspace>('workspaces_create', { projectId: project.id, expectedHead, expectedRevision: project.revision, idempotencyKey }),
  workspacesList: (projectId: string) => request<ManagedWorkspace[]>('workspaces_list', { projectId }),
  workspacesResolve: async (selection: CodeWorkspacePreparationRequest) => {
    const checked = validateCodeWorkspacePreparationRequest(selection);
    return parseCodeWorkspacePreparation(await request<unknown>('workspaces_resolve', { selection: checked }), checked);
  },
};
