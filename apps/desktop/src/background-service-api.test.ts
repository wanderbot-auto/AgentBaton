// Mocked Native boundary only; this does not exercise macOS service registration.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { actionableBackgroundReceipt, backgroundServiceApi, BackgroundServiceError, backgroundServiceSession as session,
  parseBackgroundReceipt, type BackgroundOperation } from './background-service-api';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const wire = (change: Record<string, unknown> = {}) => ({ api_version: 1, platform: 'macos', mechanism: 'per_user_launchagent',
  login_registration: 'disabled', outcome: 'observed', code: 'ok', side_effect_state: 'not_started',
  current_daemon_action: 'none', effective_login_start: 'not_observed', ...change });
let owner: symbol;
beforeEach(async () => {
  vi.resetAllMocks();
  const reset = session.open();
  vi.mocked(invoke).mockResolvedValue(wire());
  await session.perform(reset, 'status'); session.close(reset);
  vi.mocked(invoke).mockClear();
  owner = session.open();
});
afterEach(() => { session.close(owner); });

async function ready() {
  vi.mocked(invoke).mockResolvedValue(wire());
  await session.perform(owner, 'status'); vi.mocked(invoke).mockClear();
}

describe('Closed background registration wire', () => {
  it.each(['status', 'enable', 'disable'] as const)('uses only the fixed no-argument Native %s command', async operation => {
    vi.mocked(invoke).mockResolvedValue(wire({ login_registration: operation === 'enable' ? 'enabled_next_login' : 'disabled' }));
    const value = await backgroundServiceApi.call(operation);
    expect(invoke).toHaveBeenCalledExactlyOnceWith('background_service_' + operation);
    expect(Object.isFrozen(value)).toBe(true);
    expect(value.effective_login_start).toBe('not_observed');
  });
  it('refuses unknown operations before Native', async () => {
    await expect(backgroundServiceApi.call('launchctl' as BackgroundOperation)).rejects.toBeInstanceOf(BackgroundServiceError);
    expect(invoke).not.toHaveBeenCalled();
    expect(() => parseBackgroundReceipt(wire(), 'launchctl' as BackgroundOperation)).toThrow(BackgroundServiceError);
  });
  it('accepts supported/refused/cancelled/partial/unknown outcomes without granting actionability', () => {
    for (const value of [wire({ outcome: 'cancelled', code: 'cancelled' }),
      wire({ outcome: 'refused', code: 'unsafe_path', login_registration: 'unknown' }),
      wire({ outcome: 'refused', code: 'ownership_conflict', login_registration: 'conflict' }),
      wire({ outcome: 'partial', code: 'partial_commit', side_effect_state: 'confirmed' }),
      wire({ outcome: 'partial', code: 'reconcile_required', side_effect_state: 'unknown' }),
      wire({ outcome: 'unknown', code: 'reconcile_required', side_effect_state: 'unknown' }),
      wire({ outcome: 'unsupported', code: 'unsupported_installation', login_registration: 'unknown' }),
      wire({ platform: 'unsupported', mechanism: 'unsupported', outcome: 'unsupported', code: 'unsupported_platform', login_registration: 'unknown' })]) {
      expect(actionableBackgroundReceipt(parseBackgroundReceipt(value, 'enable'))).toBe(false);
    }
  });
  it('allows only the requested applied or no-change write result', () => {
    for (const operation of ['enable', 'disable'] as const) {
      const registration = operation === 'enable' ? 'enabled_next_login' : 'disabled';
      expect(actionableBackgroundReceipt(parseBackgroundReceipt(wire({ outcome: 'applied', side_effect_state: 'confirmed', login_registration: registration }), operation))).toBe(true);
      expect(actionableBackgroundReceipt(parseBackgroundReceipt(wire({ login_registration: registration }), operation))).toBe(true);
      const wrong = registration === 'disabled' ? 'enabled_next_login' : 'disabled';
      expect(() => parseBackgroundReceipt(wire({ login_registration: wrong }), operation)).toThrow(BackgroundServiceError);
      expect(() => parseBackgroundReceipt(wire({ outcome: 'applied', side_effect_state: 'confirmed', login_registration: wrong }), operation)).toThrow(BackgroundServiceError);
    }
  });
  it('rejects extra fields, coercions, unknown values, contradictory scope and invented effective startup', () => {
    const values: unknown[] = [null, [], new Date(), { ...wire(), api_version: true }, { ...wire(), api_version: 2 },
      wire({ path: '/PRIVATE/CREDENTIALS' }), wire({ platform: 'linux' }), wire({ mechanism: 'launchctl' }),
      wire({ login_registration: 'running' }), wire({ outcome: 'success' }), wire({ code: 'RAW_ERROR_SECRET' }),
      wire({ side_effect_state: 'not_started', outcome: 'applied' }), wire({ side_effect_state: 'confirmed' }),
      wire({ current_daemon_action: 'started' }), wire({ effective_login_start: 'running' }),
      wire({ outcome: 'observed', code: 'cancelled' }), wire({ login_registration: 'conflict' }),
      wire({ outcome: 'cancelled', code: 'ok' }), wire({ outcome: 'refused', code: 'ok' }),
      wire({ outcome: 'unknown', code: 'io_failed', side_effect_state: 'unknown' }),
      wire({ outcome: 'partial', code: 'partial_commit' }), wire({ outcome: 'unsupported', code: 'unsupported_platform', login_registration: 'unknown' }),
      wire({ platform: 'unsupported', mechanism: 'unsupported', outcome: 'unsupported', code: 'unsupported_platform', login_registration: 'disabled' })];
    const missing = wire() as Record<string, unknown>; delete missing.code; values.push(missing);
    for (const value of values) expect(() => parseBackgroundReceipt(value, 'status')).toThrow(BackgroundServiceError);
    for (const outcome of ['applied', 'cancelled', 'partial', 'unknown']) expect(() => parseBackgroundReceipt(wire({ outcome }), 'status')).toThrow(BackgroundServiceError);
  });
  it('does not invoke accessors or accept hidden extra keys/inherited fields', () => {
    const getter = vi.fn(() => 'PRIVATE'); const accessor = wire(); Object.defineProperty(accessor, 'code', { get: getter });
    expect(() => parseBackgroundReceipt(accessor, 'status')).toThrow(BackgroundServiceError); expect(getter).not.toHaveBeenCalled();
    const symbol = { ...wire(), [Symbol('secret')]: 'PRIVATE' }; expect(() => parseBackgroundReceipt(symbol, 'status')).toThrow(BackgroundServiceError);
    expect(() => parseBackgroundReceipt(Object.create(wire()), 'status')).toThrow(BackgroundServiceError);
  });
  it('sanitizes transport exceptions', async () => {
    vi.mocked(invoke).mockRejectedValue(new Error('/PRIVATE/ACCOUNT/API_KEY'));
    await expect(backgroundServiceApi.call('status')).rejects.toMatchObject({ reason: 'unknown', message: 'Background setting could not be verified' });
  });
});

describe('Single-flight and lost response retention across UI contexts', () => {
  it('requires a fresh read in each new context before a write', async () => {
    await expect(session.perform(owner, 'enable')).rejects.toMatchObject({ reason: 'refresh_required' });
    expect(invoke).not.toHaveBeenCalled();
    await ready();
    const other = session.open();
    try { await expect(session.perform(other, 'enable')).rejects.toMatchObject({ reason: 'refresh_required' }); }
    finally { session.close(other); }
    expect(invoke).not.toHaveBeenCalled();
  });
  it('blocks concurrent read/write and never retries a timed-out transport', async () => {
    await ready();
    let reject!: (reason?: unknown) => void;
    vi.mocked(invoke).mockImplementation(() => new Promise((_, no) => { reject = no; }));
    const operation = session.perform(owner, 'enable');
    await expect(session.perform(owner, 'enable')).rejects.toMatchObject({ reason: 'busy' });
    await expect(session.perform(owner, 'status')).rejects.toMatchObject({ reason: 'busy' });
    expect(invoke).toHaveBeenCalledTimes(1);
    reject(new Error('PRIVATE_RESPONSE_UNKNOWN')); await expect(operation).rejects.toMatchObject({ reason: 'unknown' });
    expect(session.getSnapshot().needsRefresh).toBe(true);
    await expect(session.perform(owner, 'enable')).rejects.toMatchObject({ reason: 'refresh_required' });
    expect(invoke).toHaveBeenCalledTimes(1);
    vi.mocked(invoke).mockResolvedValue(wire());
    await session.perform(owner, 'status');
    expect(session.getSnapshot().needsRefresh).toBe(false);
  });
  it('does not let a successful old-context reply clear uncertainty or authorize its replacement', async () => {
    await ready(); let finish!: (value: unknown) => void;
    vi.mocked(invoke).mockImplementation(() => new Promise(resolve => { finish = resolve; }));
    const old = session.perform(owner, 'enable'); session.close(owner);
    const other = session.open();
    try {
      expect(session.getSnapshot().needsRefresh).toBe(true);
      await expect(session.perform(other, 'status')).rejects.toMatchObject({ reason: 'busy' });
      finish(wire({ outcome: 'applied', side_effect_state: 'confirmed', login_registration: 'enabled_next_login' }));
      await old;
      expect(session.getSnapshot().needsRefresh).toBe(true);
      await expect(session.perform(other, 'disable')).rejects.toMatchObject({ reason: 'refresh_required' });
      expect(invoke).toHaveBeenCalledTimes(1);
      vi.mocked(invoke).mockResolvedValue(wire({ login_registration: 'enabled_next_login' }));
      await session.perform(other, 'status');
      expect(session.getSnapshot().needsRefresh).toBe(false);
    } finally { session.close(other); }
  });
  it('invalidates other contexts after a write begins and after cancellation requires another read', async () => {
    await ready(); const other = session.open();
    try {
      await session.perform(other, 'status'); vi.mocked(invoke).mockResolvedValue(wire({ outcome: 'cancelled', code: 'cancelled' }));
      await session.perform(owner, 'enable'); vi.mocked(invoke).mockClear();
      await expect(session.perform(other, 'enable')).rejects.toMatchObject({ reason: 'refresh_required' });
      await expect(session.perform(owner, 'enable')).rejects.toMatchObject({ reason: 'refresh_required' });
      expect(invoke).not.toHaveBeenCalled();
    } finally { session.close(other); }
  });
});
