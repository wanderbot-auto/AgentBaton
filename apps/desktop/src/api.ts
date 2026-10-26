import { invoke } from '@tauri-apps/api/core';

export interface Task {
  id: string;
  project_id?: string;
  goal: string;
  state: string;
  revision: number;
  created_at: string;
  updated_at: string;
}
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
export interface Snapshot { tasks: Task[]; sequence: number; api_version: number }
export interface EventBatch { events: JournalEvent[]; next_cursor: number; sequence: number; gap: boolean }
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
  snapshot: () => request<Snapshot>('system_snapshot'),
  create: (goal: string, idempotencyKey: string, projectId?: string) => request<Task>('tasks_create', { goal, idempotencyKey, ...(projectId ? { projectId } : {}) }),
  cancel: (task: Task, idempotencyKey: string) => request<Task>('tasks_cancel', { taskId: task.id, expectedRevision: task.revision, idempotencyKey }),
  events: (cursor: number) => request<EventBatch>('events_subscribe', { cursor, limit: 100 }),
  projectsList: () => request<Project[]>('projects_list'),
  projectsImport: (path: string, name: string, idempotencyKey: string) => request<Project>('projects_import', { path, name, idempotencyKey }),
  projectsArchive: (project: Project, idempotencyKey: string) => request<Project>('projects_archive', { projectId: project.id, expectedRevision: project.revision, idempotencyKey }),
  projectsReassociate: (project: Project, path: string, idempotencyKey: string) => request<Project>('projects_reassociate', { projectId: project.id, path, expectedRevision: project.revision, idempotencyKey }),
  workspacesCreate: (project: Project, expectedHead: string, idempotencyKey: string) => request<ManagedWorkspace>('workspaces_create', { projectId: project.id, expectedHead, expectedRevision: project.revision, idempotencyKey }),
  workspacesList: (projectId: string) => request<ManagedWorkspace[]>('workspaces_list', { projectId }),
};
