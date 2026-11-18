import { describe, expect, it } from 'vitest';
import {
  HostWireError, parseHostAcknowledgement, parseHostPage, parseHostPending, parseHostReceipt,
  parseHostReconciliation, parseHostRegistration, parseHostSubmission, parseHostView, sameHostRegistration,
} from './hosts-types';

const entry = '14d2e12e-bb40-4baf-86b1-95ffe94e4f88';
const key = '735f6fa9-7067-476d-b605-c6a85cd768b8';
const id = 'c92a9c20-fb94-48dc-8e34-7038bfdb2cbd';
const time = '2024-02-29T00:00:00Z';
const input = {
  schema_version: 1, name: 'Host', declared_platform: 'linux',
  endpoint: { expected_host_identity: 'protocol-id', address: 'host.example.invalid', user: 'baton', port: 22, host_key_alias: 'alias' },
  known_hosts: { reference: 'provisioned:hosts', file_sha256: 'a'.repeat(64) },
  transport_identity_ref: 'provisioned:identity',
  configuration: { reference: 'provisioned:config', declared_ownership: 'external_read_only' },
  directory_mappings: [{ id: 'source', remote_root: '/remote/source' }],
} as const;
const rawIntent = {
  intent_version: 1, entry_id: entry, request_key: key, registration: input,
  registration_sha256: 'b'.repeat(64), created_at: time,
};
const receipt = { receipt_version: 1, host_id: id, revision: 1, created_sequence: 10, created_at: time, registration: input };
const view = {
  view_version: 1, registration: 'registered',
  record: {
    record_version: 1, id, revision: 1, created_sequence: 10, created_at: time,
    creation_key: key, registration: input,
    creation_facts: {
      local_validation: 'configuration_syntax_validated', references: 'unresolved',
      remote_probe: 'not_probed', remote_connection: 'not_contacted', remote_capabilities: 'unknown',
      execution_enabled: false,
    },
  },
};

describe('Host schema 1 source contract', () => {
  it('normalizes field order without rewriting metadata and supports all eight mappings', () => {
    const maximal = {
      ...input, name: '主'.repeat(85) + 'a',
      endpoint: { ...input.endpoint, port: 65_535, address: 'a'.repeat(253) },
      transport_identity_ref: 'a'.repeat(256),
      directory_mappings: Array.from({ length: 8 }, (_, index) => ({ id: 'a'.repeat(63) + index, remote_root: '/' + 'p'.repeat(511) })),
    };
    const parsed = parseHostRegistration(maximal);
    expect(parsed).toEqual(maximal);
    const reversed = Object.fromEntries(Object.entries(maximal).reverse());
    expect(sameHostRegistration(parsed, parseHostRegistration(reversed))).toBe(true);
    expect(parseHostRegistration({ ...input, directory_mappings: [] }).directory_mappings).toEqual([]);
  });

  it('rejects byte overflow, invalid Unicode, metadata paths, and unsafe endpoint numerics', () => {
    for (const changed of [
      { name: '主'.repeat(86) }, { name: ' untrimmed' }, { name: 'host\u0085' }, { name: 'host\ud800' },
      { endpoint: { ...input.endpoint, address: '-option' } },
      { endpoint: { ...input.endpoint, expected_host_identity: 'LOCAL' } },
      { endpoint: { ...input.endpoint, port: 65_536 } },
      { endpoint: { ...input.endpoint, port: Number.MAX_SAFE_INTEGER + 1 } },
      { endpoint: { ...input.endpoint, port: 2.5 } },
      { endpoint: { ...input.endpoint, port: '22' } },
      { transport_identity_ref: '/secret/config' },
      { known_hosts: { ...input.known_hosts, file_sha256: 'A'.repeat(64) } },
      { directory_mappings: [{ id: 'source', remote_root: '/remote/../escape' }] },
      { directory_mappings: [{ id: 'source', remote_root: '/remote//nested' }] },
      { directory_mappings: [...input.directory_mappings, ...input.directory_mappings] },
      { directory_mappings: Array.from({ length: 9 }, (_, index) => ({ id: String(index), remote_root: '/remote' })) },
    ]) expect(() => parseHostRegistration({ ...input, ...changed })).toThrow(HostWireError);
  });

  it('rejects extra fields, missing null, invalid dates and old or future versions', () => {
    for (const version of [0, 2, null]) {
      expect(() => parseHostRegistration({ ...input, schema_version: version })).toThrow(expect.objectContaining({ kind: 'unsupported' }));
      expect(() => parseHostPending({ pending_version: version, intent: null })).toThrow(HostWireError);
      expect(() => parseHostReceipt({ ...receipt, receipt_version: version })).toThrow(HostWireError);
      expect(() => parseHostView({ ...view, view_version: version })).toThrow(HostWireError);
    }
    expect(parseHostPending({ pending_version: 1, intent: null })).toBeNull();
    expect(() => parseHostPending({ pending_version: 1 })).toThrow(HostWireError);
    expect(() => parseHostPending({ pending_version: 1, intent: { ...rawIntent, created_at: '2025-02-29T00:00:00Z' } })).toThrow(HostWireError);
    expect(() => parseHostReceipt({ ...receipt, created_at: '2024-01-01T24:00:00Z' })).toThrow(HostWireError);
    expect(() => parseHostRegistration({ ...input, endpoint: { ...input.endpoint, flags: '-hidden' } })).toThrow(HostWireError);
    expect(parseHostReceipt({ ...receipt, created_at: '2024-02-29T00:00:00.123456789Z' }).created_at)
      .toBe('2024-02-29T00:00:00.123456789Z');
  });

  it('binds a displayed receipt and acknowledgement to the exact entry and metadata', () => {
    const intent = parseHostPending({ pending_version: 1, intent: rawIntent })!;
    const recovery = { recovery_version: 1, entry_id: entry, lookup: { lookup_version: 1, key, receipt } };
    expect(parseHostReconciliation(recovery, intent)).toEqual(receipt);
    expect(parseHostReconciliation({ ...recovery, lookup: { ...recovery.lookup, receipt: null } }, intent)).toBeNull();
    expect(() => parseHostReconciliation({ ...recovery, lookup: { lookup_version: 1, key } }, intent)).toThrow(HostWireError);
    expect(() => parseHostReconciliation({ ...recovery, lookup: { ...recovery.lookup, key: entry } }, intent)).toThrow(HostWireError);
    expect(() => parseHostReconciliation({ ...recovery, entry_id: id }, intent)).toThrow(HostWireError);
    expect(() => parseHostReconciliation({ ...recovery, lookup: { ...recovery.lookup, receipt: { ...receipt, registration: { ...input, name: 'Different' } } } }, intent)).toThrow(HostWireError);
    expect(() => parseHostSubmission({ submission_version: 1, entry_id: id, receipt }, intent.registration, entry)).toThrow(HostWireError);
    expect(parseHostAcknowledgement({ acknowledgement_version: 1, entry_id: entry, acknowledged: false }, entry)).toBe(false);
    expect(() => parseHostAcknowledgement({ acknowledgement_version: 1, entry_id: id, acknowledged: true }, entry)).toThrow(HostWireError);
  });

  it('accepts only a bounded ordered registry page and safely representable sequence numbers', () => {
    expect(parseHostPage({ view_version: 1, hosts: [view], next_created_sequence: 10 }, 9, 1).hosts[0].created_sequence).toBe(10);
    for (const candidate of [
      { view_version: 1, hosts: [view, view], next_created_sequence: 10 },
      { view_version: 1, hosts: [view], next_created_sequence: 11 },
      { view_version: 1, hosts: [], next_created_sequence: 10 },
      { view_version: 1, hosts: [view], next_created_sequence: Number.MAX_SAFE_INTEGER + 1 },
      { view_version: 1, hosts: [view], next_created_sequence: '10' },
    ]) expect(() => parseHostPage(candidate, 9, 1)).toThrow(HostWireError);
    expect(() => parseHostPage({ view_version: 1, hosts: [view], next_created_sequence: null }, 10, 1)).toThrow(HostWireError);
    expect(() => parseHostReceipt({ ...receipt, created_sequence: Number.MAX_SAFE_INTEGER + 1 })).toThrow(HostWireError);
    expect(() => parseHostView({ ...view, record: { ...view.record, creation_facts: { ...view.record.creation_facts, execution_enabled: true } } })).toThrow(HostWireError);
  });
});
