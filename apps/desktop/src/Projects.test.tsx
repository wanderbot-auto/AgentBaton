import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { Projects } from './Projects';
import { api, DaemonError, type Project } from './api';

// Browser component tests use mocked Tauri transport; core IPC and Git tests
// separately validate the real source/managed-directory behavior.
vi.mock('./api', async importOriginal => {
  const actual = await importOriginal<typeof import('./api')>();
  return { ...actual, api: { projectsImport: vi.fn(), projectsArchive: vi.fn(), projectsReassociate: vi.fn(), workspacesCreate: vi.fn(), workspacesList: vi.fn() } };
});
const head = 'a'.repeat(40);
const project: Project = { id: 'project-1', name: 'My repository', identity: { canonical_root: '/source/repository', device: 1, inode: 2, kind: { kind: 'git', common_dir: '/source/repository/.git', head, dirty: true } }, archived: false, revision: 3, created_at: '2026-10-27T00:00:00Z', updated_at: '2026-10-27T00:00:00Z' };
const refresh = vi.fn().mockResolvedValue(undefined);
function show(projects = [project]) {
  const onSelect = vi.fn();
  render(<Projects projects={projects} selectedId={project.id} onSelect={onSelect} refresh={refresh} connected language="en" />);
  return onSelect;
}
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.workspacesList).mockResolvedValue([]);
});
describe('Project management using mocked Tauri transport', () => {
  it('imports only an explicitly entered source and selects its returned identity', async () => {
    const user = userEvent.setup();
    vi.mocked(api.projectsImport).mockResolvedValue(project);
    const selected = show([]);
    await user.type(screen.getByLabelText('Project name'), project.name);
    await user.type(screen.getByLabelText('Project directory'), project.identity.canonical_root);
    await user.click(screen.getByRole('button', { name: 'Import project' }));
    await waitFor(() => expect(api.projectsImport).toHaveBeenCalledWith('/source/repository', 'My repository', expect.any(String)));
    expect(selected).toHaveBeenCalledWith(project.id);
    expect(refresh).toHaveBeenCalled();
    expect(screen.getByLabelText('Project directory')).toHaveValue('');
  });
  it('shows observed dirty identity and creates a separate workspace without a renderer destination', async () => {
    const user = userEvent.setup();
    vi.mocked(api.workspacesCreate).mockResolvedValue({ id: 'workspace-1', project_id: project.id, root: '/managed/workspace-1', identity: null, base_revision: head, state: 'creating', revision: 1, created_at: project.created_at });
    show();
    expect(screen.getByText('Source had local changes when observed')).toBeVisible();
    expect(screen.getByText(head)).toBeVisible();
    await waitFor(() => expect(api.workspacesList).toHaveBeenCalledWith(project.id));
    await user.click(screen.getByRole('button', { name: 'Create isolated workspace' }));
    expect(api.workspacesCreate).toHaveBeenCalledWith(project, head, expect.any(String));
    expect(screen.queryByLabelText(/destination/i)).not.toBeInTheDocument();
  });
  it('reports conflict without changing the displayed project identity', async () => {
    const user = userEvent.setup();
    vi.mocked(api.projectsReassociate).mockRejectedValue(new DaemonError('STALE_REVISION', 'Project revision changed; refresh before relinking', 'not_started'));
    show();
    await user.type(screen.getByLabelText('Replacement project directory'), '/moved/repository');
    await user.click(screen.getByRole('button', { name: 'Reassociate directory' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Project revision changed');
    expect(api.projectsReassociate).toHaveBeenCalledWith(project, '/moved/repository', expect.any(String));
    expect(screen.getByText('/source/repository', { selector: 'dd' })).toBeVisible();
  });
  it('shows unknown workspace outcome and prevents another creation', async () => {
    vi.mocked(api.workspacesList).mockResolvedValue([{ id: 'workspace-1', project_id: project.id, root: '/managed/workspace-1', identity: null, base_revision: head, state: 'outcome_unknown', revision: 1, created_at: project.created_at }]);
    show();
    expect(await screen.findByText('Outcome unknown — reconcile before retrying')).toBeVisible();
    expect(screen.getByRole('button', { name: 'Create isolated workspace' })).toBeDisabled();
    expect(screen.getByText('No verified workspace identity')).toBeVisible();
    expect(screen.getByText('No execution lease is reported. Workspace readiness does not grant permission to dispatch.')).toBeVisible();
  });
  it('archives metadata only and makes archived operations unavailable', async () => {
    const user = userEvent.setup();
    vi.mocked(api.projectsArchive).mockResolvedValue({ ...project, archived: true, revision: 4 });
    const { rerender } = render(<Projects projects={[project]} selectedId={project.id} onSelect={vi.fn()} refresh={refresh} connected language="en" />);
    await user.click(screen.getByRole('button', { name: 'Archive project' }));
    expect(api.projectsArchive).toHaveBeenCalledWith(project, expect.any(String));
    rerender(<Projects projects={[{ ...project, archived: true, revision: 4 }]} selectedId={project.id} onSelect={vi.fn()} refresh={refresh} connected language="en" />);
    expect(screen.getByRole('button', { name: 'Archive project' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Create isolated workspace' })).toBeDisabled();
    expect(screen.getByText('Archiving prevents new work. Original source files and history are retained.')).toBeVisible();
  });
});
