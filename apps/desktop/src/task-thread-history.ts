import { useEffect, useRef, useState } from 'react';
import { request, type JournalEvent } from './api';
import { earlierEventWindow } from './event-window';
import { sessionEventText } from './ManagedSessions';

type Position = { before?: number; after?: number };
const limit = 60;
const empty = { events: [] as JournalEvent[], before: null as number | null, ready: false, replaced: false, detached: false };
const sequence = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value > 0;

async function page(taskId: string, position: Position = {}) {
  const value = await request<{ task_id: string; events: JournalEvent[]; next_cursor: number | null }>(
    'tasks_history', { taskId, ...position, limit },
  );
  if (!value || value.task_id !== taskId || !Array.isArray(value.events) || value.events.length > limit) {
    throw new Error('Invalid task history page');
  }
  let previous = position.after ?? 0;
  for (const event of value.events) {
    if (!event || !sequence(event.sequence) || event.sequence <= previous
      || (position.before !== undefined && event.sequence >= position.before)
      || typeof event.event_id !== 'string' || typeof event.aggregate_id !== 'string'
      || sessionEventText(event) === null) throw new Error('Invalid task history entry');
    previous = event.sequence;
  }
  const cursorEntry = position.after === undefined ? value.events[0] : value.events.at(-1);
  if (value.next_cursor !== null && (!sequence(value.next_cursor) || value.next_cursor !== cursorEntry?.sequence)) {
    throw new Error('Invalid task history cursor');
  }
  return value;
}

export function useTaskThreadHistory(taskId: string | undefined, connected: boolean, revision: string) {
  const [state, setState] = useState(() => ({ ...empty, taskId }));
  const [loading, setLoading] = useState(false);
  const [moreRecent, setMoreRecent] = useState(false);
  const [fault, setFault] = useState<Position | null>(null);
  const [retryCount, retry] = useState(0);
  const [latestCount, showLatest] = useState(0);
  const lastLatestRequest = useRef(0);
  const current = state.taskId === taskId ? state : { ...empty, taskId };
  const context = useRef({ taskId, connected, epoch: 0 });
  if (context.current.taskId !== taskId || context.current.connected !== connected) {
    context.current = { taskId, connected, epoch: context.current.epoch + 1 };
  }
  const pastRead = useRef<number | undefined>(undefined);
  const liveAfter = useRef(0);
  const faultRef = useRef(fault);
  faultRef.current = fault;

  useEffect(() => {
    if (!taskId || !connected) return;
    const forceLatest = latestCount !== lastLatestRequest.current;
    lastLatestRequest.current = latestCount;
    if (current.detached && !forceLatest) return;
    let active = true;
    const position = retryCount > 0 && !current.ready ? {} : liveAfter.current > 0 ? { after: liveAfter.current } : {};
    // A new task must start at its own latest page, even when its sequence is older.
    const from = !forceLatest && state.taskId === taskId && current.ready ? position : {};
    if (!current.ready) setLoading(true);
    setMoreRecent(false);
    setFault(previous => previous?.before !== undefined && state.taskId === taskId ? previous : null);
    void (async () => {
      let cursor: Position = from;
      for (let pages = 0; pages < 4; pages++) {
        const requestPosition = cursor;
        const next = await page(taskId, requestPosition);
        if (!active) return;
        if (next.events.length) liveAfter.current = next.events.at(-1)!.sequence;
        else if (cursor.after === undefined) liveAfter.current = 0;
        setState(previous => {
          if (requestPosition.after === undefined || previous.taskId !== taskId) {
            return { taskId, ready: true, events: next.events, before: next.next_cursor, replaced: false, detached: false };
          }
          if (previous.detached) return previous;
          const merged = earlierEventWindow(next.events, previous.events);
          return {
            ...previous,
            events: merged.replaced ? next.events : [...previous.events, ...next.events],
            before: merged.replaced ? next.events[0].sequence : previous.before,
            replaced: previous.replaced || merged.replaced,
          };
        });
        if (cursor.after === undefined || next.next_cursor === null) break;
        cursor = { after: next.next_cursor };
        if (pages === 3) setMoreRecent(true);
      }
    })().catch(() => { if (active) setFault(previous => previous?.before !== undefined ? previous : from); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [taskId, connected, revision, retryCount, latestCount]);

  async function loadEarlier(before = current.before) {
    if (!taskId || !connected || before === null || pastRead.current === context.current.epoch) return;
    const stamp = context.current.epoch;
    pastRead.current = stamp;
    setLoading(true);
    setFault(null);
    try {
      const next = await page(taskId, { before });
      if (context.current.epoch !== stamp) return;
      setState(previous => {
        if (previous.taskId !== taskId || previous.before !== before) return previous;
        const merged = earlierEventWindow(next.events, previous.events);
        return { ...previous, events: merged.events, before: next.next_cursor, replaced: merged.replaced, detached: previous.detached || merged.replaced };
      });
    } catch {
      if (context.current.epoch === stamp) setFault({ before });
    } finally {
      if (pastRead.current === stamp) pastRead.current = undefined;
      if (context.current.epoch === stamp) setLoading(false);
    }
  }

  return {
    ...current,
    loading: connected && (loading || !current.ready && !fault),
    error: state.taskId === taskId && fault !== null || !!taskId && !current.ready && fault !== null,
    loadEarlier: () => void loadEarlier(),
    loadLatest: () => showLatest(n => n + 1),
    moreRecent: !current.detached && moreRecent,
    loadRecent: () => retry(n => n + 1),
    retry: () => faultRef.current?.before !== undefined ? void loadEarlier(faultRef.current.before) : current.detached ? showLatest(n => n + 1) : retry(n => n + 1),
  };
}

/** Human-readable projection of a retained scheduler prompt. The full
 * document remains available in task evidence and is never used for actions. */
export function taskMessageText(event: JournalEvent) {
  const body = sessionEventText(event);
  if (event.kind !== 'session.message_queued' || !body?.startsWith('{"budget":')) return body;
  try {
    const envelope = JSON.parse(body);
    if (typeof envelope.goal === 'string' && typeof envelope.idempotency_key === 'string'
      && /^goal-turn:[a-f0-9]{64}$/.test(envelope.idempotency_key) && Array.isArray(envelope.inputs)) {
      return envelope.goal;
    }
  } catch { /* Ordinary user text can begin with JSON. */ }
  return body;
}

export function groupTranscript(events: JournalEvent[], text: (event: JournalEvent) => string | null, userKind: string) {
  const groups: { first: JournalEvent; parts: JournalEvent[]; text: string; user: boolean }[] = [];
  for (const event of events) {
    const body = text(event);
    if (body === null) continue;
    const user = event.kind === userKind;
    const previous = groups.at(-1);
    if (!user && previous && !previous.user && previous.first.aggregate_id === event.aggregate_id) {
      previous.text += body;
      previous.parts.push(event);
    } else {
      groups.push({ first: event, parts: [event], text: body, user });
    }
  }
  return groups;
}
