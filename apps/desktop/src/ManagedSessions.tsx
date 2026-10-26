import { useCallback, useEffect, useRef, useState } from 'react';
import { api, DaemonError, type JournalEvent, type ManagedWorkspace, type Project, type Task } from './api';
import { sessionApi, type ExecutionGrant, type ManagedSession, type NamedConnection } from './session-api';
import { sessionMessages } from './session-i18n';
import { messages, type Language } from './i18n';

interface Props { language: Language; connected: boolean; projects: Project[]; tasks: Task[]; projectId: string; onProjectChange: (id: string) => void; refreshTasks: () => Promise<void> }
export function ManagedSessions({ language, connected, projects, tasks, projectId, onProjectChange, refreshTasks }: Props) {
  const t = sessionMessages[language];
  const [connections, setConnections] = useState<NamedConnection[]>([]);
  const [sessions, setSessions] = useState<ManagedSession[]>([]);
  const [grants, setGrants] = useState<ExecutionGrant[]>([]);
  const [workspaces, setWorkspaces] = useState<ManagedWorkspace[]>([]);
  const [workspaceId, setWorkspaceId] = useState('');
  const [connectionId, setConnectionId] = useState('');
  const [taskId, setTaskId] = useState('');
  const [grantId, setGrantId] = useState('');
  const [sessionId, setSessionId] = useState('');
  const [attempts, setAttempts] = useState(1);
  const [concurrency, setConcurrency] = useState(1);
  const [duration, setDuration] = useState(60);
  const [acknowledged, setAcknowledged] = useState(false);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(false);
  const [scopeReady, setScopeReady] = useState(false);
  const [error, setError] = useState('');
  const [text, setText] = useState('');
  const [uncertainSend, setUncertainSend] = useState<Set<string>>(() => new Set());
  const [events, setEvents] = useState<JournalEvent[]>([]);
  const [byteLimitedPage, setByteLimitedPage] = useState(false);
  const [sessionFresh, setSessionFresh] = useState(false);
  const [sessionWorkspace, setSessionWorkspace] = useState<ManagedWorkspace | null>(null);
  const operationKeys = useRef(new Map<string, string>());
  const scopeRequest = useRef(0);
  const evidenceRequest = useRef(0);
  const selected = sessions.find(session => session.id === sessionId);
  const project = projects.find(record => record.id === projectId);
  const workspace = workspaces.find(record => record.id === workspaceId);
  const connection = connections.find(record => record.id === connectionId);
  const task = tasks.find(record => record.id === taskId);
  const grant = grants.find(record => record.id === grantId);
  const matchingGrants = grants.filter(record => !record.revoked && record.expires_at_ms > Date.now() && record.scope.workspace_ids.includes(workspaceId) && record.scope.connection_ids.includes(connectionId));
  const grantMatches = !!grant && matchingGrants.some(record => record.id === grant.id);
  const canGrant = connected && scopeReady && !project?.archived && !!project && workspace?.state === 'ready' && connection?.probe_state === 'ready';
  const stateText = (value: string) => value in t ? t[value as keyof typeof t] : `${t.unknown}: ${value}`;

  function showError(e: unknown) {
    setError((e instanceof Error ? e.message : String(e)) + (e instanceof DaemonError && e.sideEffectState === 'unknown' ? ` ${t.uncertain}` : ''));
  }
  const refresh = useCallback(async () => {
    if (!connected) return;
    setLoading(true);
    try {
      const [nextConnections, nextSessions] = await Promise.all([sessionApi.connectionsList(), sessionApi.sessionsList()]);
      setConnections(nextConnections); setSessions(current => mergeSessions(current, nextSessions));
    } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setLoading(false); }
  }, [connected]);
  useEffect(() => { void refresh(); }, [refresh]);
  const refreshScope = useCallback(async () => {
    const request = ++scopeRequest.current;
    setScopeReady(false);
    if (!projectId || !connected) { setWorkspaces([]); setGrants([]); return; }
    try {
      const [nextWorkspaces, nextGrants] = await Promise.all([api.workspacesList(projectId), sessionApi.grantsList(projectId)]);
      if (request !== scopeRequest.current) return;
      setWorkspaces(nextWorkspaces); setGrants(nextGrants); setScopeReady(true);
    } catch (e) { if (request === scopeRequest.current) setError(e instanceof Error ? e.message : String(e)); }
  }, [projectId, connected]);
  useEffect(() => { setWorkspaceId(''); setGrantId(''); setTaskId(''); setAcknowledged(false); }, [projectId]);
  useEffect(() => { void refreshScope(); return () => { ++scopeRequest.current; }; }, [refreshScope]);
  useEffect(() => { setAcknowledged(false); setGrantId(''); }, [workspaceId, connectionId, attempts, concurrency, duration]);

  const refreshEvidence = useCallback(async (id: string) => {
    const request = ++evidenceRequest.current;
    const snapshot = await api.snapshot();
    const batch = await api.events(Math.max(0, snapshot.sequence - 100));
    if (request === evidenceRequest.current) { setEvents(batch.events.filter(event => event.aggregate_id === id)); setByteLimitedPage(!!batch.byte_limited); }
  }, []);
  useEffect(() => {
    ++evidenceRequest.current; setEvents([]); setByteLimitedPage(false); setText(''); setSessionFresh(false);
    if (sessionId && connected) void refreshEvidence(sessionId).catch(e => setError(e instanceof Error ? e.message : String(e)));
    return () => { ++evidenceRequest.current; };
  }, [sessionId, connected, refreshEvidence]);
  useEffect(() => {
    setSessionWorkspace(null);
    if (!selected || !connected) return;
    let disposed = false;
    void api.workspacesList(selected.project_id).then(records => {
      if (!disposed) setSessionWorkspace(records.find(record => record.id === selected.workspace_id) ?? null);
    }).catch(() => { /* The ID remains visible; no path is invented. */ });
    return () => { disposed = true; };
  }, [selected?.id, selected?.project_id, selected?.workspace_id, connected]);
  useEffect(() => {
    if (!selected || !connected || !['starting', 'ready', 'running', 'cancel_requested'].includes(selected.state)) return;
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    async function tick() {
      try {
        const record = await sessionApi.sessionsGet(selected!.id);
        if (disposed) return;
        setSessions(current => mergeSessions(current, [record]));
        setSessionFresh(true);
        await refreshEvidence(record.id);
      } catch (e) { if (!disposed) { setSessionFresh(false); setError(e instanceof Error ? e.message : String(e)); } }
      if (!disposed) timer = setTimeout(() => void tick(), 1000);
    }
    void tick();
    return () => { disposed = true; clearTimeout(timer); };
  }, [selected?.id, selected?.state, connected, refreshEvidence]);

  async function mutate<T>(fingerprint: string, operation: (key: string) => Promise<T>, onSuccess: (result: T) => void) {
    if (!connected || busy) return;
    const key = operationKeys.current.get(fingerprint) ?? crypto.randomUUID();
    operationKeys.current.set(fingerprint, key);
    setBusy(true); setError('');
    try {
      const result = await operation(key);
      operationKeys.current.delete(fingerprint); onSuccess(result);
      await refresh(); await refreshScope(); await refreshTasks();
    } catch (e) { showError(e); }
    finally { setBusy(false); }
  }
  async function send(event: React.SubmitEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selected || !sessionFresh || selected.state !== 'ready' || busy || !connected || !text.trim() || uncertainSend.has(selected.id)) return;
    setBusy(true); setError('');
    const fingerprint = JSON.stringify(['send', selected.id, selected.revision, text]);
    const key = operationKeys.current.get(fingerprint) ?? crypto.randomUUID();
    operationKeys.current.set(fingerprint, key);
    try {
      const record = await sessionApi.sessionsSend(selected, text, key);
      operationKeys.current.delete(fingerprint);
      setText(''); setSessions(current => mergeSessions(current, [record]));
      await refreshEvidence(record.id);
    } catch (e) {
      if (!(e instanceof DaemonError) || e.sideEffectState === 'unknown') setUncertainSend(current => new Set([...current, selected.id]));
      showError(e);
    } finally { setBusy(false); }
  }
  return <section className="managed-sessions" aria-labelledby="sessions-title">
    <div className="section-heading"><h2 id="sessions-title">{t.sessions}</h2><button disabled={!connected || busy || loading} onClick={() => { void refresh(); void refreshScope(); if (sessionId) void refreshEvidence(sessionId).catch(showError); }}>{t.refresh}</button></div>
    {error && <div className="error" role="alert"><strong>{t.error}</strong><p>{error}</p></div>}
    {!connected && <p>{t.unavailable}</p>}
    {loading && <p role="status">{t.loading}</p>}
    {!connections.length && <p>{t.noConnection}</p>}
    <fieldset disabled={busy || !connected} className="session-scope"><legend>{t.grantTitle}</legend>
      <label htmlFor="session-project">{t.project}</label><select id="session-project" value={projectId} onChange={event => onProjectChange(event.target.value)}><option value="">{t.choose}</option>{projects.filter(record => !record.archived).map(record => <option key={record.id} value={record.id}>{record.name}</option>)}</select>
      <label htmlFor="session-task">{t.task}</label><select id="session-task" value={taskId} onChange={event => setTaskId(event.target.value)}><option value="">{t.choose}</option>{tasks.filter(record => record.project_id === projectId).map(record => <option key={record.id} value={record.id} disabled={record.state !== 'draft'}>{record.goal} ({record.state})</option>)}</select>
      <label htmlFor="session-workspace">{t.workspace}</label><select id="session-workspace" value={workspaceId} onChange={event => setWorkspaceId(event.target.value)}><option value="">{t.choose}</option>{workspaces.map(record => <option key={record.id} value={record.id} disabled={record.state !== 'ready'}>{record.root} ({record.state})</option>)}</select>
      <label htmlFor="session-connection">{t.connection}</label><select id="session-connection" value={connectionId} onChange={event => setConnectionId(event.target.value)}><option value="">{t.choose}</option>{connections.map(record => <option key={record.id} value={record.id} disabled={record.probe_state !== 'ready'}>{record.name} ({record.probe_state})</option>)}</select>
      <label htmlFor="grant-attempts">{t.attempts}</label><input id="grant-attempts" type="number" min={1} max={100} value={attempts} onChange={event => setAttempts(Number(event.target.value))} />
      <label htmlFor="grant-concurrency">{t.concurrency}</label><input id="grant-concurrency" type="number" min={1} max={attempts} value={concurrency} onChange={event => setConcurrency(Number(event.target.value))} />
      <label htmlFor="grant-duration">{t.duration}</label><input id="grant-duration" type="number" min={1} max={86400} value={duration} onChange={event => setDuration(Number(event.target.value))} />
    </fieldset>
    <dl><dt>{t.recipient}</dt><dd>{connection ? `${connection.name} · ${connection.provider} · ${connection.identity_ref} (${connection.identity_status})` : t.choose}</dd><dt>{t.writable}</dt><dd>{workspace?.root ?? t.choose}</dd><dt>{t.host}</dt><dd>{connection?.host_id ?? t.unknown}</dd></dl>
    <p>{t.grantHelp}</p><p>{t.advisory}</p><p>{t.nativeConfirmation}</p><p className="hint">{t.noCredentials}</p>
    <label className="acknowledgement"><input type="checkbox" checked={acknowledged} disabled={busy || !canGrant} onChange={event => setAcknowledged(event.target.checked)} />{t.acknowledgement}</label>
    <button disabled={busy || !canGrant || !acknowledged || !Number.isSafeInteger(attempts) || attempts < 1 || !Number.isSafeInteger(concurrency) || concurrency < 1 || concurrency > attempts || !Number.isSafeInteger(duration) || duration < 1 || duration > 86400} onClick={() => {
      const scope = { project_id: projectId, workspace_id: workspaceId, connection_id: connectionId, max_attempts: attempts, max_concurrency: concurrency, max_duration_ms: duration * 1000 };
      void mutate(JSON.stringify(['grant', scope]), key => sessionApi.grantsCreate(scope, key), record => { setGrants(current => [...current, record]); setGrantId(record.id); setAcknowledged(false); });
    }}>{t.grant}</button>
    {!canGrant && <p className="hint">{t.grantUnavailable}</p>}
    <div className="session-start"><label htmlFor="session-grant">{t.grantSelect}</label><select id="session-grant" disabled={busy || !scopeReady} value={grantId} onChange={event => setGrantId(event.target.value)}><option value="">{t.choose}</option>{matchingGrants.map(record => <option key={record.id} value={record.id}>{record.id}</option>)}</select>
      {grant && <><p role="status">{grant.revoked ? t.revoked : grant.expires_at_ms <= Date.now() ? t.expired : t.active}</p><dl><dt>{t.expiry}</dt><dd>{new Date(grant.expires_at_ms).toISOString()}</dd><dt>{t.attempts}</dt><dd>{grant.scope.max_attempts}</dd><dt>{t.concurrency}</dt><dd>{grant.scope.max_concurrency}</dd><dt>{t.duration}</dt><dd>{grant.scope.max_duration_ms / 1000}</dd><dt>{t.networkScope}</dt><dd>{grant.scope.network.mode}</dd></dl><details><summary>{t.enforcement}</summary><pre>{JSON.stringify(grant.enforcement, null, 2)}</pre></details><p className="hint">{t.advisory}</p><button disabled={busy || grant.revoked || !connected} onClick={() => void mutate(JSON.stringify(['revoke', grant.id, grant.revision]), key => sessionApi.grantsRevoke(grant, key), record => setGrants(current => current.map(existing => existing.id === record.id ? record : existing)))}>{t.revoke}</button></>}
      <p>{t.startHelp}</p><button disabled={busy || !connected || !scopeReady || !grantMatches || task?.state !== 'draft' || !workspaceId || !connectionId || project?.archived} onClick={() => {
        if (task) void mutate(JSON.stringify(['session', task.id, task.revision, workspaceId, connectionId, grantId]), key => sessionApi.sessionsCreate(task.id, task.revision, workspaceId, connectionId, grantId, key), record => { setSessions(current => mergeSessions(current, [record])); setSessionId(record.id); });
      }}>{t.newSession}</button>{!grantMatches && <p className="hint">{t.sessionUnavailable}</p>}
    </div>
    <div className="workspace"><section>{!sessions.length && <p>{t.emptySessions}</p>}<ul className="task-list">{sessions.map(record => <li key={record.id}><button disabled={busy} aria-label={`${connections.find(item => item.id === record.connection_id)?.name ?? record.connection_id} · ${stateText(record.state)} · ${record.id}`} aria-pressed={sessionId === record.id} onClick={() => setSessionId(record.id)}><strong>{connections.find(item => item.id === record.connection_id)?.name ?? record.connection_id}</strong><span>{stateText(record.state)}</span><small>{record.id}</small></button></li>)}</ul></section>
      <section className="inspector">{selected ? <><h3>{selected.id}</h3><p role="status">{stateText(selected.state)}{selected.turn_state ? ` · ${stateText(selected.turn_state)}` : ''}{!sessionFresh && !['stopped', 'failed', 'outcome_unknown'].includes(selected.state) ? ` · ${t.stale}` : ''}</p><dl><dt>{t.task}</dt><dd>{selected.task_id}</dd><dt>{t.connection}</dt><dd>{selected.connection_id}</dd><dt>{t.recipient}</dt><dd>{connections.find(record => record.id === selected.connection_id)?.name ?? selected.connection_id}</dd><dt>{t.workspace}</dt><dd>{selected.workspace_id}</dd><dt>{t.writable}</dt><dd>{sessionWorkspace?.root ?? t.unknown}</dd><dt>{t.grantSelect}</dt><dd>{selected.grant_id}</dd><dt>{t.lastObserved}</dt><dd>{selected.updated_at}</dd></dl>{selected.last_error && <p>{selected.last_error}</p>}
        <form onSubmit={send}><label htmlFor="session-message">{t.message}</label><textarea id="session-message" value={text} onChange={event => setText(event.target.value)} disabled={busy || !connected || !sessionFresh || selected.state !== 'ready' || uncertainSend.has(selected.id)} maxLength={16384} rows={4} /><p className="hint">{t.promptHelp}</p><button disabled={busy || !connected || !sessionFresh || selected.state !== 'ready' || !text.trim() || uncertainSend.has(selected.id)}>{t.send}</button></form>
        {uncertainSend.has(selected.id) && <div className="error"><p>{t.uncertain}</p><button disabled={busy} onClick={() => { setText(''); setUncertainSend(current => { const next = new Set(current); next.delete(selected.id); return next; }); }}>{t.clearUncertain}</button></div>}
        <button disabled={busy || !connected || !['starting', 'ready', 'running'].includes(selected.state)} onClick={() => void mutate(JSON.stringify(['cancel', selected.id, selected.revision]), key => sessionApi.sessionsCancel(selected, key), record => setSessions(current => mergeSessions(current, [record])))}>{t.cancel}</button><p className="hint">{t.cancelHelp}</p><p className="hint">{t.resumeUnsupported}</p>
        <h3>{t.outputTitle}</h3><p className="hint">{t.eventWindow}</p>{byteLimitedPage && <p role="status">{messages[language].bytePageLimit}</p>}{!events.length && <p>{t.noEvents}</p>}<ol className="session-events">{events.map(event => <li key={event.event_id}>{sessionEventText(event) && <p className="message-text"><strong>{event.kind === 'session.message_queued' ? t.queuedMessage : t.agentMessage}</strong><br />{sessionEventText(event)}</p>}<details><summary>{event.kind} · {event.occurred_at}</summary><pre>{JSON.stringify(event.payload, null, 2)}</pre></details></li>)}</ol>
      </> : <p>{t.selected}</p>}</section></div>
  </section>;
}
export function mergeSessions(current: ManagedSession[], incoming: ManagedSession[]): ManagedSession[] {
  const records = new Map(current.map(record => [record.id, record]));
  for (const record of incoming) {
    if ((records.get(record.id)?.revision ?? -1) <= record.revision) records.set(record.id, record);
  }
  return [...records.values()];
}
export function sessionEventText(event: JournalEvent): string | null {
  if (!event.payload || typeof event.payload !== 'object') return null;
  const payload = event.payload as Record<string, unknown>;
  if (event.kind === 'session.message_queued' && typeof payload.text === 'string') return payload.text;
  if (event.kind !== 'session.update' || !payload.update || typeof payload.update !== 'object') return null;
  const kind = (payload.update as Record<string, unknown>).kind;
  if (!kind || typeof kind !== 'object') return null;
  const content = kind as Record<string, unknown>;
  return content.kind === 'agent_text' && typeof content.text === 'string' ? content.text : null;
}
