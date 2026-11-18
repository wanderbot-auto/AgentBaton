import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { StrictMode } from 'react';
import { beforeEach, expect, it, vi } from 'vitest';
import { DaemonError } from './api';
import { Hosts, useHostRegistrationSession } from './Hosts';
import { hostsApi } from './hosts-api';
import { hostMessages } from './hosts-i18n';
import { HostWireError, type HostIntent, type HostPage, type HostReceipt, type HostRecord, type HostRegistration } from './hosts-types';

vi.mock('./hosts-api', () => ({ hostsApi: {
  register: vi.fn(), list: vi.fn(), get: vi.fn(), pending: vi.fn(), reconcile: vi.fn(), retry: vi.fn(), acknowledge: vi.fn(),
} }));
const api = vi.mocked(hostsApi);
const en = hostMessages.en;
const key = '735f6fa9-7067-476d-b605-c6a85cd768b8';
const entry = '14d2e12e-bb40-4baf-86b1-95ffe94e4f88';
const id = 'c92a9c20-fb94-48dc-8e34-7038bfdb2cbd';
const otherId = 'eb752aa0-990a-4c23-ac67-4e5fb7431ec0';
const created = '2024-02-29T00:00:00Z';
const metadata: HostRegistration = {
  schema_version: 1, name: 'Synthetic Host', declared_platform: 'macos',
  endpoint: { expected_host_identity: 'protocol-id', address: 'host.example.invalid', user: 'baton', port: 2222, host_key_alias: 'alias' },
  known_hosts: { reference: 'known:one', file_sha256: 'a'.repeat(64) },
  transport_identity_ref: 'identity:one',
  configuration: { reference: 'config:one', declared_ownership: 'external_read_only' },
  directory_mappings: [],
};
const pending = (registration = metadata, requestKey = key): HostIntent => ({
  intent_version: 1, entry_id: entry, request_key: requestKey, registration,
  registration_sha256: 'b'.repeat(64), created_at: created,
});
const receipt = (registration = metadata): HostReceipt => ({
  receipt_version: 1, host_id: id, revision: 1, created_sequence: 8, created_at: created, registration,
});
const record = (name: string, hostId = id): HostRecord => ({
  record_version: 1, id: hostId, revision: 1, created_sequence: hostId === id ? 8 : 9,
  created_at: created, creation_key: hostId === id ? key : otherId, registration: { ...metadata, name },
  creation_facts: {
    local_validation: 'configuration_syntax_validated', references: 'unresolved',
    remote_probe: 'not_probed', remote_connection: 'not_contacted', remote_capabilities: 'unknown', execution_enabled: false,
  },
});

function Harness({ connected = true, language = 'en' as 'en' | 'zh' }) {
  const session = useHostRegistrationSession();
  return <Hosts language={language} daemonConnected={connected} session={session} />;
}

function open(title: string = en.title) {
  const disclosure = screen.getByText(title, { selector: 'summary' }).closest('details')!;
  disclosure.open = true;
  fireEvent(disclosure, new Event('toggle'));
}

function fill() {
  const values = [
    [en.name, metadata.name], [en.platform, metadata.declared_platform],
    [en.protocol, metadata.endpoint.expected_host_identity], [en.address, metadata.endpoint.address],
    [en.user, metadata.endpoint.user], [en.port, metadata.endpoint.port], [en.alias, metadata.endpoint.host_key_alias],
    [en.known, metadata.known_hosts.reference], [en.checksum, metadata.known_hosts.file_sha256],
    [en.identity, metadata.transport_identity_ref], [en.config, metadata.configuration.reference],
    [en.ownership, metadata.configuration.declared_ownership],
  ];
  for (const [label, value] of values) fireEvent.change(screen.getByLabelText(String(label)), { target: { value } });
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}

beforeEach(() => {
  vi.resetAllMocks();
  api.pending.mockResolvedValue(null);
  api.list.mockResolvedValue({ hosts: [], next_created_sequence: null });
});

it('loads once when expanded, supports keyboard submit and displays a receipt before acknowledgement', async () => {
  const user = userEvent.setup();
  api.register.mockImplementation(async (registration, requestKey) => {
    api.pending.mockResolvedValue(pending(registration, requestKey));
    return { entry_id: entry, receipt: receipt(registration) };
  });
  api.acknowledge.mockResolvedValue(true);
  render(<StrictMode><Harness /></StrictMode>);
  expect(api.pending).not.toHaveBeenCalled();
  open();
  const submit = await screen.findByRole('button', { name: en.register });
  await waitFor(() => expect(submit).toBeEnabled());
  expect(api.pending).toHaveBeenCalledTimes(1);
  expect(api.list).toHaveBeenCalledTimes(1);
  expect(screen.getByLabelText(en.checksum)).toHaveAccessibleDescription(en.checksumHint);
  fill();
  fireEvent.click(screen.getByRole('button', { name: en.add }));
  fireEvent.change(screen.getByLabelText(`${en.mappingId} 1`), { target: { value: 'source' } });
  fireEvent.change(screen.getByLabelText(`${en.root} 1`), { target: { value: '/remote/source' } });
  submit.focus();
  await user.keyboard('{Enter}');
  const ack = await screen.findByRole('button', { name: en.acknowledge });
  expect(api.register).toHaveBeenCalledTimes(1);
  expect(api.register.mock.calls[0][0]).toEqual({ ...metadata, directory_mappings: [{ id: 'source', remote_root: '/remote/source' }] });
  expect(api.acknowledge).not.toHaveBeenCalled();
  expect(submit).toBeDisabled();
  expect(within(screen.getByRole('region', { name: en.registered })).getByText(id)).toBeVisible();
  await user.click(ack);
  expect(await screen.findByText(en.acknowledged)).toBeVisible();
  expect(api.acknowledge).toHaveBeenCalledWith(entry, expect.objectContaining({ created_sequence: 8, host_id: id }));
  expect(screen.getByRole('button', { name: en.register })).toBeEnabled();
});

it('holds the original across offline reads, enables only explicit retry, and requires recheck after false acknowledgement', async () => {
  api.pending.mockResolvedValue(pending());
  api.reconcile.mockResolvedValueOnce(null).mockResolvedValue(receipt());
  api.retry.mockResolvedValue({ entry_id: entry, receipt: receipt() });
  api.acknowledge.mockResolvedValueOnce(false).mockResolvedValueOnce(true);
  const harness = render(<Harness connected={false} />);
  open();
  expect(await screen.findByRole('region', { name: en.pending })).toHaveTextContent(key);
  expect(screen.getByText(en.disconnected)).toBeVisible();
  expect(api.reconcile).not.toHaveBeenCalled();
  expect(api.list).not.toHaveBeenCalled();
  expect(screen.getByRole('button', { name: en.register })).toBeDisabled();

  harness.rerender(<Harness connected />);
  expect(await screen.findByText(en.unresolved)).toBeVisible();
  expect(api.retry).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: en.retry }));
  fireEvent.click(await screen.findByRole('button', { name: en.acknowledge }));
  expect(await screen.findByText(en.recheck)).toBeVisible();
  expect(screen.getByRole('region', { name: en.registered })).toHaveTextContent(id);
  expect(screen.getByRole('button', { name: en.register })).toBeDisabled();
  expect(screen.queryByRole('button', { name: en.retry })).toBeNull();
  fireEvent.click(screen.getByRole('button', { name: en.check }));
  fireEvent.click(await screen.findByRole('button', { name: en.acknowledge }));
  expect(await screen.findByText(en.acknowledged)).toBeVisible();
  expect(api.retry.mock.calls).toEqual([[pending()]]);
  expect(api.register).not.toHaveBeenCalled();
});

it('preserves a known receipt and its pending capture when acknowledgement transport fails', async () => {
  api.pending.mockResolvedValue(pending());
  api.reconcile.mockResolvedValue(receipt());
  api.acknowledge.mockRejectedValue(new DaemonError('PROVIDER_DISCONNECTED', 'lost', 'unknown'));
  render(<Harness />);
  open();
  fireEvent.click(await screen.findByRole('button', { name: en.acknowledge }));
  expect(await screen.findByText(en.issues.disconnected)).toBeVisible();
  expect(screen.getByRole('region', { name: en.registered })).toHaveTextContent(id);
  expect(screen.getByRole('region', { name: en.pending })).toHaveTextContent(key);
  expect(screen.getByRole('button', { name: en.register })).toBeDisabled();
});

it.each([
  [new DaemonError('VERSION_UNSUPPORTED', 'old API/schema', 'not_started'), en.issues.unsupported],
  [new DaemonError('AUTH_REQUIRED', 'auth', 'not_started'), en.issues.auth],
  [new HostWireError('conflict'), en.issues.conflict],
])('keeps pending data and refuses both mutations after a failed lookup', async (error, message) => {
  api.pending.mockResolvedValue(pending());
  api.reconcile.mockRejectedValue(error);
  render(<Harness />);
  open();
  expect(await screen.findByText(message)).toBeVisible();
  expect(screen.getByRole('region', { name: en.pending })).toHaveTextContent(key);
  expect(screen.getByRole('button', { name: en.register })).toBeDisabled();
  expect(screen.queryByRole('button', { name: en.retry })).toBeNull();
  expect(screen.queryByRole('button', { name: en.acknowledge })).toBeNull();
  expect(api.register).not.toHaveBeenCalled();
});

it('ignores obsolete pending and list data across disconnect/reconnect and keeps the last good list on error', async () => {
  const lateList = deferred<HostPage>();
  const latePending = deferred<HostIntent | null>();
  api.list.mockReturnValueOnce(lateList.promise)
    .mockResolvedValueOnce({ hosts: [record('Current')], next_created_sequence: null })
    .mockRejectedValueOnce(new DaemonError('VERSION_UNSUPPORTED', 'old', 'not_started'));
  api.pending.mockReturnValueOnce(latePending.promise);
  const harness = render(<Harness />);
  open();
  await waitFor(() => expect(api.list).toHaveBeenCalledTimes(1));
  harness.rerender(<Harness connected={false} />);
  await waitFor(() => expect(api.pending).toHaveBeenCalledTimes(2));
  harness.rerender(<Harness connected />);
  expect(await screen.findByRole('heading', { name: 'Current' })).toBeVisible();
  await act(async () => {
    lateList.resolve({ hosts: [record('Obsolete')], next_created_sequence: null });
    latePending.resolve(pending());
  });
  expect(screen.queryByRole('heading', { name: 'Obsolete' })).toBeNull();
  expect(screen.queryByRole('region', { name: en.pending })).toBeNull();
  expect(api.reconcile).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: en.refresh }));
  expect(await screen.findByText(en.stale)).toBeVisible();
  expect(screen.getByText(new RegExp(en.issues.unsupported))).toBeVisible();
  expect(screen.getByRole('heading', { name: 'Current' })).toBeVisible();
});

it('reads paginated records and cannot show an obsolete inspection on a different Host', async () => {
  const slow = deferred<HostRecord>();
  api.list.mockResolvedValueOnce({ hosts: [record('First')], next_created_sequence: 8 })
    .mockResolvedValueOnce({ hosts: [record('Second', otherId)], next_created_sequence: null });
  api.get.mockReturnValueOnce(slow.promise).mockResolvedValueOnce(record('Second inspected', otherId));
  render(<Harness />);
  open();
  fireEvent.click(await screen.findByRole('button', { name: en.more }));
  await screen.findByRole('heading', { name: 'Second' });
  expect(api.list.mock.calls).toEqual([[null], [8]]);
  fireEvent.click(screen.getByRole('button', { name: /View registration · First/ }));
  fireEvent.click(screen.getByRole('button', { name: /View registration · Second/ }));
  expect(await screen.findByText('Second inspected')).toBeVisible();
  await act(async () => slow.resolve(record('Obsolete details')));
  expect(screen.queryByText('Obsolete details')).toBeNull();
  expect(screen.getByText('Second inspected')).toBeVisible();
  expect(api.get.mock.calls).toEqual([[id], [otherId]]);
});

it('bounds the bilingual mapping form and rejects invalid UTF-8 byte length before submission', async () => {
  const harness = render(<Harness language="zh" />);
  const zh = hostMessages.zh;
  open(zh.title);
  const add = await screen.findByRole('button', { name: zh.add });
  await waitFor(() => expect(add).toBeEnabled());
  for (let i = 0; i < 8; ++i) fireEvent.click(add);
  expect(add).toBeDisabled();
  expect(screen.getByLabelText(`${zh.root} 8`)).toBeVisible();
  fireEvent.click(screen.getByRole('button', { name: `${zh.remove} 1` }));
  expect(add).toBeEnabled();
  for (let i = 0; i < 7; ++i) fireEvent.click(screen.getByRole('button', { name: `${zh.remove} 1` }));

  harness.rerender(<Harness />);
  fill();
  fireEvent.change(screen.getByLabelText(en.name), { target: { value: '主'.repeat(86) } });
  fireEvent.click(screen.getByRole('button', { name: en.register }));
  expect(await screen.findByText(en.issues.badInput)).toBeVisible();
  expect(api.register).not.toHaveBeenCalled();
});

it('retains the original after a lost submission response and a temporarily empty journal', async () => {
  api.register.mockRejectedValue(new Error('native reply lost'));
  render(<Harness />);
  open();
  const submit = await screen.findByRole('button', { name: en.register });
  await waitFor(() => expect(submit).toBeEnabled());
  fill();
  fireEvent.click(submit);
  expect(await screen.findByText(en.issues.unknown)).toBeVisible();
  const [registration, originalKey] = api.register.mock.calls[0];
  expect(screen.getByRole('region', { name: en.pending })).toHaveTextContent(originalKey);
  expect(submit).toBeDisabled();
  expect(api.reconcile).not.toHaveBeenCalled();

  const disclosure = screen.getByText(en.title, { selector: 'summary' }).closest('details')!;
  disclosure.open = false;
  fireEvent(disclosure, new Event('toggle'));
  expect(screen.queryByRole('region', { name: en.pending })).toBeNull();
  open();
  expect(await screen.findByText(en.issues.unknown)).toBeVisible();
  expect(screen.getByRole('button', { name: en.register })).toBeDisabled();

  api.pending.mockResolvedValue(pending(registration, originalKey));
  api.reconcile.mockResolvedValue(receipt(registration));
  fireEvent.click(screen.getByRole('button', { name: en.check }));
  expect(await screen.findByRole('button', { name: en.acknowledge })).toBeVisible();
  expect(api.reconcile.mock.calls).toEqual([[pending(registration, originalKey)]]);
  expect(api.retry).not.toHaveBeenCalled();
  expect(api.register).toHaveBeenCalledTimes(1);
});

it('allows a new attempt only after an explicit pre-dispatch refusal and a successful empty journal read', async () => {
  api.register.mockRejectedValue(new DaemonError('INVALID_REQUEST', 'invalid', 'not_started'));
  render(<Harness />);
  open();
  const submit = await screen.findByRole('button', { name: en.register });
  await waitFor(() => expect(submit).toBeEnabled());
  fill();
  fireEvent.click(submit);
  expect(await screen.findByText(en.issues.badInput)).toBeVisible();
  expect(submit).toBeEnabled();
  expect(api.pending).toHaveBeenCalledTimes(2);
  expect(api.register).toHaveBeenCalledTimes(1);
  expect(api.reconcile).not.toHaveBeenCalled();
});
