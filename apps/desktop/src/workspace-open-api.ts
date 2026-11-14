import { invoke } from '@tauri-apps/api/core';
import { api } from './api';
import { parseWorkspaceOpenResult, validateWorkspaceOpenRequest, workspaceOpenId, workspaceOpenPath, workspaceOpenRevision, type WorkspaceOpenSelection } from './workspace-open-wire';
export interface WorkspaceOpenChoice { selection: WorkspaceOpenSelection; label: 'source' | 'isolated'; recordedPath: string }
const invalid = (): never => { throw new Error('Invalid workspace open choices'); };
export const workspaceOpenApi = {
  choices: async (projectId: string): Promise<WorkspaceOpenChoice[]> => {
    if (!workspaceOpenId(projectId)) return invalid();
    const projects = await api.projectsList();
    if (!Array.isArray(projects) || projects.length > 1000 || projects.filter(v => v?.id === projectId).length !== 1) return invalid();
    const project = projects.find(v => v.id === projectId)!;
    if (!workspaceOpenRevision(project.revision) || project.archived !== false || !workspaceOpenPath(project.identity?.canonical_root)) return invalid();
    const selection = { kind: 'project_source' as const, project_id: projectId, expected_project_revision: project.revision };
    const rows = await api.workspacesList(projectId);
    if (!Array.isArray(rows) || rows.length > 1000) return invalid();
    const ids = new Set<string>(), choices: WorkspaceOpenChoice[] = [{ selection, label: 'source', recordedPath: project.identity.canonical_root }];
    for (const row of rows) {
      if (!row || !workspaceOpenId(row.id) || ids.has(row.id) || row.project_id !== projectId || !workspaceOpenRevision(row.revision) || !['ready', 'creating', 'outcome_unknown'].includes(row.state)) return invalid();
      ids.add(row.id);
      if (row.state !== 'ready') continue;
      if (!row.identity || !workspaceOpenPath(row.root) || row.identity.canonical_root !== row.root) return invalid();
      choices.push({ selection: { kind: 'managed_workspace', project_id: projectId, expected_project_revision: project.revision, workspace_id: row.id, expected_workspace_revision: row.revision }, label: 'isolated', recordedPath: row.root });
    }
    return choices;
  },
  open: async (request: unknown) => parseWorkspaceOpenResult(await invoke<unknown>('open_workspace_terminal', { request: validateWorkspaceOpenRequest(request) })),
};
