export interface HostRegistration {
  schema_version: 1;
  name: string;
  declared_platform: 'linux' | 'macos';
  endpoint: {
    expected_host_identity: string;
    address: string;
    user: string;
    port: number;
    host_key_alias: string;
  };
  known_hosts: { reference: string; file_sha256: string };
  transport_identity_ref: string;
  configuration: { reference: string; declared_ownership: 'baton_managed' | 'external_read_only' };
  directory_mappings: { id: string; remote_root: string }[];
}

export interface HostIntent {
  intent_version: 1;
  entry_id: string;
  request_key: string;
  registration: HostRegistration;
  registration_sha256: string;
  created_at: string;
}

export interface HostReceipt {
  receipt_version: 1;
  host_id: string;
  revision: 1;
  created_sequence: number;
  created_at: string;
  registration: HostRegistration;
}

export interface HostSubmission {
  entry_id: string;
  receipt: HostReceipt;
}

const creationFacts = {
  local_validation: 'configuration_syntax_validated',
  references: 'unresolved',
  remote_probe: 'not_probed',
  remote_connection: 'not_contacted',
  remote_capabilities: 'unknown',
  execution_enabled: false,
} as const;

export interface HostRecord {
  record_version: 1;
  id: string;
  revision: 1;
  created_sequence: number;
  created_at: string;
  creation_key: string;
  registration: HostRegistration;
  creation_facts: typeof creationFacts;
}

export interface HostPage {
  hosts: HostRecord[];
  next_created_sequence: number | null;
}

export class HostWireError extends Error {
  constructor(readonly kind: 'invalid' | 'unsupported' | 'conflict') {
    super(`Host data: ${kind}`);
  }
}

function invalid(): never {
  throw new HostWireError('invalid');
}

function conflict(): never {
  throw new HostWireError('conflict');
}

function fields(value: unknown, ...names: string[]): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) invalid();
  const record = value as Record<string, unknown>;
  if (Object.keys(record).length !== names.length || names.some(name => !Object.hasOwn(record, name))) invalid();
  return record;
}

function one(value: unknown): 1 {
  if (value !== 1) throw new HostWireError('unsupported');
  return 1;
}

function text(value: unknown, pattern: RegExp): string {
  if (typeof value !== 'string' || !pattern.test(value)) invalid();
  return value;
}

function choice<T extends string>(value: unknown, a: T, b: T): T {
  if (value !== a && value !== b) invalid();
  return value as T;
}

const encoder = new TextEncoder();
const uuidPattern = /^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/;
const registryUuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const identifier = /^[A-Za-z0-9_-]{1,64}$/;
const referencePattern = /^[A-Za-z0-9._:-]{1,256}$/;
const sha256Pattern = /^[0-9a-f]{64}$/;

export const hostUuid = (value: unknown) => text(value, uuidPattern);
export const hostId = (value: unknown) => text(value, registryUuidPattern);
export const maxHostMappings = 8;
export const maxHostPage = 25;

export function positiveHostInteger(value: unknown, maximum = Number.MAX_SAFE_INTEGER): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 1 || value > maximum) invalid();
  return value;
}

function utc(value: unknown, fractional: boolean): string {
  const date = text(value, fractional
    ? /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d{1,9})?Z$/
    : /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$/);
  const seconds = date.slice(0, 19);
  const parsed = new Date(`${seconds}Z`);
  if (date.startsWith('0000-') || !Number.isFinite(parsed.getTime()) || parsed.toISOString().slice(0, 19) !== seconds) invalid();
  return date;
}

function reference(value: unknown): string {
  const ref = text(value, referencePattern);
  if (ref === '.' || ref === '..') invalid();
  return ref;
}

export function parseHostRegistration(value: unknown): HostRegistration {
  const r = fields(value, 'schema_version', 'name', 'declared_platform', 'endpoint', 'known_hosts',
    'transport_identity_ref', 'configuration', 'directory_mappings');
  one(r.schema_version);
  const name = text(r.name, /^[^\p{Cc}\p{Cs}]+$/u);
  if (/^\p{White_Space}|\p{White_Space}$/u.test(name) || encoder.encode(name).length > 256) invalid();
  const endpoint = fields(r.endpoint, 'expected_host_identity', 'address', 'user', 'port', 'host_key_alias');
  const expectedHostIdentity = text(endpoint.expected_host_identity, identifier);
  if (expectedHostIdentity.toLowerCase() === 'local') invalid();
  const address = text(endpoint.address, /^(?!-)[A-Za-z0-9.-]{1,253}$/);
  const known = fields(r.known_hosts, 'reference', 'file_sha256');
  const config = fields(r.configuration, 'reference', 'declared_ownership');
  if (!Array.isArray(r.directory_mappings) || r.directory_mappings.length > maxHostMappings) invalid();
  const seen = new Set<string>();
  const directoryMappings = r.directory_mappings.map((value: unknown) => {
    const mapping = fields(value, 'id', 'remote_root');
    const id = text(mapping.id, identifier);
    const path = text(mapping.remote_root, /^\/[A-Za-z0-9/_.-]{1,511}$/);
    if (seen.has(id) || path.endsWith('/') || path.includes('//') || path.split('/').some(part => part === '.' || part === '..')) invalid();
    seen.add(id);
    return { id, remote_root: path };
  });
  const registration: HostRegistration = {
    schema_version: 1,
    name,
    declared_platform: choice(r.declared_platform, 'linux', 'macos'),
    endpoint: {
      expected_host_identity: expectedHostIdentity,
      address,
      user: text(endpoint.user, identifier),
      port: positiveHostInteger(endpoint.port, 65_535),
      host_key_alias: text(endpoint.host_key_alias, identifier),
    },
    known_hosts: { reference: reference(known.reference), file_sha256: text(known.file_sha256, sha256Pattern) },
    transport_identity_ref: reference(r.transport_identity_ref),
    configuration: {
      reference: reference(config.reference),
      declared_ownership: choice(config.declared_ownership, 'baton_managed', 'external_read_only'),
    },
    directory_mappings: directoryMappings,
  };
  if (encoder.encode(JSON.stringify(registration)).length > 16 * 1024) invalid();
  return registration;
}

// Parsers reconstruct the fixed key order, so field order on the wire does not affect comparison.
export const sameHostRegistration = (a: HostRegistration, b: HostRegistration) =>
  JSON.stringify(a) === JSON.stringify(b);

export function parseHostPending(value: unknown): HostIntent | null {
  const pending = fields(value, 'pending_version', 'intent');
  one(pending.pending_version);
  if (pending.intent === null) return null;
  const intent = fields(pending.intent, 'intent_version', 'entry_id', 'request_key', 'registration', 'registration_sha256', 'created_at');
  return {
    intent_version: one(intent.intent_version),
    entry_id: hostUuid(intent.entry_id),
    request_key: hostUuid(intent.request_key),
    registration: parseHostRegistration(intent.registration),
    registration_sha256: text(intent.registration_sha256, sha256Pattern),
    created_at: utc(intent.created_at, false),
  };
}

export function parseHostReceipt(value: unknown): HostReceipt {
  const receipt = fields(value, 'receipt_version', 'host_id', 'revision', 'created_sequence', 'created_at', 'registration');
  return {
    receipt_version: one(receipt.receipt_version),
    host_id: hostId(receipt.host_id),
    revision: one(receipt.revision),
    created_sequence: positiveHostInteger(receipt.created_sequence),
    created_at: utc(receipt.created_at, true),
    registration: parseHostRegistration(receipt.registration),
  };
}

export function parseHostSubmission(value: unknown, registration: HostRegistration, expectedEntry?: string): HostSubmission {
  const submission = fields(value, 'submission_version', 'entry_id', 'receipt');
  one(submission.submission_version);
  const entry = hostUuid(submission.entry_id);
  const receipt = parseHostReceipt(submission.receipt);
  if ((expectedEntry !== undefined && entry !== expectedEntry) || !sameHostRegistration(receipt.registration, registration)) conflict();
  return { entry_id: entry, receipt };
}

export function parseHostReconciliation(value: unknown, intent: HostIntent): HostReceipt | null {
  const outcome = fields(value, 'recovery_version', 'entry_id', 'lookup');
  one(outcome.recovery_version);
  const lookup = fields(outcome.lookup, 'lookup_version', 'key', 'receipt');
  one(lookup.lookup_version);
  if (outcome.entry_id !== intent.entry_id || lookup.key !== intent.request_key) conflict();
  if (lookup.receipt === null) return null;
  const receipt = parseHostReceipt(lookup.receipt);
  if (!sameHostRegistration(receipt.registration, intent.registration)) conflict();
  return receipt;
}

export function parseHostAcknowledgement(value: unknown, entryId: string): boolean {
  const ack = fields(value, 'acknowledgement_version', 'entry_id', 'acknowledged');
  one(ack.acknowledgement_version);
  if (ack.entry_id !== entryId) conflict();
  if (typeof ack.acknowledged !== 'boolean') invalid();
  return ack.acknowledged;
}

export function parseHostView(value: unknown, expectedId?: string): HostRecord {
  const view = fields(value, 'view_version', 'record', 'registration');
  one(view.view_version);
  if (view.registration !== 'registered') invalid();
  const record = fields(view.record, 'record_version', 'id', 'revision', 'created_sequence',
    'created_at', 'creation_key', 'registration', 'creation_facts');
  const facts = fields(record.creation_facts, ...Object.keys(creationFacts));
  if (Object.entries(creationFacts).some(([key, value]) => facts[key] !== value)) invalid();
  const id = hostId(record.id);
  if (expectedId !== undefined && id !== expectedId) conflict();
  return {
    record_version: one(record.record_version),
    id,
    revision: one(record.revision),
    created_sequence: positiveHostInteger(record.created_sequence),
    created_at: utc(record.created_at, true),
    creation_key: text(record.creation_key, referencePattern),
    registration: parseHostRegistration(record.registration),
    creation_facts: creationFacts,
  };
}

export function parseHostPage(value: unknown, after: number | null, limit: number): HostPage {
  const page = fields(value, 'view_version', 'hosts', 'next_created_sequence');
  one(page.view_version);
  if (!Array.isArray(page.hosts) || page.hosts.length > limit) invalid();
  let sequence = after ?? 0;
  const ids = new Set<string>();
  const keys = new Set<string>();
  const hosts = page.hosts.map((value: unknown) => {
    const host = parseHostView(value);
    if (host.created_sequence <= sequence || ids.has(host.id) || keys.has(host.creation_key)) invalid();
    sequence = host.created_sequence;
    ids.add(host.id);
    keys.add(host.creation_key);
    return host;
  });
  const cursor = page.next_created_sequence === null ? null : positiveHostInteger(page.next_created_sequence);
  if (cursor !== null && (!hosts.length || cursor !== sequence)) invalid();
  return { hosts, next_created_sequence: cursor };
}
