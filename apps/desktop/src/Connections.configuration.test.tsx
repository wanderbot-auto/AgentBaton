import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { Connections } from './Connections';
import { sessionApi } from './session-api';
import { DaemonError } from './api';
import { sessionMessages } from './session-i18n';
vi.mock('./session-api', () => ({ sessionApi: { connectionsList: vi.fn(), connectionsRegister: vi.fn() } }));
beforeEach(() => { vi.resetAllMocks(); vi.mocked(sessionApi.connectionsList).mockResolvedValue([]); });
async function selectedImport() {
  const user = userEvent.setup();
  render(<Connections language="en" connected />);
  await user.type(screen.getByLabelText('Connection name'), 'restricted');
  await user.type(screen.getByLabelText('Canonical executable path'), '/generated/adapter');
  await user.selectOptions(screen.getByLabelText('Protocol route'), 'claude_code');
  await user.selectOptions(screen.getByLabelText('Configuration source'), 'claude_restrictions');
  await user.type(screen.getByLabelText('Canonical Claude settings file'), '/selected/settings.json');
  return user;
}
it('offers only the supported explicit source and sends its file through native registration', async () => {
  vi.mocked(sessionApi.connectionsRegister).mockRejectedValue(new DaemonError('PERMISSION_DENIED', 'Cancelled', 'not_started'));
  const user = await selectedImport();
  expect(screen.getByText(/Only disabledMcpjsonServers is imported/)).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Register and run version probe' }));
  await waitFor(() => expect(sessionApi.connectionsRegister).toHaveBeenCalledWith('restricted', '/generated/adapter', [], expect.any(String), 'claude_code', '/selected/settings.json'));
  await user.selectOptions(screen.getByLabelText('Protocol route'), 'codex');
  expect(screen.queryByLabelText('Canonical Claude settings file')).not.toBeInTheDocument();
  expect(screen.queryByRole('option', { name: 'Import Claude MCP restrictions' })).not.toBeInTheDocument();
  expect(screen.getByLabelText('Configuration source')).toHaveValue('default');
});
it('keeps the original import key after an uncertain commit followed by native cancellation', async () => {
  vi.mocked(sessionApi.connectionsRegister)
    .mockRejectedValueOnce(new DaemonError('OUTCOME_UNKNOWN', 'Unknown response', 'unknown'))
    .mockRejectedValueOnce(new DaemonError('PERMISSION_DENIED', 'Cancelled review', 'not_started'))
    .mockRejectedValueOnce(new DaemonError('OUTCOME_UNKNOWN', 'Still unknown', 'unknown'));
  const user = await selectedImport();
  const button = screen.getByRole('button', { name: 'Register and run version probe' });
  for (const text of ['Unknown response', 'Cancelled review', 'Still unknown']) {
    await user.click(button); await screen.findByText(new RegExp(text));
    expect(screen.getByRole('alert')).toHaveTextContent(sessionMessages.en.uncertain);
  }
  const calls = vi.mocked(sessionApi.connectionsRegister).mock.calls;
  expect(calls).toHaveLength(3); expect(calls[1]).toEqual(calls[0]); expect(calls[2]).toEqual(calls[0]);
});
it('renews review after definite source drift without a prior uncertain commit', async () => {
  vi.mocked(sessionApi.connectionsRegister)
    .mockRejectedValueOnce(new DaemonError('INPUT_CHANGED', 'Source changed', 'not_started'))
    .mockRejectedValueOnce(new DaemonError('PERMISSION_DENIED', 'Cancelled review', 'not_started'));
  const user = await selectedImport();
  const button = screen.getByRole('button', { name: 'Register and run version probe' });
  await user.click(button); await screen.findByText('Source changed');
  await user.click(button); await screen.findByText('Cancelled review');
  const calls = vi.mocked(sessionApi.connectionsRegister).mock.calls;
  expect(calls).toHaveLength(2); expect(calls[1][3]).not.toEqual(calls[0][3]); expect(calls[1][5]).toEqual(calls[0][5]);
});
it('does not send an empty selected source', async () => {
  const user = await selectedImport();
  await user.clear(screen.getByLabelText('Canonical Claude settings file'));
  expect(screen.getByRole('button', { name: 'Register and run version probe' })).toBeDisabled();
  expect(sessionApi.connectionsRegister).not.toHaveBeenCalled();
});

it('selects only Codex top-level policy fields through the same native review and replay workflow', async () => {
  vi.mocked(sessionApi.connectionsRegister)
    .mockRejectedValueOnce(new DaemonError('OUTCOME_UNKNOWN', 'Unknown policy registration', 'unknown'))
    .mockRejectedValueOnce(new DaemonError('PERMISSION_DENIED', 'Cancelled policy review', 'not_started'));
  const user = userEvent.setup();
  render(<Connections language="en" connected />);
  await user.type(screen.getByLabelText('Connection name'), 'policy');
  await user.type(screen.getByLabelText('Canonical executable path'), '/generated/codex');
  await user.selectOptions(screen.getByLabelText('Protocol route'), 'codex');
  await user.selectOptions(screen.getByLabelText('Configuration source'), 'codex_policy');
  expect(screen.getByText(/all other settings are not imported/)).toBeVisible();
  await user.type(screen.getByLabelText('Canonical Codex TOML file'), '/selected/config.toml');
  const button = screen.getByRole('button', { name: 'Register and run version probe' });
  await user.click(button); await screen.findByText(/Unknown policy registration/);
  await user.click(button); await screen.findByText(/Cancelled policy review/);
  const calls = vi.mocked(sessionApi.connectionsRegister).mock.calls;
  expect(calls).toHaveLength(2); expect(calls[1]).toEqual(calls[0]);
  expect(calls[0]).toEqual(['policy', '/generated/codex', [], expect.any(String), 'codex', '/selected/config.toml']);
  expect(screen.getByRole('alert')).toHaveTextContent(sessionMessages.en.uncertain);
  await user.selectOptions(screen.getByLabelText('Protocol route'), 'custom_acp');
  expect(screen.queryByLabelText('Canonical Codex TOML file')).not.toBeInTheDocument();
  expect(screen.queryByRole('option', { name: 'Import Codex execution policy' })).not.toBeInTheDocument();
});
