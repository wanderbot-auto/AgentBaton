// Mocked Native receipts and DOM behavior, not a macOS service or OS-dialog test.
import { StrictMode } from 'react';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { BackgroundServiceSettings } from './BackgroundServiceSettings';
import { backgroundServiceSession } from './background-service-api';
import { backgroundServiceMessages as copy } from './background-service-i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const wire = (change: Record<string, unknown> = {}) => ({ api_version: 1, platform: 'macos', mechanism: 'per_user_launchagent',
  login_registration: 'disabled', outcome: 'observed', code: 'ok', side_effect_state: 'not_started',
  current_daemon_action: 'none', effective_login_start: 'not_observed', ...change });
const applied = (enabled = true) => wire({ outcome: 'applied', side_effect_state: 'confirmed', login_registration: enabled ? 'enabled_next_login' : 'disabled' });
async function open(language: 'en' | 'zh' = 'en') {
  await userEvent.click(screen.getByText(copy[language].title));
  return screen.findByRole('region', { name: copy[language].registration });
}
beforeEach(async () => {
  cleanup(); vi.resetAllMocks();
  const owner = backgroundServiceSession.open(); vi.mocked(invoke).mockResolvedValue(wire());
  await backgroundServiceSession.perform(owner, 'status'); backgroundServiceSession.close(owner);
  vi.mocked(invoke).mockClear();
});
afterEach(() => cleanup());

describe('Quiet optional background settings', () => {
  it('does not read or change registration until the setting is expanded', async () => {
    render(<BackgroundServiceSettings language="en" />);
    expect(screen.queryByRole('region')).toBeNull(); expect(invoke).not.toHaveBeenCalled();
    const panel = await open(); await within(panel).findByText(copy.en.disabled);
    expect(invoke).toHaveBeenCalledExactlyOnceWith('background_service_status');
    expect(within(panel).getByRole('button', { name: copy.en.enable })).toBeEnabled();
    expect(within(panel).getByText(copy.en.boundary)).toBeVisible();
    expect(within(panel).getByText(copy.en.observation)).toBeVisible();
    expect(screen.queryByText(/running|launchctl|per_user_launchagent|not_observed/i)).toBeNull();
  });
  it('performs one read under StrictMode and never enables by default', async () => {
    render(<StrictMode><BackgroundServiceSettings language="en" /></StrictMode>);
    const panel = await open(); await within(panel).findByText(copy.en.disabled);
    expect(invoke).toHaveBeenCalledExactlyOnceWith('background_service_status');
  });
  it('connects explicit enable/disable to distinct Native confirmations without parameters or automatic refresh', async () => {
    render(<BackgroundServiceSettings language="en" />); const panel = await open();
    await within(panel).findByText(copy.en.disabled);
    vi.mocked(invoke).mockResolvedValue(applied());
    await userEvent.click(within(panel).getByRole('button', { name: copy.en.enable }));
    await within(panel).findByText(copy.en.enabled);
    expect(invoke).toHaveBeenLastCalledWith('background_service_enable'); expect(invoke).toHaveBeenCalledTimes(2);
    vi.mocked(invoke).mockResolvedValue(applied(false));
    await userEvent.click(within(panel).getByRole('button', { name: copy.en.disable }));
    await within(panel).findByText(copy.en.disabled);
    expect(invoke).toHaveBeenLastCalledWith('background_service_disable'); expect(invoke).toHaveBeenCalledTimes(3);
  });
  it.each([wire({ platform: 'unsupported', mechanism: 'unsupported', login_registration: 'unknown', outcome: 'unsupported', code: 'unsupported_platform' }),
    wire({ login_registration: 'unknown', outcome: 'unsupported', code: 'unsupported_installation' }),
    wire({ login_registration: 'conflict', outcome: 'refused', code: 'ownership_conflict' }),
    wire({ login_registration: 'disabled', outcome: 'refused', code: 'unsafe_path' }),
  ])('keeps unsupported or unsafe observations non-actionable', async value => {
    vi.mocked(invoke).mockResolvedValue(value);
    render(<BackgroundServiceSettings language="en" />); const panel = await open();
    await within(panel).findByRole('alert');
    expect(within(panel).getByRole('button', { name: copy.en.enable })).toBeDisabled();
    expect(within(panel).queryByText(copy.en.disabled)).toBeNull();
    expect(within(panel).getByRole('alert').textContent).not.toMatch(/unsafe_path|ownership_conflict|unsupported_installation/);
    expect(invoke).toHaveBeenCalledExactlyOnceWith('background_service_status');
  });
  it('handles cancellation without claiming a change or silently repeating it', async () => {
    render(<BackgroundServiceSettings language="en" />); const panel = await open(); await within(panel).findByText(copy.en.disabled);
    vi.mocked(invoke).mockResolvedValue(wire({ outcome: 'cancelled', code: 'cancelled' }));
    await userEvent.click(within(panel).getByRole('button', { name: copy.en.enable }));
    await within(panel).findByText(copy.en.cancelled);
    expect(within(panel).getByRole('button', { name: copy.en.enable })).toBeDisabled();
    expect(within(panel).queryByText(copy.en.enabled)).toBeNull(); expect(invoke).toHaveBeenCalledTimes(2);
  });
  it.each([
    wire({ outcome: 'partial', code: 'partial_commit', side_effect_state: 'confirmed' }),
    wire({ outcome: 'unknown', code: 'reconcile_required', side_effect_state: 'unknown' }),
  ])('requires manual refresh after a partial or unknown write outcome', async result => {
    render(<BackgroundServiceSettings language="en" />); const panel = await open(); await within(panel).findByText(copy.en.disabled);
    vi.mocked(invoke).mockResolvedValue(result);
    await userEvent.click(within(panel).getByRole('button', { name: copy.en.enable }));
    await within(panel).findByText(copy.en.refreshFirst);
    expect(within(panel).getByRole('button', { name: copy.en.enable })).toBeDisabled(); expect(invoke).toHaveBeenCalledTimes(2);
    vi.mocked(invoke).mockResolvedValue(wire({ login_registration: 'enabled_next_login' }));
    await userEvent.click(within(panel).getByRole('button', { name: copy.en.refresh }));
    await within(panel).findByText(copy.en.enabled);
    expect(within(panel).getByRole('button', { name: copy.en.disable })).toBeEnabled();
    expect(invoke).toHaveBeenLastCalledWith('background_service_status'); expect(invoke).toHaveBeenCalledTimes(3);
  });
  it('does not leak transport/invalid response data and does not retry the write', async () => {
    render(<BackgroundServiceSettings language="en" />); const panel = await open(); await within(panel).findByText(copy.en.disabled);
    vi.mocked(invoke).mockRejectedValue(new Error('PRIVATE_HOME_CREDENTIAL_TOKEN'));
    await userEvent.click(within(panel).getByRole('button', { name: copy.en.enable }));
    await within(panel).findByText(copy.en.transport);
    expect(screen.queryByText(/PRIVATE_HOME_CREDENTIAL_TOKEN/)).toBeNull();
    expect(within(panel).getByRole('button', { name: copy.en.enable })).toBeDisabled(); expect(invoke).toHaveBeenCalledTimes(2);
    vi.mocked(invoke).mockResolvedValue(wire({ api_version: 2, raw: 'PRIVATE_HOME_CREDENTIAL_TOKEN' }));
    await userEvent.click(within(panel).getByRole('button', { name: copy.en.refresh }));
    await waitFor(() => expect(within(panel).getByRole('button', { name: copy.en.refresh })).toBeEnabled());
    expect(within(panel).getByText(copy.en.unknown)).toBeVisible(); expect(screen.queryByText(/PRIVATE_HOME_CREDENTIAL_TOKEN/)).toBeNull();
    expect(invoke).toHaveBeenCalledTimes(3);
  });
  it('blocks double-clicks and preserves an unmounted write as unknown when its old success arrives', async () => {
    const first = render(<BackgroundServiceSettings language="en" />); const panel = await open(); await within(panel).findByText(copy.en.disabled);
    let finish!: (value: unknown) => void;
    vi.mocked(invoke).mockImplementation(() => new Promise(resolve => { finish = resolve; }));
    await userEvent.dblClick(within(panel).getByRole('button', { name: copy.en.enable }));
    expect(invoke).toHaveBeenCalledTimes(2);
    first.unmount();
    render(<BackgroundServiceSettings language="en" />); const next = await open();
    expect(within(next).getByRole('button', { name: copy.en.refresh })).toBeDisabled();
    await act(async () => { finish(applied()); });
    await within(next).findByText(copy.en.unknown);
    expect(within(next).queryByText(copy.en.enabled)).toBeNull(); expect(invoke).toHaveBeenCalledTimes(2);
    expect(within(next).getByRole('button', { name: copy.en.enable })).toBeDisabled();
    vi.mocked(invoke).mockResolvedValue(wire({ login_registration: 'enabled_next_login' }));
    await userEvent.click(within(next).getByRole('button', { name: copy.en.refresh }));
    await within(next).findByText(copy.en.enabled); expect(invoke).toHaveBeenCalledTimes(3);
  });
  it('survives closing and reopening after a transport-unknown write without an automatic read', async () => {
    render(<BackgroundServiceSettings language="en" />); let panel = await open(); await within(panel).findByText(copy.en.disabled);
    vi.mocked(invoke).mockRejectedValue(new Error('UNKNOWN_PRIVATE_WRITE'));
    await userEvent.click(within(panel).getByRole('button', { name: copy.en.enable })); await within(panel).findByText(copy.en.transport);
    await userEvent.click(screen.getByText(copy.en.title)); await waitFor(() => expect(screen.queryByRole('region')).toBeNull());
    panel = await open(); expect(invoke).toHaveBeenCalledTimes(2);
    expect(within(panel).getByText(copy.en.unknown)).toBeVisible();
    expect(within(panel).getByRole('button', { name: copy.en.enable })).toBeDisabled();
  });
  it('renders complete Chinese scope and next-login state without technical fields', async () => {
    render(<BackgroundServiceSettings language="zh" />); const panel = await open('zh'); await within(panel).findByText(copy.zh.disabled);
    for (const text of [copy.zh.help, copy.zh.boundary, copy.zh.observation, copy.zh.confirmation]) expect(within(panel).getByText(text)).toBeVisible();
    expect(within(panel).getByRole('button', { name: copy.zh.enable })).toBeEnabled();
    expect(within(panel).queryByText(/api_version|not_observed|launchagent|side_effect_state/)).toBeNull();
  });
});
