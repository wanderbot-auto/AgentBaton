import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { request, type JournalEvent } from './api';
import { groupTranscript, taskMessageText, useTaskThreadHistory } from './task-thread-history';

vi.mock('./api', async original => ({ ...await original<typeof import('./api')>(), request: vi.fn() }));
const message = (sequence: number, text: string, agent = false): JournalEvent => ({
  sequence, event_id: `message-${sequence}`, aggregate_id: 'session', revision: sequence,
  kind: agent ? 'session.update' : 'session.message_queued', occurred_at: '2026-01-01T00:00:00Z',
  payload: agent ? { update: { kind: { kind: 'agent_text', text } } } : { text }, source: 'fixture',
});
const page = (task_id: string, events: JournalEvent[], next_cursor: number | null = null) => ({ task_id, events, next_cursor });
beforeEach(() => vi.resetAllMocks());

it('loads scoped pages and reconciles new messages without dropping older pages', async () => {
  const old = message(2, 'Earlier question'), initial = message(48, 'Later reply', true), added = message(240, 'Next question');
  vi.mocked(request)
    .mockResolvedValueOnce(page('task', [initial], initial.sequence))
    .mockResolvedValueOnce(page('task', [old]))
    .mockResolvedValueOnce(page('task', [added]));
  const { result, rerender } = renderHook(({revision}) => useTaskThreadHistory('task', true, revision), { initialProps: { revision: '1' } });
  await waitFor(() => expect(result.current.events).toEqual([initial]));
  act(() => result.current.loadEarlier());
  await waitFor(() => expect(result.current.events).toEqual([old, initial]));
  rerender({ revision: '2' });
  await waitFor(() => expect(result.current.events).toEqual([old, initial, added]));
  expect(vi.mocked(request).mock.calls).toEqual([
    ['tasks_history', { taskId: 'task', limit: 60 }],
    ['tasks_history', { taskId: 'task', before: 48, limit: 60 }],
    ['tasks_history', { taskId: 'task', after: 48, limit: 60 }],
  ]);
});

it('rejects wrong-task responses and allows a retry without substituting journal data', async () => {
  vi.mocked(request).mockResolvedValueOnce(page('different', [message(1, 'Wrong task')])).mockResolvedValueOnce(page('task', [message(2, 'Right task')]));
  const { result } = renderHook(() => useTaskThreadHistory('task', true, '1'));
  await waitFor(() => expect(result.current.error).toBe(true));
  expect(result.current.events).toEqual([]);
  act(() => result.current.retry());
  await waitFor(() => expect(result.current.events).toEqual([message(2, 'Right task')]));
  expect(result.current.error).toBe(false);
});

it('displays a scheduler step without its low-level execution envelope', () => {
  const envelope = JSON.stringify({ budget: {}, goal: 'Validate response time', idempotency_key: `goal-turn:${'a'.repeat(64)}`, inputs: [] });
  const entries = [message(1, envelope), message(2, 'First ', true), message(3, 'result', true)];
  expect(groupTranscript(entries, taskMessageText, 'session.message_queued').map(turn => turn.text)).toEqual(['Validate response time', 'First result']);
  const ordinary = '{"budget":"ordinary message"}';
  expect(taskMessageText(message(4, ordinary))).toBe(ordinary);
});
