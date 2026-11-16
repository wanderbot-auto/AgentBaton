import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { TaskNotifications } from './TaskNotifications';
import { aggregateTaskNotifications, retainNotificationEvidence } from './task-notifications-model';
import type { Task } from './api';
vi.mock('./task-notifications-model', async original => {
  const actual = await original<typeof import('./task-notifications-model')>();
  return { ...actual, aggregateTaskNotifications: vi.fn(actual.aggregateTaskNotifications), retainNotificationEvidence: vi.fn(actual.retainNotificationEvidence) };
});
beforeEach(() => { vi.clearAllMocks(); localStorage.clear(); });
const task: Task = { id: 'task-1', goal: 'Review failed task', state: 'failed', revision: 1, created_at: 'now', updated_at: 'now' };
it('reuses evidence and aggregation for unchanged data, including new observation objects', () => {
  const props = { tasks: [task], sessions: [], events: [], language: 'en' as const, observation: { connected: true, tasksFresh: true, sessionsFresh: true, eventsFresh: true, eventSequence: 0, eventLimited: false }, onOpenTask: vi.fn(), onViewEvent: vi.fn() };
  const { rerender } = render(<TaskNotifications {...props} />);
  const evidenceCalls = vi.mocked(retainNotificationEvidence).mock.calls.length;
  const modelCalls = vi.mocked(aggregateTaskNotifications).mock.calls.length;
  for (let i = 0; i < 10; i++) rerender(<TaskNotifications {...props} observation={{ ...props.observation }} onOpenTask={() => {}} />);
  expect(retainNotificationEvidence).toHaveBeenCalledTimes(evidenceCalls);
  expect(aggregateTaskNotifications).toHaveBeenCalledTimes(modelCalls);
  fireEvent.click(screen.getByRole('button', { name: 'Notifications (1)' }));
  expect(aggregateTaskNotifications).toHaveBeenCalledTimes(modelCalls);
  rerender(<TaskNotifications {...props} observation={{ ...props.observation, tasksFresh: false }} />);
  expect(aggregateTaskNotifications).toHaveBeenCalledTimes(modelCalls + 1);
  expect(screen.getByTestId('task-notification')).toHaveTextContent('Last observed · refresh pending or unavailable');
  rerender(<TaskNotifications {...props} tasks={[{ ...task, state: 'cancel_requested', revision: 2 }]} />);
  expect(screen.getByTestId('task-notification')).toHaveTextContent('Cancellation requested');
});
