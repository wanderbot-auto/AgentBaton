import { invoke } from '@tauri-apps/api/core';

export type BackgroundOperation = 'status' | 'enable' | 'disable';
export type BackgroundRegistration = 'disabled' | 'enabled_next_login' | 'conflict' | 'unknown';
export type BackgroundCode = 'ok' | 'cancelled' | 'unsupported_platform' | 'unsupported_installation'
  | 'home_unavailable' | 'home_mismatch' | 'custom_runtime' | 'unsafe_path' | 'ownership_conflict'
  | 'content_conflict' | 'installation_changed' | 'state_changed' | 'busy' | 'native_parent_unavailable'
  | 'io_failed' | 'partial_commit' | 'reconcile_required';
export type BackgroundReceipt = Readonly<{
  api_version: 1; platform: 'macos' | 'unsupported'; mechanism: 'per_user_launchagent' | 'unsupported';
  login_registration: BackgroundRegistration;
  outcome: 'observed' | 'applied' | 'cancelled' | 'refused' | 'partial' | 'unknown' | 'unsupported';
  code: BackgroundCode; side_effect_state: 'not_started' | 'confirmed' | 'unknown';
  current_daemon_action: 'none'; effective_login_start: 'not_observed';
}>;

export class BackgroundServiceError extends Error {
  constructor(readonly reason: 'invalid' | 'unknown' | 'busy' | 'refresh_required') {
    super('Background setting could not be verified');
    this.name = 'BackgroundServiceError';
  }
}
const keys = ['api_version', 'platform', 'mechanism', 'login_registration', 'outcome', 'code',
  'side_effect_state', 'current_daemon_action', 'effective_login_start'] as const;
const refusedCodes = ['home_unavailable', 'home_mismatch', 'custom_runtime', 'unsafe_path',
  'ownership_conflict', 'content_conflict', 'installation_changed', 'state_changed', 'busy',
  'native_parent_unavailable', 'io_failed'] as const;
const oneOf = (value: unknown, values: readonly unknown[]) => values.includes(value);
function requireWire(condition: unknown): asserts condition {
  if (!condition) throw new BackgroundServiceError('invalid');
}

export function parseBackgroundReceipt(value: unknown, operation: BackgroundOperation): BackgroundReceipt {
  requireWire(oneOf(operation, ['status', 'enable', 'disable']));
  requireWire(value !== null && typeof value === 'object' && !Array.isArray(value));
  requireWire(Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null);
  const descriptors = Object.getOwnPropertyDescriptors(value);
  requireWire(Reflect.ownKeys(descriptors).length === keys.length && keys.every(key =>
    Object.hasOwn(descriptors, key) && Object.hasOwn(descriptors[key], 'value') && descriptors[key].enumerable));
  const row = Object.fromEntries(keys.map(key => [key, descriptors[key].value])) as Record<string, unknown>;
  requireWire(row.api_version === 1 && row.current_daemon_action === 'none' && row.effective_login_start === 'not_observed'
    && oneOf(row.login_registration, ['disabled', 'enabled_next_login', 'conflict', 'unknown']));
  if (row.platform === 'unsupported') {
    requireWire(row.mechanism === 'unsupported' && row.login_registration === 'unknown' && row.outcome === 'unsupported'
      && row.code === 'unsupported_platform' && row.side_effect_state === 'not_started');
  } else {
    requireWire(row.platform === 'macos' && row.mechanism === 'per_user_launchagent');
    switch (row.outcome) {
      case 'observed':
        requireWire(row.code === 'ok' && row.side_effect_state === 'not_started'
          && oneOf(row.login_registration, ['disabled', 'enabled_next_login'])
          && (operation === 'status' || row.login_registration === (operation === 'enable' ? 'enabled_next_login' : 'disabled')));
        break;
      case 'applied':
        requireWire(operation !== 'status' && row.code === 'ok' && row.side_effect_state === 'confirmed'
          && row.login_registration === (operation === 'enable' ? 'enabled_next_login' : 'disabled'));
        break;
      case 'cancelled':
        requireWire(operation !== 'status' && row.code === 'cancelled' && row.side_effect_state === 'not_started');
        break;
      case 'refused':
        requireWire(oneOf(row.code, refusedCodes) && row.side_effect_state === 'not_started');
        break;
      case 'partial':
        requireWire(operation !== 'status' && oneOf(row.code, ['partial_commit', 'reconcile_required'])
          && oneOf(row.side_effect_state, ['confirmed', 'unknown']));
        break;
      case 'unknown':
        requireWire(operation !== 'status' && row.code === 'reconcile_required' && row.side_effect_state === 'unknown');
        break;
      case 'unsupported':
        requireWire(row.code === 'unsupported_installation' && row.side_effect_state === 'not_started'
          && row.login_registration === 'unknown');
        break;
      default: requireWire(false);
    }
  }
  return Object.freeze(row) as BackgroundReceipt;
}

export function actionableBackgroundReceipt(value: BackgroundReceipt): boolean {
  return value.platform === 'macos' && value.code === 'ok' && (value.outcome === 'observed' || value.outcome === 'applied')
    && (value.login_registration === 'disabled' || value.login_registration === 'enabled_next_login');
}
const commands = { status: 'background_service_status', enable: 'background_service_enable', disable: 'background_service_disable' } as const;
export const backgroundServiceApi = {
  async call(operation: BackgroundOperation): Promise<BackgroundReceipt> {
    try {
      requireWire(oneOf(operation, ['status', 'enable', 'disable']));
      return parseBackgroundReceipt(await invoke<unknown>(commands[operation]), operation);
    }
    catch (error) {
      if (error instanceof BackgroundServiceError) throw error;
      throw new BackgroundServiceError('unknown');
    }
  },
};

type Snapshot = Readonly<{ pending: BackgroundOperation | null; needsRefresh: boolean; generation: number }>;
type Observation = Readonly<{ receipt: BackgroundReceipt; generation: number }>;
const listeners = new Set<() => void>();
const contexts = new Map<symbol, boolean>();
let snapshot: Snapshot = Object.freeze({ pending: null, needsRefresh: false, generation: 0 });
let pendingOwner: symbol | null = null;
function publish(change: Partial<Snapshot>) {
  snapshot = Object.freeze({ ...snapshot, ...change });
  for (const listener of listeners) listener();
}

// Keep only coordination/uncertainty across settings mounts, never an old receipt.
export const backgroundServiceSession = {
  subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener); }; },
  getSnapshot: () => snapshot,
  open() { const owner = Symbol('background-setting-context'); contexts.set(owner, false); return owner; },
  close(owner: symbol) {
    contexts.delete(owner);
    if (pendingOwner === owner && snapshot.pending !== 'status') publish({ needsRefresh: true });
  },
  async perform(owner: symbol, operation: BackgroundOperation): Promise<Observation> {
    if (!contexts.has(owner)) throw new BackgroundServiceError('refresh_required');
    if (snapshot.pending !== null) throw new BackgroundServiceError('busy');
    if (operation !== 'status' && (snapshot.needsRefresh || !contexts.get(owner))) throw new BackgroundServiceError('refresh_required');
    const generation = snapshot.generation + 1;
    if (operation !== 'status') for (const context of contexts.keys()) contexts.set(context, false);
    contexts.set(owner, false);
    pendingOwner = owner;
    publish({ pending: operation, generation });
    try {
      const receipt = await backgroundServiceApi.call(operation);
      const actionable = actionableBackgroundReceipt(receipt);
      if (contexts.has(owner)) {
        contexts.set(owner, actionable);
        publish({ needsRefresh: !actionable && receipt.outcome !== 'cancelled' });
      } else if (operation !== 'status') publish({ needsRefresh: true });
      return { receipt, generation };
    } catch (error) {
      publish({ needsRefresh: true });
      throw error;
    } finally {
      pendingOwner = null;
      publish({ pending: null });
    }
  },
};
