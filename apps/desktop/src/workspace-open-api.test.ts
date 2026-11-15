import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { api } from './api';
import { workspaceOpenApi } from './workspace-open-api';
import { parseWorkspaceOpenContext, parseWorkspaceOpenResult, validateWorkspaceOpenRequest, type WorkspaceOpenSelection } from './workspace-open-wire';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('./api', () => ({ api: { projectsList: vi.fn(), workspacesList: vi.fn() } }));
const projectId = '11111111-1111-4111-8111-111111111111', workspaceId = '22222222-2222-4222-8222-222222222222';
const selection: WorkspaceOpenSelection = { kind: 'project_source', project_id: projectId, expected_project_revision: 3 };
const request = { selection, terminal: 'macos_terminal' };
const project = { id: projectId, revision: 3, archived: false, identity: { canonical_root: '/source dirty', kind: { kind: 'git', dirty: true } } };
const managed = { id: workspaceId, project_id: projectId, revision: 4, root: '/managed/数据', state: 'ready', identity: { canonical_root: '/managed/数据', kind: { kind: 'directory' } } };
const context = () => ({ selection, canonical_path: '/space 目录/new\nline/-leaf', directory_identity: Array.from({ length: 4 }, (_, n) => ({ device: '9007199254740993', inode: String(10 + n) })) });
beforeEach(() => { vi.resetAllMocks(); vi.mocked(invoke).mockResolvedValue({ status: 'handoff_accepted', code: 'accepted' }); });
describe('Directory-only exact wire and native bridge / no real OS handoff', () => {
  it('sends only IDs, revisions and fixed enum; receives direct closed native result', async () => {
    expect(await workspaceOpenApi.open(request)).toEqual({ status: 'handoff_accepted', code: 'accepted' });
    expect(invoke).toHaveBeenCalledExactlyOnceWith('open_workspace_terminal', { request });
  });
  it('supports explicit managed selection without supplying any filesystem pin', async () => {
    const selected = { kind: 'managed_workspace', project_id: projectId, expected_project_revision: 3, workspace_id: workspaceId, expected_workspace_revision: 4 };
    await workspaceOpenApi.open({ selection: selected, terminal: 'macos_terminal' });
    expect(invoke).toHaveBeenCalledExactlyOnceWith('open_workspace_terminal', { request: { selection: selected, terminal: 'macos_terminal' } });
  });
  it.each(['path', 'program', 'args', 'env', 'connection', 'grant', 'approval', 'session'])('refuses renderer field %s before transport', async field => {
    await expect(workspaceOpenApi.open({ ...request, [field]: 'private' })).rejects.toThrow('contract');
    await expect(workspaceOpenApi.open({ ...request, selection: { ...selection, [field]: 'private' } })).rejects.toThrow('contract');
    expect(invoke).not.toHaveBeenCalled();
  });
  it.each([0, -1, 1.5, true, '3', Number.MAX_SAFE_INTEGER + 1, Infinity, NaN])('rejects unsafe revision %s before transport', async revision => {
    await expect(workspaceOpenApi.open({ ...request, selection: { ...selection, expected_project_revision: revision } })).rejects.toThrow(); expect(invoke).not.toHaveBeenCalled();
  });
  it('rejects noncanonical IDs/terminals and unknown union kinds', () => {
    for (const value of [{ ...request, terminal: 'ghostty' }, { ...request, terminal: 'macos_terminal\n' }, { ...request, selection: { ...selection, project_id: projectId + '\n' } }, { ...request, selection: { ...selection, kind: 'remote' } }, { ...request, selection: { ...selection, project_id: 'A' + projectId.slice(1) } }]) expect(() => validateWorkspaceOpenRequest(value)).toThrow();
  });
  it('retains opaque u64 and safely accepts absolute special-character directory text', () => {
    expect(parseWorkspaceOpenContext(context(), selection)).toEqual(context());
    const root = { selection, canonical_path: '/', directory_identity: [{ device: '18446744073709551615', inode: '0' }] };
    expect(parseWorkspaceOpenContext(root, selection)).toEqual(root);
  });
  it('rejects malformed/extra/foreign context, identity count and decimal encodings', () => {
    for (const value of [{ ...context(), path: '/private' }, { ...context(), selection: { ...selection, expected_project_revision: 4 } }, { ...context(), canonical_path: 'relative' }, { ...context(), canonical_path: '/a/../b' }, { ...context(), canonical_path: '/a//b' }, { ...context(), canonical_path: '/a/' }, { ...context(), canonical_path: '/a\0b' }, { ...context(), directory_identity: [] }]) expect(() => parseWorkspaceOpenContext(value, selection)).toThrow();
    for (const bad of [1, true, '01', '-1', '1.0', '1\n', '1\r', '1\u2028', '18446744073709551616']) {
      const value = context(); value.directory_identity[0].device = bad as string; expect(() => parseWorkspaceOpenContext(value, selection)).toThrow();
    }
  });
  it('preserves refused versus unknown and rejects success/error contradictions or private output', async () => {
    for (const value of [{ status: 'refused', code: 'unsupported_platform' }, { status: 'outcome_unknown', code: 'opener_timeout' }, { status: 'outcome_unknown', code: 'opener_failed' }]) expect(parseWorkspaceOpenResult(value)).toEqual(value);
    for (const value of [{ status: 'refused', code: 'opener_timeout' }, { status: 'handoff_accepted', code: 'opener_failed' }, { status: 'handoff_accepted', code: 'accepted', message: 'private' }, { status: 'outcome_unknown', code: 'accepted' }, null]) { vi.mocked(invoke).mockResolvedValue(value); await expect(workspaceOpenApi.open(request)).rejects.toThrow(); }
  });
  it('does not retry an uncertain native transport', async () => {
    vi.mocked(invoke).mockRejectedValue(new Error('lost')); await expect(workspaceOpenApi.open(request)).rejects.toThrow('lost'); expect(invoke).toHaveBeenCalledTimes(1);
  });
  it('lists dirty source and ready Git/data records without adopting any workspace', async () => {
    vi.mocked(api.projectsList).mockResolvedValue([project] as never); vi.mocked(api.workspacesList).mockResolvedValue([managed, { ...managed, id: '33333333-3333-4333-8333-333333333333', state: 'creating', identity: null }, { ...managed, id: '44444444-4444-4444-8444-444444444444', state: 'outcome_unknown', identity: null }] as never);
    const choices = await workspaceOpenApi.choices(projectId); expect(choices).toHaveLength(2); expect(choices.map(v => v.label)).toEqual(['source', 'isolated']);
    expect(choices[1].selection).toEqual({ kind: 'managed_workspace', project_id: projectId, expected_project_revision: 3, workspace_id: workspaceId, expected_workspace_revision: 4 }); expect(invoke).not.toHaveBeenCalled();
  });
  it('refuses foreign/duplicate/not-identity-ready records and archived/stale project metadata', async () => {
    vi.mocked(api.projectsList).mockResolvedValue([project] as never);
    for (const rows of [[managed, managed], [{ ...managed, project_id: workspaceId }], [{ ...managed, identity: null }], [{ ...managed, revision: 0 }], [{ ...managed, state: 'stopped' }]]) { vi.mocked(api.workspacesList).mockResolvedValue(rows as never); await expect(workspaceOpenApi.choices(projectId)).rejects.toThrow(); }
    for (const projects of [[{ ...project, archived: true }], [project, project], [{ ...project, revision: Number.MAX_SAFE_INTEGER + 1 }], []]) { vi.mocked(api.projectsList).mockResolvedValue(projects as never); await expect(workspaceOpenApi.choices(projectId)).rejects.toThrow(); }
    expect(invoke).not.toHaveBeenCalled();
  });
});
