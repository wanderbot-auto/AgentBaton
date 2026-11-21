import { invoke } from '@tauri-apps/api/core';
import { beforeEach, expect, it, vi } from 'vitest';
import { DaemonError } from './api';
import { hostsApi } from './hosts-api';
import { parseHostRegistration, type HostIntent } from './hosts-types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);
const entry = '14d2e12e-bb40-4baf-86b1-95ffe94e4f88';
const key = '735f6fa9-7067-476d-b605-c6a85cd768b8';
const id = 'c92a9c20-fb94-48dc-8e34-7038bfdb2cbd';
const created = '2024-02-29T00:00:00Z';
const metadata = parseHostRegistration({
  schema_version: 1, name: 'Host', declared_platform: 'macos',
  endpoint: { expected_host_identity: 'protocol', address: 'host.example.invalid', user: 'baton', port: 22, host_key_alias: 'alias' },
  known_hosts: { reference: 'known:one', file_sha256: 'a'.repeat(64) }, transport_identity_ref: 'identity:one',
  configuration: { reference: 'config:one', declared_ownership: 'external_read_only' }, directory_mappings: [],
});
const intent: HostIntent = {
  intent_version: 1, entry_id: entry, request_key: key, registration: metadata,
  registration_sha256: 'b'.repeat(64), created_at: created,
};
const receipt = { receipt_version: 1, host_id: id, revision: 1, created_sequence: 7, created_at: created, registration: metadata } as const;
const submission = { submission_version: 1, entry_id: entry, receipt };
const view = {
  view_version: 1, registration: 'registered',
  record: {
    record_version: 1, id, revision: 1, created_sequence: 7, created_at: created, creation_key: key, registration: metadata,
    creation_facts: { local_validation: 'configuration_syntax_validated', references: 'unresolved',
      remote_probe: 'not_probed', remote_connection: 'not_contacted', remote_capabilities: 'unknown', execution_enabled: false },
  },
};
const ok = (result: unknown) => ({ request_id: id, revision: 99, warnings: [], result });

beforeEach(() => call.mockReset());

it('sends only the seven fixed native commands with the actual closed argument spelling', async () => {
  call.mockResolvedValueOnce(ok(submission))
    .mockResolvedValueOnce(ok({ view_version: 1, hosts: [view], next_created_sequence: null }))
    .mockResolvedValueOnce(ok(view))
    .mockResolvedValueOnce(ok({ pending_version: 1, intent }))
    .mockResolvedValueOnce(ok({ recovery_version: 1, entry_id: entry, lookup: { lookup_version: 1, key, receipt: null } }))
    .mockResolvedValueOnce(ok(submission))
    .mockResolvedValueOnce(ok({ acknowledgement_version: 1, entry_id: entry, acknowledged: false }));

  expect((await hostsApi.register(metadata, key)).receipt).toEqual(receipt);
  expect((await hostsApi.list(1, 2)).hosts[0].id).toBe(id);
  expect((await hostsApi.get(id)).created_sequence).toBe(7);
  expect(await hostsApi.pending()).toEqual(intent);
  expect(await hostsApi.reconcile(intent)).toBeNull();
  expect((await hostsApi.retry(intent)).receipt).toEqual(receipt);
  expect(await hostsApi.acknowledge(entry, receipt)).toBe(false);
  expect(call.mock.calls).toEqual([
    ['hosts_register', { registration: metadata, idempotencyKey: key }],
    ['hosts_list', { request: { after_created_sequence: 1, limit: 2 } }],
    ['hosts_get', { hostId: id }],
    ['hosts_registration_pending', {}],
    ['hosts_registration_reconcile', { entryId: entry }],
    ['hosts_registration_retry', { entryId: entry }],
    ['hosts_registration_acknowledge', { entryId: entry, hostId: id, createdSequence: 7 }],
  ]);
});

it.each(['AUTH_REQUIRED', 'VERSION_UNSUPPORTED', 'CAPABILITY_UNSUPPORTED', 'OUTCOME_UNKNOWN'])(
  'keeps structured %s failures from the existing request helper', async code => {
    const expected = { code, message: 'Local failure', side_effect_state: 'unknown', retryable: false };
    call.mockResolvedValue({ request_id: id, revision: 19, warnings: [], error: expected });
    await expect(hostsApi.reconcile(intent)).rejects.toMatchObject(new DaemonError(code, expected.message, 'unknown', false));
    expect(call).toHaveBeenCalledTimes(1);
  },
);

it('refuses mismatched replies and invalid arguments without dispatching additional requests', async () => {
  call.mockResolvedValueOnce(ok({ ...submission, entry_id: id }));
  await expect(hostsApi.retry(intent)).rejects.toMatchObject({ kind: 'conflict' });
  call.mockResolvedValueOnce(ok({ ...view, record: { ...view.record, id: key } }));
  await expect(hostsApi.get(id)).rejects.toMatchObject({ kind: 'conflict' });
  await expect(hostsApi.register({ ...metadata, name: '主'.repeat(86) }, key)).rejects.toMatchObject({ kind: 'invalid' });
  await expect(hostsApi.list(Number.MAX_SAFE_INTEGER + 1)).rejects.toMatchObject({ kind: 'invalid' });
  await expect(hostsApi.acknowledge(entry, { ...receipt, created_sequence: 9007199254740992 })).rejects.toMatchObject({ kind: 'invalid' });
  expect(call).toHaveBeenCalledTimes(2);
});
