import { useCallback, useEffect, useRef, useState } from 'react';
import { api, DaemonError, type JournalEvent, type Task, type Project } from './api';
import { initialLanguage, messages, type Language } from './i18n';
import { Projects } from './Projects';
import { Connections } from './Connections';
import { ManagedSessions } from './ManagedSessions';
import { sessionMessages } from './session-i18n';
import { earlierEventWindow, type WindowReplacement } from './event-window';

type View = 'work' | 'tasks' | 'capabilities' | 'history';
export function App() {
  const [language, setLanguage] = useState<Language>(initialLanguage);
  const t = messages[language];
  const [view, setView] = useState<View>('work');
  const [sessionsOpen, setSessionsOpen] = useState(false);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [projectId, setProjectId] = useState('');
  const [events, setEvents] = useState<JournalEvent[]>([]);
  const [earliestCursor, setEarliestCursor] = useState(0);
  const [windowReplacement, setWindowReplacement] = useState<WindowReplacement>(null);
  const [byteLimitedPage, setByteLimitedPage] = useState(false);
  const [selectedId, setSelectedId] = useState<string>();
  const [status, setStatus] = useState<'connecting' | 'connected' | 'disconnected'>('connecting');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [goal, setGoal] = useState('');
  const createKey = useRef<string | null>(null);
  const refreshSequence = useRef(0);
  const selected = tasks.find(task => task.id === selectedId);
  const stateText = (state: string) => state in t ? t[state as keyof typeof t] : `${t.unknown}: ${state}`;
  const refresh = useCallback(async () => {
    const request = ++refreshSequence.current;
    setStatus('connecting');
    try {
      const snapshot = await api.snapshot();
      const projectRecords = await api.projectsList();
      if (request !== refreshSequence.current) return;
      setTasks(snapshot.tasks);
      setProjects(projectRecords);
      setStatus('connected');
      const cursor = Math.max(0, snapshot.sequence - 100);
      const batch = await api.events(cursor).catch((eventError: unknown) => {
        if (eventError instanceof DaemonError && eventError.code === 'RESOURCE_LIMIT') {
          if (request === refreshSequence.current) setError(eventError.message);
          return null; // A bounded-history refusal does not disconnect authenticated task controls.
        }
        throw eventError;
      });
      if (request !== refreshSequence.current || !batch) return;
      setEvents(earlierEventWindow(batch.events, []).events);
      setWindowReplacement(null);
      setByteLimitedPage(!!batch.byte_limited);
      setEarliestCursor(cursor);
      setError(batch.gap ? messages[language].eventGap : '');
    } catch (e) {
      if (request !== refreshSequence.current) return;
      setStatus('disconnected');
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [language]);
  useEffect(() => { void refresh(); }, [refresh]);
  useEffect(() => {
    document.documentElement.lang = language === 'zh' ? 'zh-CN' : 'en';
    localStorage.setItem('agentbaton.language', language);
  }, [language]);
  function showError(e: unknown) {
    setError((e instanceof Error ? e.message : String(e)) + (e instanceof DaemonError && e.sideEffectState === 'unknown' ? ` ${t.sideEffectUnknown}` : ''));
  }
  async function createTask(event: React.SubmitEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!goal.trim() || busy) return;
    setBusy(true); setError('');
    createKey.current ??= crypto.randomUUID();
    try {
      const task = projectId ? await api.create(goal.trim(), createKey.current, projectId) : await api.create(goal.trim(), createKey.current);
      createKey.current = null;
      setGoal(''); setSelectedId(task.id); setView('tasks');
      await refresh();
    } catch (e) { showError(e); }
    finally { setBusy(false); }
  }
  async function cancelTask(task: Task) {
    setBusy(true); setError('');
    try { await api.cancel(task, crypto.randomUUID()); await refresh(); }
    catch (e) { showError(e); }
    finally { setBusy(false); }
  }
  async function loadEarlier() {
    setBusy(true);
    try {
      const cursor = Math.max(0, earliestCursor - 100);
      const batch = await api.events(cursor);
      if (batch.gap) { setError(t.eventGap); return; }
      const window = earlierEventWindow(batch.events.filter(event => event.sequence <= earliestCursor), events);
      setEvents(window.events);
      if (window.replaced) setWindowReplacement(window.reason);
      setByteLimitedPage(!!batch.byte_limited);
      setEarliestCursor(cursor);
    } catch (e) { showError(e); }
    finally { setBusy(false); }
  }
  return <div className="shell">
    <aside>
      <div className="brand">AgentBaton<span>LOCAL WORKSPACE</span></div>
      <nav aria-label={language === 'zh' ? '主导航' : 'Main navigation'}>
        {(['work', 'tasks', 'capabilities', 'history'] as const).map(item => <button key={item} aria-current={view === item ? 'page' : undefined} onClick={() => setView(item)}>{t[item]}</button>)}
      </nav>
      <label className="language">{t.language}<select value={language} onChange={event => setLanguage(event.target.value as Language)}><option value="en">English</option><option value="zh">简体中文</option></select></label>
    </aside>
    <main>
      <header><div><p className="eyebrow">{t[view]}</p><h1>{t.title}</h1><p>{t.subtitle}</p></div><button onClick={() => void refresh()} disabled={busy}>{t.refresh}</button></header>
      <p className={`connection ${status}`} role="status">{t[status]}</p>
      {error && <div className="error" role="alert"><strong>{t.error}</strong><p>{error}</p></div>}
      {status === 'disconnected' && <section className="empty"><h2>{t.disconnected}</h2><p>{t.disconnectedHelp}</p><button onClick={() => void refresh()}>{t.reconnect}</button></section>}
      {(view === 'work' || view === 'tasks') && <>
        {view === 'work' && <Projects projects={projects} selectedId={projectId} onSelect={id => { setProjectId(id); createKey.current = null; }} refresh={refresh} connected={status === 'connected'} language={language} />}
        {view === 'work' && <section><button aria-expanded={sessionsOpen} aria-controls="session-workbench" onClick={() => setSessionsOpen(open => !open)}>{sessionMessages[language].sessions}</button>{sessionsOpen && <div id="session-workbench"><ManagedSessions language={language} connected={status === 'connected'} projects={projects} tasks={tasks} projectId={projectId} onProjectChange={id => { setProjectId(id); createKey.current = null; }} refreshTasks={refresh} /></div>}</section>}
        <form onSubmit={createTask} className="create-task">
          <label htmlFor="task-project">{t.project}</label><select id="task-project" disabled={busy} value={projectId} onChange={event => { setProjectId(event.target.value); createKey.current = null; }} aria-describedby="project-help"><option value="">{t.localDrafts}</option>{projects.map(project => <option key={project.id} value={project.id} disabled={project.archived}>{project.name}{project.archived ? ` (${t.archived})` : ''}</option>)}</select><p id="project-help">{t.projectHelp}</p>
          <label htmlFor="goal">{t.goal}</label><textarea id="goal" disabled={busy} value={goal} maxLength={8192} onChange={event => { setGoal(event.target.value); createKey.current = null; }} aria-describedby="create-help" required rows={3} />
          <p id="create-help">{t.createHelp}</p><button type="submit" className="primary" disabled={busy || status !== 'connected' || !goal.trim() || projects.some(project => project.id === projectId && project.archived)}>{busy ? t.working : t.create}</button>
        </form>
        <div className="workspace"><section><h2>{t.recorded} <span>{tasks.length}</span></h2>{tasks.length === 0 ? <p className="empty">{t.empty}</p> : <ul className="task-list">{tasks.map(task => <li key={task.id}><button onClick={() => setSelectedId(task.id)} aria-pressed={selectedId === task.id}><strong>{task.goal}</strong><span>{stateText(task.state)}</span></button></li>)}</ul>}</section>
          <section className="inspector">{selected ? <><h2>{selected.goal}</h2><p className="state">{stateText(selected.state)}</p><dl><dt>{t.taskId}</dt><dd>{selected.id}</dd><dt>{t.project}</dt><dd>{projects.find(project => project.id === selected.project_id)?.name ?? selected.project_id ?? t.localDrafts}</dd><dt>{t.revision}</dt><dd>{selected.revision}</dd><dt>{t.updated}</dt><dd>{selected.updated_at}</dd></dl><p>{selected.state === 'draft' ? t.noExecution : t.noAttempt}</p><button onClick={() => void cancelTask(selected)} disabled={busy || status !== 'connected' || ['cancel_requested', 'cancelled', 'accepted', 'failed'].includes(selected.state)}>{t.cancel}</button><p className="hint">{t.cancelHelp}</p><EventList events={events.filter(event => event.aggregate_id === selected.id)} title={t.evidence} empty={t.noEvents} />{earliestCursor > 0 && <p>{t.moreEvents}</p>}</> : <p>{t.select}</p>}</section>
        </div>
      </>}
      {view === 'capabilities' && <Connections language={language} connected={status === 'connected'} />}
      {view === 'history' && <>{windowReplacement && <p role="status">{windowReplacement === 'bytes' ? t.windowBudget : t.windowLimit}</p>}{byteLimitedPage && <p role="status">{t.bytePageLimit}</p>}<EventList events={events} title={t.evidence} empty={t.noEvents} />{earliestCursor > 0 && <><p>{events.length >= 500 ? t.windowLimit : t.moreEvents}</p><button onClick={() => void loadEarlier()} disabled={busy || events.length >= 500 || status !== 'connected'}>{t.earlier}</button></>}</>}
    </main>
  </div>;
}
function EventList({ events, title, empty }: { events: JournalEvent[]; title: string; empty: string }) {
  return <section className="events"><h2>{title}</h2>{events.length ? <ol>{events.map(event => <li key={event.event_id}><details><summary><span>{event.kind}</span> <time>{event.occurred_at}</time> <small>#{event.sequence}</small></summary><pre>{JSON.stringify(event.payload, null, 2)}</pre></details></li>)}</ol> : <p>{empty}</p>}</section>;
}
