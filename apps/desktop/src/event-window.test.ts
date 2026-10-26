import { describe, expect, it } from 'vitest';
import { earlierEventWindow } from './event-window';
import type { JournalEvent } from './api';
const event = (sequence: number, text: string): JournalEvent => ({ sequence, event_id: `event-${sequence}`, aggregate_id: 'task', revision: 1, kind: 'session.provider_update', occurred_at: '2026-10-26T00:00:00Z', source: 'generated_fixture', payload: { text } });
describe('bounded visible journal window', () => {
  it('continues to older pages when the current record-count window is full', () => {
    const older = Array.from({ length: 100 }, (_, index) => event(index + 1, 'small'));
    const current = Array.from({ length: 500 }, (_, index) => event(index + 101, 'small'));
    const result = earlierEventWindow(older, current);
    expect(result.replaced).toBe(true);
    expect(result.events.map(event => event.sequence)).toEqual(older.map(event => event.sequence));
    expect(current).toHaveLength(500);
  });
  it('replaces the view with the requested older page before large Unicode records accumulate', () => {
    const text = '界'.repeat(400_000);
    const older = [event(1, text), event(2, text)];
    const current = Array.from({ length: 6 }, (_, index) => event(index + 3, text));
    const result = earlierEventWindow(older, current);
    expect(result.replaced).toBe(true);
    expect(result.events.map(event => event.sequence)).toEqual([1, 2]);
    expect(current.map(event => event.sequence)).toEqual([3, 4, 5, 6, 7, 8]);
  });
});
