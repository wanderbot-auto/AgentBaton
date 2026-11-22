import { fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import type { Project } from './api';
import { CapabilitiesPage } from './CapabilitiesPage';
import { managedMcpsApi } from './managed-mcps-api';
import { managedSkillsApi } from './managed-skills-api';
import { managedSkillFixture } from './managed-skills-test-fixtures';
import { sessionApi, type ManagedSession } from './session-api';

vi.mock('./managed-skills-api', async original => ({
  ...await original<typeof import('./managed-skills-api')>(), managedSkillsApi: { list: vi.fn(), preview: vi.fn(), create: vi.fn() },
}));
vi.mock('./managed-mcps-api', async original => ({
  ...await original<typeof import('./managed-mcps-api')>(), managedMcpsApi: { list: vi.fn() },
}));
vi.mock('./session-api', () => ({ sessionApi: { connectionsList: vi.fn() } }));

const date = '2026-11-22T10:00:00Z';
const project: Project = {
  id: 'project', name: 'Release project', identity: { canonical_root: '/fixture', device: 1, inode: 2, kind: { kind: 'directory' } },
  revision: 1, archived: false, created_at: date, updated_at: date,
};

beforeEach(() => {
  vi.resetAllMocks();
  vi.stubGlobal('matchMedia', vi.fn(() => ({ matches: true, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
  vi.mocked(managedMcpsApi.list).mockResolvedValue({ profiles: [], unavailable: [] });
});
afterEach(() => vi.unstubAllGlobals());

it('scopes verified configured rows by recorded sessions and never labels a configuration as loaded', async () => {
  const fixture = managedSkillFixture();
  const unrelated = { ...fixture.connection, id: 'other', name: 'Unrelated Claude', provider: 'claude_code' };
  const session: ManagedSession = {
    id: 'session', task_id: 'task', project_id: project.id, workspace_id: 'workspace',
    connection_id: fixture.connection.id, grant_id: 'grant', state: 'stopped', revision: 1,
    native_session_id: null, created_at: date, updated_at: date, last_error: null, turn_state: null,
  };
  vi.mocked(sessionApi.connectionsList).mockResolvedValue([fixture.connection, unrelated]);
  vi.mocked(managedSkillsApi.list).mockResolvedValue({
    profiles: [{ ...fixture.profile, managed_skill: fixture.preview.skill }], unavailable: [],
  });
  render(<CapabilitiesPage language="en" connected projects={[project]} projectId={project.id}
    sessions={[session]} sessionsFresh refreshScope={vi.fn()} />);
  const row = await screen.findByRole('button', { name: /writing.*Configured/ });
  expect(screen.queryByRole('button', { name: /Unrelated Claude/ })).not.toBeInTheDocument();
  fireEvent.click(row);
  const detail = screen.getByRole('complementary', { name: 'writing' });
  expect(within(detail).getByText(/unknown/i)).toBeVisible();
  expect(row).toHaveAttribute('aria-expanded', 'true');
  fireEvent.keyDown(detail, { key: 'Escape' });
  expect(row).toHaveFocus();
  fireEvent.change(screen.getByRole('combobox', { name: 'Capability scope' }), { target: { value: '' } });
  expect(await screen.findByRole('button', { name: /Unrelated Claude/ })).toBeVisible();
  expect(managedSkillsApi.preview).not.toHaveBeenCalled();
  expect(managedSkillsApi.create).not.toHaveBeenCalled();
});

it('hides configuration fields until a verified base is available and re-reads when the editor reopens', async () => {
  const fixture = managedSkillFixture();
  vi.mocked(sessionApi.connectionsList).mockResolvedValue([]);
  vi.mocked(managedSkillsApi.list).mockResolvedValue({ profiles: [], unavailable: [] });
  render(<CapabilitiesPage language="en" connected projects={[]} projectId="" sessions={[]} sessionsFresh refreshScope={vi.fn()} />);
  await screen.findByText('0 records');
  fireEvent.click(screen.getByRole('button', { name: 'Skills' }));
  fireEvent.click(screen.getByRole('button', { name: 'Configure Skill' }));
  const panel = await screen.findByRole('region', { name: 'Managed Skills' });
  const register = await within(panel).findByRole('button', { name: 'Register a base connection' });
  expect(within(panel).queryByRole('textbox')).not.toBeInTheDocument();
  fireEvent.click(register);
  expect(screen.getByText('Register and run version probe', { selector: 'summary' })).toBeVisible();
  vi.mocked(sessionApi.connectionsList).mockResolvedValue([fixture.connection]);
  vi.mocked(managedSkillsApi.list).mockResolvedValue({ profiles: [fixture.profile], unavailable: [] });
  fireEvent.click(screen.getByRole('button', { name: 'Configure Skill' }));
  expect(await within(panel).findByRole('option', { name: 'Approved Codex · ready' })).toBeVisible();
  expect(managedSkillsApi.preview).not.toHaveBeenCalled();
});

it('expands a selected capability under its own row in a narrow window', async () => {
  vi.stubGlobal('matchMedia', vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
  const fixture = managedSkillFixture();
  vi.mocked(sessionApi.connectionsList).mockResolvedValue([fixture.connection]);
  vi.mocked(managedSkillsApi.list).mockResolvedValue({
    profiles: [{ ...fixture.profile, managed_skill: fixture.preview.skill }], unavailable: [],
  });
  render(<CapabilitiesPage language="en" connected projects={[]} projectId="" sessions={[]} sessionsFresh refreshScope={vi.fn()} />);
  const row = await screen.findByRole('button', { name: /writing.*Configured/ });
  fireEvent.click(row);
  const details = screen.getByRole('complementary', { name: 'writing' });
  expect(row.nextElementSibling).toBe(details);
  expect(row).toHaveAttribute('aria-controls', details.id);
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(within(details).getByRole('heading', { name: 'writing' })).toHaveFocus();
  fireEvent.click(within(details).getByRole('button', { name: 'Close capability details' }));
  expect(screen.queryByRole('complementary', { name: 'writing' })).not.toBeInTheDocument();
  expect(row).toHaveFocus();
});
