import type { JournalEvent } from './api';

export const MAX_EVENT_WINDOW_BYTES = 8 * 1024 * 1024;
export const MAX_EVENT_WINDOW_RECORDS = 500;
export type WindowReplacement = 'bytes' | 'count' | null;
interface EventWindow { events: JournalEvent[]; replaced: boolean; reason: WindowReplacement }

// This limits displayed UTF-8 JSON, not the persistent journal. A bounded older
// page replaces the current view when combining them would exceed the window.
export function earlierEventWindow(older: JournalEvent[], current: JournalEvent[]): EventWindow {
  const encoder = new TextEncoder();
  let bytes = 2; // JSON array brackets.
  for (const [index, event] of older.entries()) bytes += encoder.encode(JSON.stringify(event)).byteLength + (index > 0 ? 1 : 0);
  if (bytes > MAX_EVENT_WINDOW_BYTES || older.length > MAX_EVENT_WINDOW_RECORDS) {
    throw new Error('The event page exceeds the display budget. The local journal was not changed.');
  }
  if (older.length + current.length > MAX_EVENT_WINDOW_RECORDS) {
    return { events: older, replaced: true, reason: 'count' };
  }
  for (const [index, event] of current.entries()) {
    bytes += encoder.encode(JSON.stringify(event)).byteLength + (older.length + index > 0 ? 1 : 0);
    if (bytes > MAX_EVENT_WINDOW_BYTES) return { events: older, replaced: true, reason: 'bytes' };
  }
  return { events: [...older, ...current], replaced: false, reason: null };
}
