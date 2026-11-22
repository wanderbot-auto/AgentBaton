import {act,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {beforeEach,expect,it,vi} from 'vitest';
import {App} from './App';
import {api} from './api';
import {sessionApi} from './session-api';
import {managedMcpsApi} from './managed-mcps-api';
import {managedSkillsApi} from './managed-skills-api';
import {backgroundServiceApi} from './background-service-api';
import {invoke} from '@tauri-apps/api/core';
import {BackgroundServiceSettings} from './BackgroundServiceSettings';
import {backgroundServiceSession} from './background-login-api';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
vi.mock('./api',async original=>({...await original<typeof import('./api')>(),api:{snapshot:vi.fn(),events:vi.fn(),projectsList:vi.fn()}}));
vi.mock('./session-api',()=>({sessionApi:{connectionsList:vi.fn(),sessionsList:vi.fn()}}));
vi.mock('./managed-mcps-api',async original=>({...await original<typeof import('./managed-mcps-api')>(),managedMcpsApi:{list:vi.fn(),preview:vi.fn(),create:vi.fn(),find:vi.fn()}}));
vi.mock('./managed-skills-api',async original=>({...await original<typeof import('./managed-skills-api')>(),managedSkillsApi:{list:vi.fn()}}));
const receipt = { api_version: 1, platform: 'macos', mechanism: 'per_user_launchagent',
  login_registration: 'disabled', outcome: 'observed', code: 'ok', side_effect_state: 'not_started',
  current_daemon_action: 'none', effective_login_start: 'not_observed' };

beforeEach(async () => {
  vi.resetAllMocks();
  localStorage.clear();
  localStorage.setItem('agentbaton.language','en');
  const owner = backgroundServiceSession.open();
  vi.mocked(invoke).mockResolvedValue(receipt);
  await backgroundServiceSession.perform(owner, 'status');
  backgroundServiceSession.close(owner);
  vi.mocked(invoke).mockClear();
});
vi.mock('./background-service-api',async original=>({...await original<typeof import('./background-service-api')>(),backgroundServiceApi:{status:vi.fn(),hide:vi.fn()}}));
it('opens background settings without hiding or changing existing settings',async()=>{vi.mocked(api.snapshot).mockResolvedValue({tasks:[],sequence:1,api_version:1,database_schema_version:19});vi.mocked(api.events).mockResolvedValue({events:[],next_cursor:1,sequence:1,gap:false});vi.mocked(api.projectsList).mockResolvedValue([]);vi.mocked(sessionApi.sessionsList).mockResolvedValue([]);vi.mocked(sessionApi.connectionsList).mockResolvedValue([]);vi.mocked(backgroundServiceApi.status).mockResolvedValue({instance_id:'12345678-1234-1234-1234-123456789abc',started_at:'2026-11-09T12:00:00Z',api_version:1,database_schema_version:19,sequence:1,owned_provider_workers:false,store_recovery:'completed_on_start'});render(<App/>);await waitFor(()=>expect(api.snapshot).toHaveBeenCalled());fireEvent.click(screen.getByRole('button',{name:'Settings'}));expect(screen.getByText('Appearance')).toBeVisible();expect(screen.getByRole('tabpanel',{name:'General'})).toContainElement(screen.getByRole('combobox',{name:'Language'}));expect(backgroundServiceApi.status).not.toHaveBeenCalled();fireEvent.click(screen.getByRole('tab',{name:'Background and login'}));expect(await screen.findByRole('region',{name:'Background service'})).toBeVisible();expect(backgroundServiceApi.hide).not.toHaveBeenCalled();});


it('keeps the observed switch value until the native change returns a verified receipt', async () => {
  vi.mocked(invoke).mockResolvedValueOnce(receipt);
  let finish!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
  render(<BackgroundServiceSettings language="en" embedded />);
  const control = await screen.findByRole('switch', { name: 'Start background work at next login' });
  expect(control).toHaveAttribute('aria-checked', 'false');
  control.focus();
  fireEvent.click(control);
  expect(invoke).toHaveBeenLastCalledWith('background_service_enable');
  expect(control).toHaveAttribute('aria-checked', 'false');
  expect(control).toHaveAttribute('aria-disabled', 'true');
  fireEvent.click(control);
  expect(invoke).toHaveBeenCalledTimes(2);
  await act(async () => finish({ ...receipt, login_registration: 'enabled_next_login', outcome: 'applied', side_effect_state: 'confirmed' }));
  expect(control).toHaveAttribute('aria-checked', 'true');
  expect(control).toHaveFocus();
});
