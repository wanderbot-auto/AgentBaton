import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { App } from './App';
import { TaskNotifications } from './TaskNotifications';
import { api, DaemonError, type JournalEvent, type Task } from './api';
import { sessionApi, type ManagedSession } from './session-api';
import type { NotificationObservation } from './task-notifications-model';

// Browser/component transport fixtures; these do not prove native IPC or providers.
vi.mock('./api', async importOriginal => {
  const actual = await importOriginal<typeof import('./api')>();
  return { ...actual, api: { ...actual.api, snapshot: vi.fn(), events: vi.fn(), projectsList: vi.fn(), create: vi.fn(), cancel: vi.fn() } };
});
vi.mock('./session-api', () => ({ sessionApi: { sessionsList: vi.fn(), connectionsList: vi.fn() } }));
const task: Task = { id: 'task-1', goal: 'Check notification sources', state: 'failed', revision: 3, created_at: '2026-10-31T00:00:00Z', updated_at: '2026-10-31T00:02:00Z' };
const session: ManagedSession = { id: 'session-1', task_id: task.id, project_id: 'project-1', workspace_id: 'workspace-1', connection_id: 'connection-1', grant_id: 'grant-1', state: 'outcome_unknown', revision: 2, native_session_id: null, created_at: task.created_at, updated_at: task.updated_at, last_error: 'Provider text is not authority', turn_state: null };
const event = (sequence: number): JournalEvent => ({ sequence, event_id: `event-${sequence}`, aggregate_id: session.id, kind: 'session.outcome_unknown', revision: sequence, occurred_at: task.updated_at, source: 'baton_supervisor', payload: { reason: 'Original observation', task_accepted: true } });
const observation: NotificationObservation = { connected: true, tasksFresh: true, sessionsFresh: true, eventsFresh: true, eventLimited: false, eventSequence: 10000 };
const snapshot = (tasks = [task]) => ({ tasks, sequence: 100, api_version: 1, database_schema_version: 17 });
beforeEach(() => {
  vi.restoreAllMocks(); vi.resetAllMocks(); localStorage.clear(); localStorage.setItem('agentbaton.language', 'en');
  vi.mocked(api.snapshot).mockResolvedValue(snapshot());
  vi.mocked(api.events).mockResolvedValue({ events: [event(100)], next_cursor: 100, sequence: 100, gap: false });
  vi.mocked(api.projectsList).mockResolvedValue([]);
  vi.mocked(sessionApi.sessionsList).mockResolvedValue([session]);
  vi.mocked(sessionApi.connectionsList).mockResolvedValue([]);
});

describe('in-app Task notifications', () => {
  it('renders one notice for 100 events, pages sources, opens a real reference and restores focus', async () => {
    const user = userEvent.setup(), onOpenTask = vi.fn(), onViewEvent = vi.fn();
    render(<TaskNotifications tasks={[task]} sessions={[session]} events={Array.from({ length: 100 }, (_, index) => event(index + 1))} language="en" observation={observation} onOpenTask={onOpenTask} onViewEvent={onViewEvent} />);
    const trigger = screen.getByRole('button', { name: 'Notifications (1)' });
    await user.click(trigger);
    const dialog = screen.getByRole('dialog', { name: 'Task notifications' });
    expect(within(dialog).getAllByTestId('task-notification')).toHaveLength(1);
    await user.click(within(dialog).getByText('Sources (100)'));
    expect(within(dialog).getByText('Page 1 / 10')).toBeVisible();
    expect(within(dialog).getAllByText('session.outcome_unknown')).toHaveLength(10);
    await user.click(within(dialog).getAllByText('session.outcome_unknown')[0]);
    expect(within(dialog).getByText('event-100')).toBeVisible();
    const originalSource = within(dialog).getByText('event-100').closest('details')!;
    expect(within(originalSource).getByText('baton_supervisor')).toBeVisible();
    await user.click(within(originalSource).getByRole('button', { name: 'View loaded original event' }));
    expect(onViewEvent).toHaveBeenCalledWith('event-100');
    expect(onOpenTask).not.toHaveBeenCalled();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it('explicit DND persists and hides emphasis/count while keeping manual evidence and Task navigation', async () => {
    const user = userEvent.setup(), onOpenTask = vi.fn();
    render(<TaskNotifications tasks={[task]} sessions={[session]} events={[event(1)]} language="en" observation={observation} onOpenTask={onOpenTask} onViewEvent={vi.fn()} />);
    const trigger = screen.getByRole('button', { name: 'Notifications (1)' });
    expect(trigger).toHaveAttribute('data-attention', 'true');
    await user.click(trigger);
    await user.click(screen.getByRole('checkbox', { name: 'Do not disturb' }));
    expect(localStorage.getItem('agentbaton.notifications.doNotDisturb')).toBe('true');
    expect(trigger).toHaveAttribute('aria-label', 'Notifications · do not disturb');
    expect(trigger).not.toHaveAttribute('data-attention');
    expect(screen.getByTestId('task-notification')).toHaveTextContent('Task failed');
    await user.click(screen.getByRole('button', { name: 'Open task' }));
    expect(onOpenTask).toHaveBeenCalledWith(task.id);
    await user.click(screen.getByRole('button', { name: 'Notifications · do not disturb' }));
    expect(screen.getByRole('checkbox', { name: 'Do not disturb' })).toBeChecked();
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(api.cancel).not.toHaveBeenCalled();
    expect(api.create).not.toHaveBeenCalled();
  });

  it('explains preference storage failure without losing the current-window setting', async () => {
    const user = userEvent.setup();
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => { throw new Error('Storage denied'); });
    render(<TaskNotifications tasks={[task]} sessions={[]} events={[]} language="en" observation={observation} onOpenTask={vi.fn()} onViewEvent={vi.fn()} />);
    await user.click(screen.getByRole('button', { name: 'Notifications (1)' }));
    await user.click(screen.getByRole('checkbox', { name: 'Do not disturb' }));
    expect(screen.getByRole('checkbox')).toBeChecked();
    expect(screen.getByRole('status')).toHaveTextContent('This preference could not be saved. It applies to this window only.');
  });

  it('keeps old references when pages disappear and renders source text safely', async () => {
    const user = userEvent.setup(), onViewEvent = vi.fn();
    const source = { ...event(1), source: '<img src=x onerror=alert(1)>', payload: { text: '<script>approved()</script>' } };
    const props = { tasks: [{ ...task, state: 'running' }], sessions: [session], language: 'en' as const, observation, onOpenTask: vi.fn(), onViewEvent };
    const mounted = render(<TaskNotifications {...props} events={[source]} />);
    mounted.rerender(<TaskNotifications {...props} sessions={[]} events={[]} observation={{ ...observation, eventsFresh: false, eventLimited: true }} />);
    await user.click(screen.getByRole('button', { name: 'Notifications (1)' }));
    expect(screen.getByTestId('task-notification')).toHaveTextContent('Past observation to review');
    expect(screen.getByText(/Evidence is limited/)).toBeVisible();
    await user.click(screen.getByText('Sources (1)'));
    await user.click(screen.getByText('session.outcome_unknown'));
    expect(screen.getByText(source.source)).toBeVisible();
    expect(document.querySelector('img')).toBeNull();
    expect(screen.getByText(/Original event is outside the loaded page/)).toBeVisible();
    expect(screen.queryByRole('button', { name: 'View loaded original event' })).not.toBeInTheDocument();
    expect(onViewEvent).not.toHaveBeenCalled();
  });

  it('uses actual App projections and navigates to loaded original events and the same Task', async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText('Local daemon connected');
    await waitFor(() => expect(sessionApi.sessionsList).toHaveBeenCalled());
    await user.click(screen.getByRole('button', { name: 'Notifications (1)' }));
    await user.click(screen.getByText('Sources (1)'));
    await user.click(screen.getByText('session.outcome_unknown'));
    await user.click(screen.getByRole('button', { name: 'View loaded original event' }));
    expect(screen.getByRole('heading', { name: 'History' })).toBeVisible();
    const original = screen.getByText(/Original observation/).closest('details');
    expect(original).toHaveAttribute('open');
    expect(api.events).toHaveBeenCalledWith(0);
    // The same source must reposition History again after the user scrolls away.
    const originalRow = document.getElementById('journal-event-event-100')!;
    const scrollIntoView = vi.fn();
    Object.defineProperty(originalRow, 'scrollIntoView', { configurable: true, value: scrollIntoView });
    await user.click(screen.getByRole('button', { name: 'Notifications (1)' }));
    await user.click(screen.getByText('Sources (1)'));
    await user.click(within(screen.getByRole('dialog')).getByText('session.outcome_unknown'));
    await user.click(screen.getByRole('button', { name: 'View loaded original event' }));
    await waitFor(() => expect(scrollIntoView).toHaveBeenCalledExactlyOnceWith({ block: 'nearest' }));
    expect(original).toHaveAttribute('open');
    await user.click(screen.getByRole('button', { name: 'Notifications (1)' }));
    await user.click(screen.getByRole('button', { name: 'Open task' }));
    expect(screen.getByTestId('task-thread')).toHaveAttribute('data-task-id', task.id);
  });

  it('revokes freshness during refresh, preserves partial-source uncertainty and ignores old generations', async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText('Local daemon connected');
    await user.click(screen.getByRole('button', { name: 'Notifications (1)' }));
    await waitFor(() => expect(screen.queryByText('Event observations are old or unavailable.')).not.toBeInTheDocument());
    await user.click(screen.getByRole('button', { name: 'Close notifications' }));
    let resolveOld!: (value: ReturnType<typeof snapshot>) => void;
    vi.mocked(api.snapshot).mockImplementationOnce(() => new Promise(resolve => { resolveOld = resolve; }));
    fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
    await user.click(screen.getByRole('button', { name: 'Notifications (1)' }));
    expect(screen.getByTestId('task-notification')).toHaveTextContent('Last observed');
    expect(screen.getByText('Event observations are old or unavailable.')).toBeVisible();
    await user.click(screen.getByRole('button', { name: 'Close notifications' }));
    vi.mocked(sessionApi.sessionsList).mockRejectedValueOnce(new DaemonError('CONFLICT', 'Session projection unavailable', 'none'));
    vi.mocked(api.events).mockResolvedValueOnce({ events: [], next_cursor: 50, sequence: 100, gap: true });
    fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
    await screen.findByText('Session projection unavailable', { exact: false });
    await act(async () => resolveOld(snapshot([{ ...task, state: 'accepted' }])));
    await user.click(screen.getByRole('button', { name: 'Notifications (1)' }));
    expect(screen.getByTestId('task-notification')).toHaveTextContent('Task failed');
    expect(screen.getByTestId('task-notification')).toHaveTextContent('Latest task status');
    expect(screen.getByText('Session observations are old or unavailable.')).toBeVisible();
    expect(screen.getByText('Event observations are old or unavailable.')).toBeVisible();
    expect(screen.getByText(/Evidence is limited/)).toBeVisible();
    expect(screen.queryByText('Task accepted')).not.toBeInTheDocument();
  });

  it('limits rendered Task rows and provides Chinese controls', async () => {
    const user = userEvent.setup();
    render(<TaskNotifications tasks={Array.from({ length: 25 }, (_, index) => ({ ...task, id: `task-${index}` }))} sessions={[]} events={[]} language="zh" observation={observation} onOpenTask={vi.fn()} onViewEvent={vi.fn()} />);
    await user.click(screen.getByRole('button', { name: '通知 (25)' }));
    expect(screen.getAllByTestId('task-notification')).toHaveLength(20);
    await user.click(screen.getByRole('button', { name: '下一页' }));
    expect(screen.getAllByTestId('task-notification')).toHaveLength(5);
    expect(screen.getByRole('checkbox', { name: '免打扰' })).toBeVisible();
  });

  it('rejects reversed conflicting originals in the App and keeps the conflict after a later page', async () => {
    const user = userEvent.setup();
    const otherTask: Task = { ...task, id: 'task-2', goal: 'Different task', state: 'draft' };
    const first = event(1);
    vi.mocked(api.snapshot).mockResolvedValue(snapshot([task, otherTask]));
    vi.mocked(api.events).mockResolvedValueOnce({ events: [first], next_cursor: 100, sequence: 100, gap: false });
    render(<App />);
    await screen.findByText('Local daemon connected');
    await user.click(screen.getByRole('button', { name: 'Notifications (1)' }));
    await user.click(screen.getByText('Sources (1)'));
    await user.click(within(screen.getByRole('dialog')).getByText('session.outcome_unknown'));
    expect(screen.getByRole('button', { name: 'View loaded original event' })).toBeVisible();
    await user.click(screen.getByRole('button', { name: 'Close notifications' }));
    const wrongOriginal = { ...event(2), event_id: first.event_id, aggregate_id: otherTask.id, payload: { reason: 'Other Task original must not be selected' } };
    vi.mocked(api.events).mockResolvedValueOnce({ events: [wrongOriginal, first], next_cursor: 100, sequence: 100, gap: false });
    await user.click(screen.getByRole('button', { name: 'Refresh' }));
    await user.click(screen.getByRole('button', { name: 'Notifications (1)' }));
    await user.click(screen.getByText('Sources (1)'));
    await user.click(within(screen.getByRole('dialog')).getByText('session.outcome_unknown'));
    expect(screen.getByText(/Conflicting event IDs were observed/)).toBeVisible();
    expect(screen.getByText(/Evidence is limited/)).toBeVisible();
    expect(screen.queryByRole('button', { name: 'View loaded original event' })).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'History' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Close notifications' }));
    vi.mocked(api.events).mockResolvedValueOnce({ events: [first], next_cursor: 100, sequence: 100, gap: false });
    await user.click(screen.getByRole('button', { name: 'Refresh' }));
    await user.click(screen.getByRole('button', { name: 'Notifications (1)' }));
    await user.click(screen.getByText('Sources (1)'));
    await user.click(within(screen.getByRole('dialog')).getByText('session.outcome_unknown'));
    expect(screen.getByText(/Conflicting event IDs were observed/)).toBeVisible();
    expect(screen.queryByRole('button', { name: 'View loaded original event' })).not.toBeInTheDocument();
  });

  it('blocks global shortcuts behind the notification modal and restores the background on Escape', async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText('Local daemon connected');
    await user.click(screen.getByTestId('task-card'));
    const thread = screen.getByTestId('task-thread');
    const background = thread.closest('.shell')!.parentElement!;
    const previousInert = background.inert, previousHidden = background.getAttribute('aria-hidden');
    await user.click(screen.getByRole('button', { name: 'Notifications (1)' }));
    expect(background.inert).toBe(true);
    await user.keyboard('{Control>}k{/Control}');
    await user.keyboard('{Control>}n{/Control}');
    await user.keyboard('{Meta>}k{/Meta}');
    await user.keyboard('{Meta>}n{/Meta}');
    expect(screen.getAllByRole('dialog', { hidden: true })).toHaveLength(1);
    expect(screen.getByRole('dialog', { name: 'Task notifications' })).toBeVisible();
    expect(screen.getByTestId('task-thread')).toBe(thread);
    expect(thread).toHaveAttribute('data-task-id', task.id);
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog', { hidden: true })).not.toBeInTheDocument();
    expect(background.inert).toBe(previousInert);
    expect(background.getAttribute('aria-hidden')).toBe(previousHidden);
    await user.click(screen.getByRole('button', { name: 'Notifications (1)' }));
    expect(screen.getByRole('dialog', { name: 'Task notifications' })).toBeVisible();
  });

  it('uses the successful event batch upper bound, allowing later events and rejecting future references', async () => {
    const user = userEvent.setup();
    // Snapshot is at 100; a separately fetched legitimate batch is at 101.
    vi.mocked(api.events).mockResolvedValue({ events: [event(101), event(999)], next_cursor: 101, sequence: 101, gap: false });
    render(<App />);
    await screen.findByText('Local daemon connected');
    await user.click(screen.getByRole('button', { name: 'Notifications (1)' }));
    await user.click(screen.getByText('Sources (1)'));
    await user.click(within(screen.getByRole('dialog')).getByText('session.outcome_unknown'));
    expect(screen.getByText('event-101')).toBeVisible();
    expect(screen.queryByText('event-999')).not.toBeInTheDocument();
    expect(screen.getByText(/Evidence is limited/)).toBeVisible();
    expect(screen.getByRole('button', { name: 'View loaded original event' })).toBeVisible();
  });
});
