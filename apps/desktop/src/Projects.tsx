import { useEffect, useRef, useState } from 'react';
import { api, DaemonError, type ManagedWorkspace, type Project } from './api';
import { messages, type Language } from './i18n';
import { DataWorkspaces } from './DataWorkspaces';
import { dataWorkspaceMessages } from './data-workspace-i18n';

interface ProjectsProps {
  projects: Project[];
  selectedId: string;
  onSelect: (id: string) => void;
  refresh: () => Promise<void>;
  connected: boolean;
  language: Language;
}

export function Projects({ projects, selectedId, onSelect, refresh, connected, language }: ProjectsProps) {
  const t = messages[language];
  const [name, setName] = useState('');
  const [path, setPath] = useState('');
  const [replacement, setReplacement] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [workspaces, setWorkspaces] = useState<ManagedWorkspace[]>([]);
  const [dataVersion,setDataVersion]=useState(0);
  const [workspaceLoad, setWorkspaceLoad] = useState<'loading' | 'loaded' | 'error'>('loading');
  const keys = useRef(new Map<string, string>());
  const workspaceRequest = useRef(0);
  const selected = projects.find(project => project.id === selectedId);
  const git = selected?.identity.kind.kind === 'git' ? selected.identity.kind : null;

  useEffect(() => {
    const request = ++workspaceRequest.current;
    setWorkspaces([]);
    setWorkspaceLoad('loading');
    if (!selected || !connected) return;
    void api.workspacesList(selected.id).then(result => {
      if (request === workspaceRequest.current) { setWorkspaces(result); setWorkspaceLoad('loaded'); }
    }).catch(e => {
      if (request === workspaceRequest.current) { setWorkspaceLoad('error'); setError(e instanceof Error ? e.message : String(e)); }
    });
    return () => { ++workspaceRequest.current; };
  }, [selected?.id, selected?.revision, connected, dataVersion]);
  useEffect(() => { setReplacement(''); }, [selected?.id]);

  async function mutate<T>(identity: string, operation: (key: string) => Promise<T>, success?: (value: T) => void) {
    if (busy || !connected) return;
    const key = keys.current.get(identity) ?? crypto.randomUUID();
    keys.current.set(identity, key);
    setBusy(true); setError('');
    try {
      const value = await operation(key);
      keys.current.delete(identity);
      success?.(value);
      await refresh();
    } catch (e) {
      setError((e instanceof Error ? e.message : String(e)) + (e instanceof DaemonError && e.sideEffectState === 'unknown' ? ` ${t.sideEffectUnknown}` : ''));
    } finally { setBusy(false); }
  }
  function importProject(event: React.SubmitEvent<HTMLFormElement>) {
    event.preventDefault();
    const nextName = name.trim(), nextPath = path.trim();
    if (!nextName || !nextPath) return;
    void mutate(JSON.stringify(['import',nextName,nextPath]), key => api.projectsImport(nextPath, nextName, key), project => {
      setName(''); setPath(''); onSelect(project.id);
    });
  }
  return <section className="projects" aria-labelledby="projects-heading">
    <h2 id="projects-heading">{t.projects}</h2>
    {error && <div className="error" role="alert"><strong>{t.error}</strong><p>{error}</p></div>}
    <form className="project-import" onSubmit={importProject}>
      <label htmlFor="project-name">{t.projectName}</label><input id="project-name" disabled={busy} value={name} onChange={event => setName(event.target.value)} maxLength={200} required />
      <label htmlFor="project-path">{t.projectPath}</label><input id="project-path" disabled={busy} value={path} onChange={event => setPath(event.target.value)} maxLength={4096} required placeholder="/absolute/path" />
      <p>{t.importHelp}</p><button disabled={!connected || busy || !name.trim() || !path.trim()}>{busy ? t.working : t.importProject}</button>
    </form>
    <div className="workspace"><section>
      {!projects.length && <p>{t.noProjects}</p>}
      <ul className="task-list">{projects.map(project => <li key={project.id}><button disabled={busy} aria-pressed={selectedId === project.id} onClick={() => onSelect(project.id)}><strong>{project.name}</strong><span>{project.archived ? t.archived : t.active}</span><small>{project.identity.canonical_root}</small></button></li>)}</ul>
    </section>{selected && <section className="inspector">
      <h3>{selected.name}</h3><p>{selected.archived ? t.archived : t.active}</p>
      <dl><dt>{t.root}</dt><dd>{selected.identity.canonical_root}</dd><dt>{t.projectRevision}</dt><dd>{selected.revision}</dd><dt>{t.observedAt}</dt><dd>{selected.updated_at}</dd>{git && <><dt>{t.head}</dt><dd>{git.head}</dd></>}</dl>
      <p>{git ? (git.dirty ? t.dirty : t.clean) : t.directory}</p>
      <p className="hint">{t.identitySnapshot}</p>
      <p className="hint">{t.archiveHelp}</p><button disabled={busy || !connected || selected.archived} onClick={() => void mutate(JSON.stringify(['archive',selected.id]), key => api.projectsArchive(selected, key))}>{t.archiveProject}</button>
      <form className="reassociate" onSubmit={event => {
        event.preventDefault();
        if (replacement.trim()) void mutate(JSON.stringify(['reassociate',selected.id,replacement.trim()]), key => api.projectsReassociate(selected, replacement.trim(), key), () => setReplacement(''));
      }}><label htmlFor="replacement-path">{t.reassociatePath}</label><input id="replacement-path" disabled={busy} value={replacement} onChange={event => setReplacement(event.target.value)} maxLength={4096} required /><p className="hint">{t.reassociateHelp}</p><button disabled={busy || !connected || selected.archived || !replacement.trim()}>{t.reassociate}</button></form>
      <DataWorkspaces key={selected.id} project={selected} connected={connected} language={language} onReady={()=>setDataVersion(value=>value+1)}/>
      <section aria-labelledby="workspaces-heading"><h3 id="workspaces-heading">{t.isolatedWorkspace}</h3><p className="hint">{t.workspaceHelp}</p>
        <button disabled={busy || !connected || workspaceLoad !== 'loaded' || selected.archived || !git?.head || workspaces.some(workspace => workspace.state !== 'ready')} onClick={() => {
          if (git?.head) void mutate(JSON.stringify(['workspace',selected.id,git.head]), key => api.workspacesCreate(selected, git.head, key), workspace => setWorkspaces(current => [...current, workspace]));
        }}>{t.createWorkspace}</button>
        {(selected.archived || !git?.head) && <p className="hint">{t.workspaceUnavailable}</p>}
        {workspaceLoad === 'loading' && <p role="status">{t.loadingWorkspaces}</p>}
        {workspaceLoad === 'loaded' && !workspaces.length && <p>{t.noWorkspaces}</p>}
        <ul className="workspace-list">{workspaces.map(workspace => <li key={workspace.id}><dl><dt>{t.workspaceId}</dt><dd>{workspace.id}</dd><dt>{t.workspaceRoot}</dt><dd>{workspace.root}</dd><dt>{t.workspaceState}</dt><dd>{workspace.state === 'ready' ? t.workspaceReady : workspace.state === 'creating' ? t.workspaceCreating : t.workspaceUnknown}</dd><dt>{dataWorkspaceMessages[language].baseRevision}</dt><dd>{workspace.base_revision}</dd></dl>{!workspace.identity && <p>{t.unknownRoot}</p>}<p className="hint">{t.noLeaseEvidence}</p></li>)}</ul>
      </section>
    </section>}</div>
  </section>;
}
