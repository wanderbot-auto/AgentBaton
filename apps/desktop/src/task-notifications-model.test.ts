import { describe, expect, it } from 'vitest';
import type { JournalEvent, Task } from './api';
import type { ManagedSession } from './session-api';
import { aggregateTaskNotifications, NOTIFICATION_LIMITS, retainNotificationEvidence, type NotificationObservation } from './task-notifications-model';

const task: Task = { id: 'task-1', project_id: 'project-1', goal: 'Review real evidence', state: 'running', revision: 3, created_at: '2026-10-31T00:00:00Z', updated_at: '2026-10-31T00:02:00Z' };
const session: ManagedSession = { id: 'session-1', task_id: task.id, project_id: 'project-1', workspace_id: 'workspace-1', connection_id: 'connection-1', grant_id: 'grant-1', state: 'ready', revision: 3, native_session_id: null, created_at: task.created_at, updated_at: task.updated_at, last_error: null, turn_state: null };
const observation: NotificationObservation = { connected: true, tasksFresh: true, sessionsFresh: true, eventsFresh: true, eventLimited: false };
const event = (sequence: number, overrides: Partial<JournalEvent> = {}): JournalEvent => ({ sequence, event_id: `event-${sequence}`, aggregate_id: session.id, revision: sequence, kind: 'session.outcome_unknown', occurred_at: task.updated_at, source: 'baton_supervisor', payload: { reason: 'failed' }, ...overrides });
const empty = { refs: [], limited: false };

describe('Task notification observation model', () => {
  it('aggregates 100 repeated events into one Task notice and retains every real reference', () => {
    const events = Array.from({ length: 100 }, (_, index) => event(index + 1));
    const evidence = retainNotificationEvidence(empty, [task], [session], events);
    const result = aggregateTaskNotifications([task], [session], evidence, observation);
    expect(result.rows).toHaveLength(1);
    expect(result.rows[0]).toMatchObject({ taskId: task.id, category: 'observed', stale: false });
    expect(result.rows[0].sources).toHaveLength(100);
    expect(new Set(result.rows[0].sources.map(source => source.eventId)).size).toBe(100);
    expect(result.rows[0].sources[0]).toMatchObject({ eventId: 'event-100', sessionId: session.id, taskId: task.id, source: 'baton_supervisor' });
    expect(retainNotificationEvidence(evidence, [task], [session], events).refs).toHaveLength(100);
  });

  it('maps only known aggregate/session IDs and never derives authority or state from payload/text', () => {
    const events = [event(1, { aggregate_id: 'unknown', payload: { task_id: task.id, state: 'accepted', approval: true } }), event(2, { kind: 'session.update', payload: { task_id: task.id, text: 'task.accepted approval.proposed AUTH_REQUIRED budget exceeded configuration drift' } }), event(3, { kind: 'task.accepted', aggregate_id: task.id, payload: { accepted: true } })];
    const evidence = retainNotificationEvidence(empty, [task], [session], events);
    expect(evidence.refs.map(ref => ref.eventId)).toEqual(['event-2', 'event-3']);
    expect(aggregateTaskNotifications([task], [session], evidence, observation).rows[0].category).toBe('observed');
    expect(JSON.stringify(evidence)).not.toContain('AUTH_REQUIRED');
    expect(aggregateTaskNotifications([task], [session], retainNotificationEvidence(empty, [task], [session], [events[1]]), observation).rows).toEqual([]);
  });

  it('requires existing Task acceptance projection and gives it priority over historical failures', () => {
    const accepted: Task = { ...task, state: 'accepted', spec_revision: 1, acceptance_ref: 'acceptance:1', acceptance_sequence: 12 };
    const failedSession: ManagedSession = { ...session, state: 'failed' };
    const evidence = retainNotificationEvidence(empty, [accepted], [failedSession], [event(11)]);
    expect(aggregateTaskNotifications([accepted], [failedSession], evidence, observation).rows[0].category).toBe('accepted');
    expect(aggregateTaskNotifications([{ ...accepted, acceptance_ref: null }], [], empty, observation).rows[0].category).toBe('acceptanceUnverified');
    expect(aggregateTaskNotifications([{ ...task, state: 'execution_succeeded' }], [], empty, observation).rows).toEqual([]);
  });

  it('keeps session failure separate from Task failure and never treats stops/turn completion as acceptance', () => {
    expect(aggregateTaskNotifications([task], [{ ...session, state: 'failed' }], empty, observation).rows[0].category).toBe('sessionAttention');
    expect(aggregateTaskNotifications([{ ...task, state: 'failed' }], [], empty, observation).rows[0].category).toBe('failed');
    expect(aggregateTaskNotifications([task], [{ ...session, state: 'stopped', turn_state: 'completed' }], retainNotificationEvidence(empty, [task], [session], [event(1, { kind: 'session.stopped', payload: { task_accepted: true, process_reaped: true } }), event(2, { kind: 'session.turn_ended' })]), observation).rows).toEqual([]);
    expect(aggregateTaskNotifications([{ ...task, state: 'cancel_requested' }], [], empty, observation).rows[0].category).toBe('cancelRequested');
    expect(aggregateTaskNotifications([task], [{ ...session, state: 'cancel_requested' }], empty, observation).rows[0].category).toBe('sessionAttention');
    expect(aggregateTaskNotifications([{ ...task, state: 'new_unknown_state' }], [], empty, observation).rows[0].category).toBe('unknown');
  });

  it('retains historical concerns across empty/replaced windows without claiming recovery', () => {
    const first = retainNotificationEvidence(empty, [task], [session], [event(1)]);
    const next = retainNotificationEvidence(first, [task], [], []);
    const row = aggregateTaskNotifications([task], [], next, { ...observation, eventLimited: true }).rows[0];
    expect(row.category).toBe('observed');
    expect(row.sources[0]).toMatchObject({ eventId: 'event-1', sessionId: session.id });
    expect(aggregateTaskNotifications([{ ...task, state: 'failed' }], [], next, { ...observation, tasksFresh: false }).rows[0].stale).toBe(true);
    expect(aggregateTaskNotifications([task], [{ ...session, state: 'failed' }], next, { ...observation, sessionsFresh: false }).rows[0].stale).toBe(true);
    expect(aggregateTaskNotifications([{ ...task, state: 'failed' }], [], next, { ...observation, connected: false }).rows[0].stale).toBe(true);
  });

  it('bounds records, projection inputs and text, and reports lost/conflicting evidence', () => {
    const events = Array.from({ length: NOTIFICATION_LIMITS.events + 1 }, (_, index) => event(index + 1));
    const evidence = retainNotificationEvidence(empty, [task], [session], events);
    expect(evidence.refs).toHaveLength(NOTIFICATION_LIMITS.events);
    expect(evidence.limited).toBe(true);
    expect(evidence.refs[0].eventId).toBe('event-2');
    const conflict = retainNotificationEvidence(evidence, [task], [session], [event(2, { source: 'different' })]);
    expect(conflict.limited).toBe(true);
    expect(conflict.refs[0].source).toBe('baton_supervisor');
    const manyTasks = Array.from({ length: NOTIFICATION_LIMITS.tasks + 1 }, (_, index) => ({ ...task, id: `task-${index}`, state: 'failed', goal: 'x'.repeat(1000) }));
    const bounded = aggregateTaskNotifications(manyTasks, [], empty, observation);
    expect(bounded.limited).toBe(true);
    expect(bounded.rows).toHaveLength(NOTIFICATION_LIMITS.tasks);
    expect(bounded.rows.every(row => row.goal.length === 160)).toBe(true);
    const malformed = retainNotificationEvidence(empty, [task], [session], [event(1, { event_id: 'x'.repeat(257) }), event(0), event(2, { kind: 'x'.repeat(129) })]);
    expect(malformed).toEqual({ refs: [], limited: true });
  });
});
