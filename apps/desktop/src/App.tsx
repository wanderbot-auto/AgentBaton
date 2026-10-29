import { useCallback, useEffect, useRef, useState } from 'react';
import { api, DaemonError, taskAllowsExecution, type JournalEvent, type Task, type Project } from './api';
import { initialLanguage, messages, type Language } from './i18n';
import { canvasMessages } from './canvas-i18n';
import { Projects } from './Projects';
import { TaskCodeWorkspace } from './TaskCodeWorkspace';
import { Connections } from './Connections';
import { ManagedSessions, sessionEventText } from './ManagedSessions';
import { sessionApi, type ManagedSession, type NamedConnection } from './session-api';
import { ContextDialog } from './ContextDialog';
import { ArtifactValidation } from './ArtifactValidation';
import { DataContractValidation } from './DataContractValidation';
import { CommandValidation } from './CommandValidation';
import { GoalPlanning } from './GoalPlanning';
import { FactoryRuntime } from './FactoryRuntime';
import { Icon, type IconName } from './Icons';
import { earlierEventWindow, type WindowReplacement } from './event-window';

type View = 'work' | 'tasks' | 'capabilities' | 'history' | 'quick';
type Filter = 'all' | 'active' | 'attention' | 'drafts' | 'done';
type Tab = 'conversation' | 'artifacts' | 'details';
type Dialog = 'projects' | 'execution' | 'search' | 'settings' | 'conversion' | null;
const activeStates = new Set(['queued', 'running', 'starting', 'ready']);
const attentionStates = new Set(['outcome_unknown', 'unknown', 'cancel_requested', 'failed', 'awaiting_approval']);
const finishedStates = new Set(['accepted', 'cancelled', 'failed', 'execution_succeeded', 'validation_passed']);

export function App() {
  const [language, setLanguage] = useState<Language>(initialLanguage);
  const t = messages[language], c = canvasMessages[language];
  const [view, setView] = useState<View>('work');
  const [navCollapsed, setNavCollapsed] = useState(false);
  const [mobileNav, setMobileNav] = useState(false);
  const [mobileLayout, setMobileLayout] = useState(() => window.matchMedia?.('(max-width:759px)').matches ?? false);
  useEffect(() => {
    const media = window.matchMedia?.('(max-width:759px)');
    if (!media) return;
    const change = () => { setMobileLayout(media.matches); setMobileNav(false); };
    media.addEventListener('change', change);
    return () => media.removeEventListener('change', change);
  }, []);
  const [dialog, setDialog] = useState<Dialog>(null);
  const [filter, setFilter] = useState<Filter>('all');
  const [tab, setTab] = useState<Tab>('conversation');
  const [query, setQuery] = useState('');
  const [tasks, setTasks] = useState<Task[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [connections, setConnections] = useState<NamedConnection[]>([]);
  const [sessions, setSessions] = useState<ManagedSession[]>([]);
  const [projectId, setProjectId] = useState('');
  const [events, setEvents] = useState<JournalEvent[]>([]);
  const [earliestCursor, setEarliestCursor] = useState(0);
  const [nextEventCursor, setNextEventCursor] = useState(0);
  const [eventPageMore, setEventPageMore] = useState(false);
  const [windowReplacement, setWindowReplacement] = useState<WindowReplacement>(null);
  const [byteLimitedPage, setByteLimitedPage] = useState(false);
  const eventRequestCursor = useRef<number | undefined>(undefined);
  const [selectedId, setSelectedId] = useState<string>();
  const [status, setStatus] = useState<'connecting' | 'connected' | 'disconnected'>('connecting');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [goal, setGoal] = useState('');
  const [quickSummary, setQuickSummary] = useState('');
  const [conversionSummary, setConversionSummary] = useState('');
  const [uncertainSessions, setUncertainSessions] = useState<Set<string>>(() => new Set());
  const createKey = useRef<string | null>(null);
  const messageKeys = useRef(new Map<string, string>());
  const refreshSequence = useRef(0);
  const composer = useRef<HTMLTextAreaElement>(null);
  const selected = tasks.find(task => task.id === selectedId);
  const selectedSessions = sessions.filter(session => session.task_id === selectedId);
  const readySession = selected&&taskAllowsExecution(selected)?selectedSessions.find(session => session.state === 'ready'):undefined;
  const uncertainTaskSend = selectedSessions.some(session => uncertainSessions.has(session.id));
  const selectedConnection = connections.find(record => record.id === readySession?.connection_id);
  const stateText = (state: string) => Object.hasOwn(t, state) ? t[state as keyof typeof t] : `${t.unknown}: ${state}`;
  const refresh = useCallback(async (silent = false) => {
    const request = ++refreshSequence.current;
    if (!silent) setStatus('connecting');
    try {
      const snapshot = await api.snapshot();
      if (request !== refreshSequence.current) return;
      setTasks(snapshot.tasks); setStatus('connected');
      const cursor = eventRequestCursor.current ?? Math.max(0, snapshot.sequence - 100);
      const [history, projectRecords, nextConnections, nextSessions] = await Promise.allSettled([api.events(cursor), api.projectsList(), sessionApi.connectionsList(), sessionApi.sessionsList()]);
      if (request !== refreshSequence.current) return;
      const failures: string[] = [];
      for (const result of [history, projectRecords, nextConnections, nextSessions]) {
        if (result.status === 'rejected') {
          const error: unknown = result.reason;
          failures.push(error instanceof Error ? error.message : String(error));
          if (!(error instanceof DaemonError) || ['AUTH_REQUIRED', 'PROVIDER_DISCONNECTED'].includes(error.code)) setStatus('disconnected');
        }
      }
      if (history.status === 'fulfilled') {
        const batch = history.value;
        setEvents(earlierEventWindow(batch.events, []).events); setWindowReplacement(null); setByteLimitedPage(!!batch.byte_limited); setEarliestCursor(cursor);
        setNextEventCursor(batch.next_cursor); setEventPageMore(batch.next_cursor < batch.sequence);
        if (batch.gap) failures.push(messages[language].eventGap);
      }
      if (projectRecords.status === 'fulfilled') setProjects(projectRecords.value);
      if (nextConnections.status === 'fulfilled') setConnections(nextConnections.value);
      if (nextSessions.status === 'fulfilled') setSessions(nextSessions.value);
      setError(failures.join(' '));
    } catch (e) {
      if (request !== refreshSequence.current) return;
      setStatus('disconnected'); setError(e instanceof Error ? e.message : String(e));
    }
  }, [language]);
  useEffect(() => { void refresh(); }, [refresh]);
  useEffect(() => {
    if (status !== 'connected' || busy || dialog) return;
    if (!selectedId && !tasks.some(task => activeStates.has(task.state) || task.state === 'cancel_requested')) return;
    const timer = setTimeout(() => void refresh(true), 1500);
    return () => clearTimeout(timer);
  }, [tasks, selectedId, status, busy, dialog, refresh]);
  useEffect(() => {
    document.documentElement.lang = language === 'zh' ? 'zh-CN' : 'en';
    try { localStorage.setItem('agentbaton.language', language); } catch { /* UI storage failure never changes execution authority. */ }
  }, [language]);
  const newTask = useCallback(() => {
    setView('work'); setSelectedId(undefined); setGoal(''); setTab('conversation'); setMobileNav(false); setDialog(null); createKey.current = null;
    setTimeout(() => composer.current?.focus(), 0);
  }, []);
  useEffect(() => {
    function shortcut(event: KeyboardEvent) {
      if (dialog) return;
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') { event.preventDefault(); setQuery(''); setDialog('search'); }
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'n') { event.preventDefault(); newTask(); }
      if (event.key === 'Escape' && selectedId && !(event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement)) setSelectedId(undefined);
    }
    document.addEventListener('keydown', shortcut);
    return () => document.removeEventListener('keydown', shortcut);
  }, [dialog, selectedId, newTask]);
  function showError(e: unknown) {
    setError((e instanceof Error ? e.message : String(e)) + (e instanceof DaemonError && e.sideEffectState === 'unknown' ? ` ${t.sideEffectUnknown}` : ''));
  }
  async function createTask(event?: React.SubmitEvent<HTMLFormElement>, summary?: string) {
    event?.preventDefault();
    const text = (summary ?? goal).trim();
    if (!text || busy || status !== 'connected') return;
    setBusy(true); setError(''); createKey.current ??= crypto.randomUUID();
    try {
      const task = projectId ? await api.create(text, createKey.current, projectId) : await api.create(text, createKey.current);
      createKey.current = null; setGoal(''); if (summary !== undefined) setQuickSummary('');
      setSelectedId(task.id); setView('tasks'); setTab('conversation'); setDialog(null);
      await refresh();
    } catch (e) { showError(e); }
    finally { setBusy(false); }
  }
  async function sendTaskMessage(event: React.SubmitEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!readySession || !goal.trim() || busy || status !== 'connected' || uncertainTaskSend) return;
    setBusy(true); setError('');
    const fingerprint = JSON.stringify([readySession.id, readySession.revision, goal]);
    const key = messageKeys.current.get(fingerprint) ?? crypto.randomUUID(); messageKeys.current.set(fingerprint, key);
    try {
      // Use the exact observed revision. A newer projection must not silently
      // upgrade this user action or replay an uncertain dispatch.
      await sessionApi.sessionsSend(readySession, goal, key);
      messageKeys.current.delete(fingerprint); setGoal(''); await refresh(true);
    } catch (e) {
      if (!(e instanceof DaemonError) || e.sideEffectState === 'unknown') setUncertainSessions(current => new Set([...current, readySession.id]));
      showError(e);
    } finally { setBusy(false); }
  }
  async function cancelTask(task: Task) {
    if(!['draft','running','outcome_unknown'].includes(task.state))return;
    setBusy(true); setError('');
    try { await api.cancel(task, crypto.randomUUID()); await refresh(); }
    catch (e) { showError(e); }
    finally { setBusy(false); }
  }
  async function loadEarlier() {
    setBusy(true);
    const request = ++refreshSequence.current;
    try {
      const cursor = Math.max(0, earliestCursor - 100), batch = await api.events(cursor);
      if (request !== refreshSequence.current) return;
      if (batch.gap) { setError(t.eventGap); return; }
      const older = batch.events.filter(event => event.sequence <= earliestCursor);
      if (!older.length) { setError(t.eventGap); return; }
      const window = earlierEventWindow(older, events);
      setEvents(window.events); setWindowReplacement(window.reason); setByteLimitedPage(!!batch.byte_limited); setEarliestCursor(cursor); eventRequestCursor.current = cursor;
      if (window.replaced || batch.byte_limited) { setNextEventCursor(batch.next_cursor); setEventPageMore(batch.next_cursor < batch.sequence); }
    } catch (e) { showError(e); }
    finally { setBusy(false); }
  }
  async function loadNextEvents() {
    setBusy(true);
    const request = ++refreshSequence.current;
    try {
      const cursor = nextEventCursor, batch = await api.events(cursor);
      if (request !== refreshSequence.current) return;
      if (batch.gap || (batch.next_cursor <= cursor && batch.sequence > cursor)) { setError(t.eventGap); return; }
      setEvents(earlierEventWindow(batch.events, []).events); setWindowReplacement(null); setByteLimitedPage(!!batch.byte_limited); setEarliestCursor(cursor); setNextEventCursor(batch.next_cursor); setEventPageMore(batch.next_cursor < batch.sequence); eventRequestCursor.current = cursor;
    } catch (e) { showError(e); }
    finally { setBusy(false); }
  }
  function openTask(id: string) { eventRequestCursor.current = undefined; setSelectedId(id); setGoal(''); setTab('conversation'); setView('tasks'); setDialog(null); setMobileNav(false); createKey.current = null; }
  function navigate(next: View) { if (next !== 'history') eventRequestCursor.current = undefined; setView(next); setSelectedId(undefined); setMobileNav(false); setGoal(''); createKey.current = null; }
  const filtered = tasks.filter(task => (!projectId || task.project_id === projectId) && (filter === 'all' || (filter === 'active' ? activeStates.has(task.state) : filter === 'attention' ? attentionStates.has(task.state) : filter === 'drafts' ? task.state === 'draft' : finishedStates.has(task.state))));
  const taskEvents = selected ? events.filter(event => event.aggregate_id === selected.id || selectedSessions.some(session => session.id === event.aggregate_id)) : [];
  const conversationEvents = taskEvents.filter(event => sessionEventText(event));
  const showingCanvas = view === 'work' || view === 'tasks';
  const nav: [Exclude<View, 'quick'>, IconName, string][] = [['work', 'grid', c.space], ['tasks', 'task', t.tasks], ['capabilities', 'capabilities', t.capabilities], ['history', 'history', t.history]];
  const projectName = (id: string | undefined) => projects.find(project => project.id === id)?.name ?? t.localDrafts;
  const dialogTitle = dialog === 'projects' ? c.projectSettings : dialog === 'execution' ? c.execution : dialog === 'search' ? c.searchTasks : dialog === 'conversion' ? c.convertTitle : c.settings;
  const eventPager = <>{windowReplacement && <p role="status">{windowReplacement === 'bytes' ? t.windowBudget : t.windowLimit}</p>}{byteLimitedPage && <p role="status">{t.bytePageLimit}</p>}{eventPageMore && <div className="event-page-controls"><p className="hint">{t.eventPageMore}</p><button onClick={() => void loadNextEvents()} disabled={busy || status !== 'connected'}>{t.nextEvents}</button></div>}</>;
  return <div className={`shell${navCollapsed ? ' nav-collapsed' : ''}${mobileNav ? ' mobile-nav-open' : ''}`}>
    <header className="appbar"><div className="brand"><svg className="brandmark" viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M3 19 11 4h5L8 19Zm9 1 7-13h4l-7 13Z" /></svg><span>AgentBaton</span></div><button className="icon-button nav-toggle" aria-label={c.toggleNav} aria-expanded={mobileLayout ? mobileNav : !navCollapsed} onClick={() => { if (mobileLayout) setMobileNav(value => !value); else setNavCollapsed(value => !value); }}><Icon name="panel" /></button><div className="location"><span>{c.personal}</span><span className="location-divider">/</span><button onClick={() => setDialog('projects')}>{projectId ? projectName(projectId) : c.allProjects}</button></div><span className={`connection ${status}`} role="status"><span className="status-dot" />{t[status]}</span><button className="icon-button" aria-label={t.refresh} disabled={busy} onClick={() => void refresh()}><Icon name="refresh" /></button></header>
    {mobileNav && <button className="mobile-nav-dismiss" aria-label={c.close} onClick={() => setMobileNav(false)} />}
    <aside className="sidebar"><div className="sidebar-top"><button className="new-button" aria-label={c.newTask} title={c.newTask} onClick={newTask}><Icon name="plus" /><span>{c.newTask}</span><kbd>⌘ N</kbd></button><button className="nav-item" aria-label={c.search} title={c.search} onClick={() => { setQuery(''); setDialog('search'); }}><Icon name="search" /><span>{c.search}</span><kbd>⌘ K</kbd></button></div><nav aria-label={language === 'zh' ? '主导航' : 'Main navigation'}>{nav.map(([item, icon, label]) => <button className="nav-item" key={item} aria-current={view === item ? 'page' : undefined} onClick={() => navigate(item)} title={label}><Icon name={icon} /><span>{label}</span></button>)}</nav><div className="sidebar-projects"><div className="sidebar-label">{t.projects}</div><button className="nav-item" onClick={() => setDialog('projects')} title={c.projectSettings}><Icon name="plus" /><span>{c.projectSettings}</span></button>{projects.filter(project => !project.archived).map(project => <button className="nav-item" key={project.id} aria-pressed={projectId === project.id} onClick={() => { setProjectId(current => current === project.id ? '' : project.id); navigate('work'); }} title={project.name}><Icon name="folder" /><span>{project.name}</span></button>)}</div><div className="sidebar-bottom"><button className="nav-item" aria-current={view === 'quick' ? 'page' : undefined} onClick={() => navigate('quick')} title={c.quick}><Icon name="chat" /><span>{c.quick}</span></button><label className="language"><span>{t.language}</span><select aria-label={t.language} value={language} onChange={event => setLanguage(event.target.value as Language)}><option value="en">English</option><option value="zh">简体中文</option></select></label><button className="nav-item" onClick={() => setDialog('settings')} title={c.settings}><Icon name="settings" /><span>{c.settings}</span></button></div></aside>
    <main className="main"><div className={`page-scroll${showingCanvas ? ' has-composer' : ''}`}><div className="canvas">
      {error && <div className="error" role="alert"><strong>{t.error}</strong><p>{error}</p></div>}
      {status === 'disconnected' && <section className="disconnected-panel"><h2>{t.disconnected}</h2><p>{t.disconnectedHelp}</p><button onClick={() => void refresh()}>{t.reconnect}</button></section>}
      {showingCanvas && !selected && <><div className="page-heading"><h1>{c.yourTasks}</h1><button className="text-button" onClick={() => setDialog('projects')}><Icon name="folder" />{projectId ? projectName(projectId) : c.allProjects}</button></div><div className="filter-row" aria-label={language === 'zh' ? '任务状态筛选' : 'Task status filters'}>{(['all', 'active', 'attention', 'drafts', 'done'] as const).map(item => <button key={item} aria-pressed={filter === item} onClick={() => setFilter(item)}>{c[item]}</button>)}</div>{tasks.length === 0 ? <div className="canvas-empty"><Icon name="task" /><p>{t.empty}</p></div> : filtered.length === 0 ? <p className="canvas-empty">{c.noMatches}</p> : <div className="task-grid">{filtered.map(task => {
        const session = sessions.find(record => record.task_id === task.id), connection = connections.find(record => record.id === session?.connection_id);
        const latest = [...events].reverse().find(event => event.aggregate_id === task.id);
        return <button className="task-card" data-testid="task-card" data-task-id={task.id} key={task.id} onClick={() => openTask(task.id)} aria-label={`${c.openTask}: ${task.goal}`}><div className="card-top"><span className={`task-state state-${task.state}`}><span className="status-dot" />{stateText(task.state)}</span><Icon name="arrow" /></div><h2>{task.goal}</h2><p className="card-preview">{!latest || latest.kind === 'task.created' ? c.recent : latest.kind === 'task.cancelled' ? t.cancelled : c.recordedAction}</p><div className="card-footer"><span><Icon name="folder" />{projectName(task.project_id)}</span><span>{connection?.name ?? c.noConnection}</span></div></button>;
      })}</div>}</>}
      {showingCanvas && selected && <section className="task-thread" data-testid="task-thread" data-task-id={selected.id}><div className="thread-back"><button className="text-button" onClick={() => setSelectedId(undefined)}><Icon name="back" />{c.back}</button><button className="text-button" onClick={() => setDialog('execution')}><Icon name="settings" />{c.execution}</button></div><div className="thread-heading"><h1>{selected.goal}</h1><span className={`task-state state-${selected.state}`}><span className="status-dot" />{stateText(selected.state)}</span></div><div className="thread-tabs" role="tablist" aria-label={language === 'zh' ? '任务内容' : 'Task content'}>{(['conversation', 'artifacts', 'details'] as const).map(item => <button key={item} role="tab" aria-selected={tab === item} tabIndex={tab === item ? 0 : -1} id={`tab-${item}`} aria-controls={`panel-${item}`} onKeyDown={event => {
        const order: Tab[] = ['conversation', 'artifacts', 'details'];
        const index = order.indexOf(item);
        const next = event.key === 'ArrowRight' ? order[(index + 1) % order.length] : event.key === 'ArrowLeft' ? order[(index + order.length - 1) % order.length] : event.key === 'Home' ? order[0] : event.key === 'End' ? order[order.length - 1] : null;
        if (next) { event.preventDefault(); setTab(next); document.getElementById(`tab-${next}`)?.focus(); }
      }} onClick={() => setTab(item)}>{c[item]}</button>)}</div><div role="tabpanel" id={`panel-${tab}`} aria-labelledby={`tab-${tab}`}>
        {tab === 'conversation' && <><div className="conversation"><article className="user-message"><p>{selected.goal}</p></article>{conversationEvents.map(event => <article className={event.kind === 'session.message_queued' ? 'user-message' : 'agent-message'} key={event.event_id}><p>{sessionEventText(event)}</p></article>)}</div>{!selectedSessions.length && <div className="thread-note"><p>{selected.state === 'draft' ? t.noExecution : t.noAttempt}</p><button onClick={() => setDialog('execution')}>{c.configure}</button></div>}{selectedSessions.map(session => <p className="session-observation" key={session.id}>{c.sessionState}: {stateText(session.state)}{session.turn_state ? ` · ${session.turn_state}` : ''}</p>)}<details className="folded-plan"><summary>{c.plans}</summary><p>{c.noPlan}</p></details><details className="observable-events"><summary>{c.eventDetails}</summary><EventList events={taskEvents} title={t.evidence} empty={t.noEvents} /></details>{eventPager}{earliestCursor > 0 && <p className="hint">{t.moreEvents}</p>}</>}
        {tab === 'artifacts' && <><ArtifactValidation key={selected.id} task={selected} sessions={sessions} language={language} connected={status === 'connected'} /><DataContractValidation key={`data-${selected.id}`} task={selected} sessions={sessions} language={language} connected={status === 'connected'} /><CommandValidation key={`command-${selected.id}`} task={selected} sessions={sessions} language={language} connected={status === 'connected'} /></>}
        {tab === 'details' && <div className="task-details"><dl><dt>{t.taskId}</dt><dd>{selected.id}</dd><dt>{t.project}</dt><dd>{projectName(selected.project_id)}</dd><dt>{t.revision}</dt><dd>{selected.revision}</dd><dt>{t.updated}</dt><dd>{selected.updated_at}</dd></dl><p>{c.retained}</p><button onClick={() => void cancelTask(selected)} disabled={busy || status !== 'connected' || !['draft','running','outcome_unknown'].includes(selected.state)}>{t.cancel}</button><p className="hint">{t.cancelHelp}</p><TaskCodeWorkspace key={`code-workspace-${selected.id}`} task={selected} language={language} connected={status === 'connected'} /><GoalPlanning key={`goal-${selected.id}`} task={selected} language={language} connected={status === 'connected'} /><FactoryRuntime key={`factory-${selected.id}`} task={selected} language={language} connected={status === 'connected'} /></div>}
      </div></section>}
      {view === 'capabilities' && <><div className="page-heading"><h1>{t.capabilities}</h1></div><Connections language={language} connected={status === 'connected'} compact /><section className="capability-empty"><h2>{c.capabilitySources}</h2><p>{c.noCapabilitySources}</p><p className="hint">{c.provenance}</p></section></>}
      {view === 'history' && <><div className="page-heading"><h1>{t.history}</h1></div><EventList events={events} title={t.evidence} empty={t.noEvents} />{eventPager}{earliestCursor > 0 && <><p>{events.length >= 500 ? t.windowLimit : t.moreEvents}</p><button onClick={() => void loadEarlier()} disabled={busy || status !== 'connected'}>{t.earlier}</button></>}</>}
      {view === 'quick' && <section className="quick-thread"><h1>{c.quickTitle}</h1><p>{c.quickHelp}</p><label htmlFor="quick-summary">{c.quickContext}</label><textarea id="quick-summary" value={quickSummary} maxLength={8192} rows={6} onChange={event => setQuickSummary(event.target.value)} placeholder={c.quickPlaceholder} /><div className="quick-actions"><button disabled title={c.quickHelp}>{c.send} · {c.unavailable}</button><button className="primary" disabled={busy || status !== 'connected' || !quickSummary.trim()} onClick={() => { setConversionSummary(quickSummary); createKey.current = null; setDialog('conversion'); }}>{c.convert}</button></div></section>}
    </div></div>
    {showingCanvas && <div className="composer-wrap"><form className="composer" data-testid="goal-composer" onSubmit={selected ? sendTaskMessage : createTask}><label className="sr-only" htmlFor="goal">{selected ? c.message : t.goal}</label><textarea ref={composer} data-testid="goal-input" id="goal" disabled={busy || (!!selected && (!readySession || uncertainTaskSend))} value={goal} maxLength={selected ? 16384 : 8192} onChange={event => { setGoal(event.target.value); createKey.current = null; }} onKeyDown={event => { if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) { event.preventDefault(); event.currentTarget.form?.requestSubmit(); } }} placeholder={selected ? c.messagePlaceholder : c.goalPlaceholder} required rows={2} /><div className="composer-controls"><div className="composer-left"><button className="icon-button" type="button" disabled title={c.attachmentUnavailable} aria-label={c.attachment}><Icon name="plus" /></button><label className="composer-project"><Icon name="folder" /><span className="sr-only">{t.project}</span><select disabled={busy || !!selected} value={selected?.project_id ?? projectId} onChange={event => { setProjectId(event.target.value); createKey.current = null; }}><option value="">{t.localDrafts}</option>{projects.map(project => <option key={project.id} value={project.id} disabled={project.archived}>{project.name}{project.archived ? ` (${t.archived})` : ''}</option>)}</select></label><span className="composer-divider" /><button type="button" className="text-button execution-choice" disabled={!selected} aria-label={c.connectionChoice} title={selected ? c.execution : c.automaticUnavailable} onClick={() => setDialog('execution')}><Icon name="capabilities" /><span>{selectedConnection?.name ?? (selected ? c.selectedConnection : c.automaticUnavailable)}</span></button></div><button type="submit" className="send-button primary" aria-label={selected ? c.send : t.create} disabled={busy || status !== 'connected' || !goal.trim() || (selected ? !readySession || uncertainTaskSend : projects.some(project => project.id === projectId && project.archived))}><Icon name="send" /></button></div></form><div className="composer-foot"><span>{selected ? uncertainTaskSend ? c.unsent : !readySession ? c.sessionRequired : selectedConnection?.name : c.draftHint}</span><span>{c.enterHint}</span></div></div>}
    </main>
    {dialog && <ContextDialog title={dialogTitle} closeLabel={c.close} onClose={() => setDialog(null)} wide={dialog === 'projects' || dialog === 'execution'}>
      {dialog === 'projects' && <Projects projects={projects} selectedId={projectId} onSelect={id => { setProjectId(id); createKey.current = null; }} refresh={refresh} connected={status === 'connected'} language={language} />}
      {dialog === 'execution' && <ManagedSessions initialTaskId={selected?.id} language={language} connected={status === 'connected'} projects={projects} tasks={tasks} projectId={selected?.project_id ?? projectId} onProjectChange={id => { setProjectId(id); createKey.current = null; }} refreshTasks={refresh} />}
      {dialog === 'settings' && <><dl className="settings-list"><dt>{c.appearance}</dt><dd>{c.light}</dd><dt>{c.readOnly}</dt><dd>{c.retained}</dd></dl><details className="settings-connection"><summary>{c.connectedHelp}</summary><p>{t[status]}</p></details></>}
      {dialog === 'search' && <><label className="sr-only" htmlFor="task-search">{c.searchTasks}</label><input id="task-search" value={query} onChange={event => setQuery(event.target.value)} placeholder={c.searchPlaceholder} data-dialog-autofocus /><ul className="search-results">{tasks.filter(task => task.goal.toLocaleLowerCase().includes(query.toLocaleLowerCase())).map(task => <li key={task.id}><button onClick={() => openTask(task.id)}><Icon name="task" /><span>{task.goal}</span><span className="task-state">{stateText(task.state)}</span></button></li>)}</ul>{!tasks.some(task => task.goal.toLocaleLowerCase().includes(query.toLocaleLowerCase())) && <p>{c.noMatches}</p>}</>}
      {dialog === 'conversion' && <>{error && <div className="error" role="alert"><strong>{t.error}</strong><p>{error}</p></div>}<p>{c.convertHelp}</p><label htmlFor="conversion-summary">{c.summary}</label><textarea id="conversion-summary" rows={5} value={conversionSummary} maxLength={8192} onChange={event => { setConversionSummary(event.target.value); createKey.current = null; }} /><div className="dialog-actions"><button onClick={() => setDialog(null)}>{c.close}</button><button className="primary" disabled={busy || status !== 'connected' || !conversionSummary.trim()} onClick={() => void createTask(undefined, conversionSummary)}>{c.createDraft}</button></div></>}
    </ContextDialog>}
  </div>;
}
function EventList({ events, title, empty }: { events: JournalEvent[]; title: string; empty: string }) {
  return <section className="events"><h2>{title}</h2>{events.length ? <ol>{events.map(event => <li key={event.event_id}><details><summary><span>{event.kind}</span> <time>{event.occurred_at}</time> <span>#{event.sequence}</span></summary><pre>{JSON.stringify(event.payload, null, 2)}</pre></details></li>)}</ol> : <p>{empty}</p>}</section>;
}
