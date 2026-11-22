import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { App } from './App';
import { api } from './api';
import { backgroundServiceApi } from './background-service-api';
import { hostMessages } from './hosts-i18n';
import { recoveryCenterApi } from './recovery-center-api';
import { sessionApi } from './session-api';
import type { HostRegistration } from './hosts-types';

const native = vi.hoisted(() => vi.fn<(command: string, args?: Record<string, unknown>) => Promise<unknown>>());
vi.mock('@tauri-apps/api/core', () => ({ invoke: native }));
vi.mock('./api', async original => ({
  ...await original<typeof import('./api')>(),
  api: { snapshot: vi.fn(), events: vi.fn(), projectsList: vi.fn() },
}));
vi.mock('./session-api', () => ({ sessionApi: { connectionsList: vi.fn(), sessionsList: vi.fn() } }));
vi.mock('./recovery-center-api', async original => ({
  ...await original<typeof import('./recovery-center-api')>(),
  recoveryCenterApi: { list: vi.fn(), get: vi.fn(), abandon: vi.fn() },
}));
vi.mock('./background-service-api', async original => ({
  ...await original<typeof import('./background-service-api')>(),
  backgroundServiceApi: { status: vi.fn(), hide: vi.fn() },
}));

const en = hostMessages.en;
const entry = '14d2e12e-bb40-4baf-86b1-95ffe94e4f88';
const id = 'c92a9c20-fb94-48dc-8e34-7038bfdb2cbd';
const time = '2024-02-29T00:00:00Z';
const result = (value: unknown) => ({ request_id: null, revision: 99, warnings: [], result: value });

beforeEach(() => {
  vi.resetAllMocks();
  localStorage.clear();
  localStorage.setItem('agentbaton.language', 'en');
  vi.mocked(api.snapshot).mockResolvedValue({ tasks: [], sequence: 1, api_version: 1, database_schema_version: 19 });
  vi.mocked(api.events).mockResolvedValue({ events: [], next_cursor: 1, sequence: 1, gap: false });
  vi.mocked(api.projectsList).mockResolvedValue([]);
  vi.mocked(sessionApi.sessionsList).mockResolvedValue([]);
  vi.mocked(sessionApi.connectionsList).mockResolvedValue([]);
  vi.mocked(backgroundServiceApi.status).mockRejectedValue(new Error('unreadable'));
  vi.mocked(recoveryCenterApi.list).mockResolvedValue([]);
});

function showHosts() {
  fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
  const dialog = screen.getByRole('region', { name: 'Settings' });
  fireEvent.click(within(dialog).getByRole('tab', { name: en.title }));
  const summary = within(dialog).getByText(en.register, { selector: 'summary' });
  fireEvent.click(summary);
  return dialog;
}

function fill() {
  const entries = [
    [en.name, 'Synthetic Host'], [en.platform, 'macos'], [en.protocol, 'protocol'],
    [en.address, 'host.example.invalid'], [en.user, 'owner'], [en.port, '22'], [en.alias, 'host'],
    [en.known, 'known:fixture'], [en.checksum, 'a'.repeat(64)], [en.identity, 'identity:fixture'],
    [en.config, 'config:fixture'], [en.ownership, 'external_read_only'],
  ];
  for (const [label, value] of entries) fireEvent.change(screen.getByLabelText(label), { target: { value } });
}

it('retains the original in-flight Host request when Settings closes before reservation', async () => {
  let registration: HostRegistration | undefined;
  let key: string | undefined;
  let reservationVisible = false;
  let finish!: (reply: unknown) => void;
  const unsettled = new Promise<unknown>(resolve => { finish = resolve; });
  const receipt = () => ({ receipt_version: 1, host_id: id, revision: 1, created_sequence: 7, created_at: time, registration });
  native.mockImplementation(async (command, args) => {
    switch (command) {
      case 'hosts_list': return result({ view_version: 1, hosts: [], next_created_sequence: null });
      case 'hosts_registration_pending': return result({ pending_version: 1, intent: reservationVisible ? {
        intent_version: 1, entry_id: entry, request_key: key, registration, registration_sha256: 'a'.repeat(64), created_at: time,
      } : null });
      case 'hosts_register':
        registration = args?.registration as HostRegistration;
        key = args?.idempotencyKey as string;
        return unsettled;
      case 'hosts_registration_reconcile':
        expect(args).toEqual({ entryId: entry });
        return result({ recovery_version: 1, entry_id: entry, lookup: { lookup_version: 1, key, receipt: receipt() } });
      case 'hosts_registration_acknowledge':
        expect(args).toEqual({ entryId: entry, hostId: id, createdSequence: 7 });
        reservationVisible = false;
        return result({ acknowledgement_version: 1, entry_id: entry, acknowledged: true });
      default: throw new Error(`Unexpected invoke ${command}`);
    }
  });

  const user = userEvent.setup();
  render(<App />);
  expect(native).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: 'Settings' }));
  const dialog = screen.getByRole('region', { name: 'Settings' });
  await user.tab();
  expect(within(dialog).getByRole('tab', { name: 'General' })).toHaveFocus();
  await user.keyboard('{ArrowRight}');
  expect(within(dialog).getByRole('tab', { name: 'Background and login' })).toHaveFocus();
  await user.keyboard('{ArrowRight}');
  expect(within(dialog).getByRole('tab', { name: en.title })).toHaveFocus();
  expect(within(dialog).getByRole('tabpanel', { name: en.title })).toBeVisible();
  expect(within(dialog).queryByRole('region', { name: 'Recover connection creations' })).toBeNull();
  expect(within(dialog).getByRole('tab', { name: 'Background and login' })).toBeVisible();
  expect(within(dialog).queryByRole('combobox', { name: 'Language' })).not.toBeInTheDocument();
  const register = within(dialog).getByText(en.register, { selector: 'summary' });
  expect(register.closest('details')).not.toHaveAttribute('open');
  await user.click(register);
  const submit = await screen.findByRole('button', { name: en.register });
  await waitFor(() => expect(submit).toBeEnabled());
  fill();
  fireEvent.click(submit);
  await waitFor(() => expect(key).toEqual(expect.stringMatching(/^[0-9a-f-]{36}$/)));
  const original = { registration, idempotencyKey: key };

  await user.keyboard('{Escape}');
  expect(screen.queryByRole('region', { name: 'Settings' })).toBeNull();
  showHosts();
  await waitFor(() => expect(native.mock.calls.filter(([command]) => command === 'hosts_registration_pending')).toHaveLength(2));
  const reopenedSubmit = screen.getByRole('button', { name: en.register });
  expect(reopenedSubmit).toBeDisabled();
  expect(screen.getByText(en.working)).toBeVisible();
  fireEvent.click(reopenedSubmit);
  expect(native.mock.calls.filter(([command]) => command === 'hosts_register')).toEqual([['hosts_register', original]]);
  expect(native.mock.calls.some(([command]) => command === 'hosts_registration_reconcile')).toBe(false);

  await act(async () => {
    reservationVisible = true;
    finish(result({ submission_version: 1, entry_id: entry, receipt: receipt() }));
  });
  const ack = await screen.findByRole('button', { name: en.acknowledge });
  expect(screen.getByRole('region', { name: en.pending })).toHaveTextContent(key!);
  expect(screen.getByRole('region', { name: en.registered })).toHaveTextContent(id);
  expect(reopenedSubmit).toBeDisabled();
  fireEvent.click(ack);
  expect(await screen.findByText(en.acknowledged)).toBeVisible();
  expect(screen.getByRole('button', { name: en.register })).toBeEnabled();
  expect(native.mock.calls.filter(([command]) => command === 'hosts_register')).toEqual([['hosts_register', original]]);
  expect(native.mock.calls.some(([command]) => command === 'hosts_registration_retry')).toBe(false);
});
