import { describe, expect, it } from 'vitest';
import { earlierEventWindow, MAX_EVENT_WINDOW_BYTES } from './event-window';
import type { JournalEvent } from './api';
const event = (sequence: number, text: string): JournalEvent => ({ sequence, event_id: `event-${sequence}`, aggregate_id: 'task', revision: 1, kind: 'session.update', occurred_at: '2026-10-27T00:00:00Z', source: 'generated_fixture', payload: { text } });
describe('byte-bounded visible event history', () => {
  it('preserves order and input arrays while the combined window fits', () => {
    const older = [event(1, 'earlier')]; const current = [event(2, 'newer')];
    expect(earlierEventWindow(older, current)).toEqual({ events: [...older, ...current], replaced: false, reason: null });
    expect(older.map(record => record.sequence)).toEqual([1]);
    expect(current.map(record => record.sequence)).toEqual([2]);
  });
  it('bounds large Unicode records by encoded bytes rather than count or UTF-16 length', () => {
    const text = '界'.repeat(400_000);
    const older = [event(1, text), event(2, text)];
    const current = Array.from({ length: 6 }, (_, index) => event(index + 3, text));
    const result = earlierEventWindow(older, current);
    expect(result).toEqual({ events: older, replaced: true, reason: 'bytes' });
    expect(new TextEncoder().encode(JSON.stringify(result.events)).byteLength).toBeLessThan(MAX_EVENT_WINDOW_BYTES);
    expect(current.map(record => record.sequence)).toEqual([3, 4, 5, 6, 7, 8]);
  });
  it('keeps the existing count bound without silently slicing away the requested older page', () => {
    const older = Array.from({ length: 100 }, (_, index) => event(index + 1, 'small'));
    const current = Array.from({ length: 500 }, (_, index) => event(index + 101, 'small'));
    expect(earlierEventWindow(older, current)).toEqual({ events: older, replaced: true, reason: 'count' });
    expect(current).toHaveLength(500);
  });
  it('rejects an invalid oversized incoming page while leaving the existing window untouched', () => {
    const current = [event(1, 'keep')];
    expect(() => earlierEventWindow([event(2, 'x'.repeat(MAX_EVENT_WINDOW_BYTES + 1))], current)).toThrow('local journal was not changed');
    expect(current).toEqual([event(1, 'keep')]);
  });
});
