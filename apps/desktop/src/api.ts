import { invoke } from '@tauri-apps/api/core';

export interface Task {
  id: string;
  goal: string;
  state: string;
  revision: number;
  created_at: string;
  updated_at: string;
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
async function request<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const envelope = await invoke<Envelope<T>>(command, args);
  if (envelope.error) {
    throw new DaemonError(envelope.error.code, envelope.error.message, envelope.error.side_effect_state);
  }
  if (envelope.result === undefined) throw new Error('Daemon returned no result');
  return envelope.result;
}
export const api = {
  snapshot: () => request<Snapshot>('system_snapshot'),
  create: (goal: string, idempotencyKey: string) => request<Task>('tasks_create', { goal, idempotencyKey }),
  cancel: (task: Task, idempotencyKey: string) => request<Task>('tasks_cancel', { taskId: task.id, expectedRevision: task.revision, idempotencyKey }),
  events: (cursor: number) => request<EventBatch>('events_subscribe', { cursor, limit: 100 }),
};
