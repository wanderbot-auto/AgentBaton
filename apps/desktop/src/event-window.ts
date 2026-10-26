import type { JournalEvent } from './api';

export const MAX_EVENT_WINDOW_BYTES = 8 * 1024 * 1024;
// Count bounds do not bound large message strings. When older history cannot
// fit with the current window, display its page alone; use its forward cursor
// to return to newer records. The local journal is unchanged.
export function earlierEventWindow(older: JournalEvent[], current: JournalEvent[]): { events: JournalEvent[]; replaced: boolean } {
  const merged = [...older, ...current];
  if (merged.length > 500) return { events: older, replaced: true };
  let bytes = 0;
  const encoder = new TextEncoder();
  for (const event of merged) {
    bytes += encoder.encode(JSON.stringify(event)).byteLength;
    if (bytes > MAX_EVENT_WINDOW_BYTES) return { events: older, replaced: true };
  }
  return { events: merged, replaced: false };
}
