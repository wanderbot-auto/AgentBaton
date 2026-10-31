import { taskHasAcceptanceProjection, type JournalEvent, type Task } from './api';
import type { ManagedSession } from './session-api';
import { MAX_EVENT_WINDOW_RECORDS } from './event-window';

export const NOTIFICATION_LIMITS = { tasks: 500, sessions: 1000, events: MAX_EVENT_WINDOW_RECORDS, rows: 20, sources: 10 } as const;
export interface NotificationObservation {
  connected: boolean;
  tasksFresh: boolean;
  sessionsFresh: boolean;
  eventsFresh: boolean;
  eventLimited: boolean;
}
export type NotificationCategory = 'accepted' | 'failed' | 'unknown' | 'cancelRequested' | 'approval' | 'acceptanceUnverified' | 'sessionAttention' | 'observed';
/** Metadata only. Payloads/text never supply task identity, authority or status. */
export interface NotificationEventSource {
  taskId: string;
  sessionId?: string;
  eventId: string;
  sequence: number;
  revision: number;
  kind: string;
  source: string;
  occurredAt: string;
}
export interface NotificationEvidence { refs: NotificationEventSource[]; limited: boolean }
export interface TaskNotification {
  taskId: string;
  goal: string;
  category: NotificationCategory;
  stale: boolean;
  taskRevision: number;
  taskState: string;
  updatedAt: string;
  sessions: { id: string; revision: number; state: string; turnState: string; updatedAt: string }[];
  sources: NotificationEventSource[];
}

const idValid = (value: unknown): value is string => typeof value === 'string' && /^[A-Za-z0-9._:-]{1,256}$/.test(value);
const revisionValid = (value: unknown): value is number => Number.isSafeInteger(value) && (value as number) > 0;
export const notificationText = (value: unknown, max = 256): string => typeof value === 'string' ? value.slice(0, max) : '';
const attentionKinds = new Set(['task.cancel_requested', 'task.failed', 'task.outcome_unknown', 'task.accepted', 'approval.proposed', 'session.cancel_requested', 'session.policy_stop_requested', 'session.outcome_unknown', 'session.late_failure']);
const taskStates = new Set(['draft', 'queued', 'starting', 'ready', 'running', 'cancelled', 'execution_succeeded', 'validation_passed']);
const sessionStates = new Set(['starting', 'ready', 'running', 'stopped']);
const sessionNeedsAttention = (session: ManagedSession) => !sessionStates.has(session.state) || session.turn_state === 'outcome_unknown';

function boundedProjections(tasks: Task[], sessions: ManagedSession[]) {
  const taskMap = new Map<string, Task>();
  const sessionMap = new Map<string, ManagedSession>();
  let limited = tasks.length > NOTIFICATION_LIMITS.tasks || sessions.length > NOTIFICATION_LIMITS.sessions;
  for (const task of tasks.slice(0, NOTIFICATION_LIMITS.tasks)) {
    if (!idValid(task.id) || !revisionValid(task.revision) || taskMap.has(task.id)) { limited = true; continue; }
    taskMap.set(task.id, task);
  }
  for (const session of sessions.slice(0, NOTIFICATION_LIMITS.sessions)) {
    if (!idValid(session.id) || !taskMap.has(session.task_id) || !revisionValid(session.revision) || sessionMap.has(session.id) || taskMap.has(session.id)) { limited = true; continue; }
    sessionMap.set(session.id, session);
  }
  return { taskMap, sessionMap, limited };
}

/** Retain bounded references across page replacement; absence never means resolved. */
export function retainNotificationEvidence(previous: NotificationEvidence, tasks: Task[], sessions: ManagedSession[], events: JournalEvent[]): NotificationEvidence {
  const { taskMap, sessionMap, limited: projectionLimited } = boundedProjections(tasks, sessions);
  let limited = previous.limited || projectionLimited || events.length > NOTIFICATION_LIMITS.events;
  const refs = new Map<string, NotificationEventSource>();
  for (const ref of previous.refs.slice(-NOTIFICATION_LIMITS.events)) refs.set(ref.eventId, ref);
  if (previous.refs.length > NOTIFICATION_LIMITS.events) limited = true;
  for (const event of events.slice(-NOTIFICATION_LIMITS.events)) {
    const session = sessionMap.get(event.aggregate_id);
    const taskId = taskMap.has(event.aggregate_id) ? event.aggregate_id : session?.task_id;
    if (!taskId) continue;
    if (!idValid(event.event_id) || !revisionValid(event.sequence) || !revisionValid(event.revision) || typeof event.kind !== 'string' || event.kind.length > 128) { limited = true; continue; }
    const ref: NotificationEventSource = { taskId, ...(session ? { sessionId: session.id } : {}), eventId: event.event_id, sequence: event.sequence, revision: event.revision, kind: event.kind, source: notificationText(event.source), occurredAt: notificationText(event.occurred_at, 80) };
    const existing = refs.get(ref.eventId);
    if (existing) {
      if (existing.taskId !== ref.taskId || existing.sequence !== ref.sequence || existing.kind !== ref.kind || existing.revision !== ref.revision || existing.sessionId !== ref.sessionId || existing.source !== ref.source) limited = true;
      continue;
    }
    refs.set(ref.eventId, ref);
  }
  const ordered = [...refs.values()].sort((a, b) => a.sequence - b.sequence);
  if (ordered.length > NOTIFICATION_LIMITS.events) limited = true;
  return { refs: ordered.slice(-NOTIFICATION_LIMITS.events), limited };
}

export function aggregateTaskNotifications(tasks: Task[], sessions: ManagedSession[], evidence: NotificationEvidence, observation: NotificationObservation): { rows: TaskNotification[]; limited: boolean } {
  const { taskMap, sessionMap, limited } = boundedProjections(tasks, sessions);
  const rows: TaskNotification[] = [];
  for (const task of taskMap.values()) {
    const relatedSessions = [...sessionMap.values()].filter(session => session.task_id === task.id && sessionNeedsAttention(session));
    const sources = evidence.refs.slice(-NOTIFICATION_LIMITS.events).filter(ref => ref.taskId === task.id).sort((a, b) => b.sequence - a.sequence);
    // A valid Task acceptance projection wins over historical session/event issues.
    let category: NotificationCategory | undefined;
    if (taskHasAcceptanceProjection(task)) category = 'accepted';
    else if (task.state === 'accepted') category = 'acceptanceUnverified';
    else if (task.state === 'failed') category = 'failed';
    else if (['outcome_unknown', 'unknown'].includes(task.state)) category = 'unknown';
    else if (task.state === 'cancel_requested') category = 'cancelRequested';
    else if (task.state === 'awaiting_approval') category = 'approval';
    else if (!taskStates.has(task.state)) category = 'unknown';
    else if (relatedSessions.length) category = 'sessionAttention';
    else if (sources.some(source => attentionKinds.has(source.kind))) category = 'observed';
    if (!category) continue;
    rows.push({ taskId: task.id, goal: notificationText(task.goal, 160), category, stale: !observation.connected || !observation.tasksFresh || (category === 'sessionAttention' && !observation.sessionsFresh), taskRevision: task.revision, taskState: notificationText(task.state, 80), updatedAt: notificationText(task.updated_at, 80), sessions: relatedSessions.map(session => ({ id: session.id, revision: session.revision, state: notificationText(session.state, 80), turnState: notificationText(session.turn_state, 80), updatedAt: notificationText(session.updated_at, 80) })), sources });
  }
  const rank = (row: TaskNotification) => row.category === 'accepted' ? 2 : row.category === 'observed' ? 3 : 1;
  rows.sort((a, b) => rank(a) - rank(b) || a.taskId.localeCompare(b.taskId));
  return { rows, limited: limited || evidence.limited };
}
