import { Alert, AppShell, Tabs } from '@mantine/core';
import { Button, Code, Disclosure, DisclosureLabel, Form, Input, Select, SelectOption, Textarea, Title } from './ui';
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { api, DaemonError, acknowledgeTaskMessage, taskAllowsExecution, type JournalEvent, type Task, type Project, type TaskMessageReceipt } from './api';
import { initialLanguage, messages, type Language } from './i18n';
import { canvasMessages } from './canvas-i18n';
import { Projects } from './Projects';
import { TaskCodeWorkspace } from './TaskCodeWorkspace';
import { TaskExperienceProposal } from './TaskExperienceProposal';
import {RecoveryBanner,useRecoveryCenter} from './RecoveryCenter';
import { ManagedSessions, sessionEventText } from './ManagedSessions';
import { sessionApi, type ManagedSession, type NamedConnection } from './session-api';
import { sessionMessages } from './session-i18n';
import { useInlineNavigation, type InlinePage } from './useInlineNavigation';
import { useHostRegistrationSession } from './Hosts';
import { CapabilitiesPage } from './CapabilitiesPage';
import { SettingsPanel, type SettingsSection } from './SettingsPanel';
import { ArtifactValidation } from './ArtifactValidation';
import { DataContractValidation } from './DataContractValidation';
import { CommandValidation } from './CommandValidation';
import { GoalPlanning } from './GoalPlanning';
import { TaskActions, type TaskAction } from './TaskActions';
import { taskActionsMessages } from './task-actions-i18n';
import { TaskPlanSummary } from './TaskPlanSummary';
import { QuickConversation } from './QuickConversation';
import { StandaloneConversations } from './StandaloneConversations';
import { standaloneConversationMessages } from './standalone-conversations-i18n';
import { TaskIntegration } from './TaskIntegration';
import { FactoryRuntime } from './FactoryRuntime';
import { Icon, type IconName } from './Icons';
import { earlierEventWindow, type WindowReplacement } from './event-window';
import { TaskNotifications } from './TaskNotifications';

type View = 'tasks' | 'capabilities' | 'history' | 'quick';
type Filter = 'all' | 'active' | 'attention' | 'drafts' | 'done';
type Tab = 'conversation' | 'artifacts' | 'details';
const activeStates = new Set(['queued', 'running', 'starting', 'ready']);
const attentionStates = new Set(['outcome_unknown', 'unknown', 'cancel_requested', 'failed', 'awaiting_approval']);
const finishedStates = new Set(['accepted', 'cancelled', 'failed', 'execution_succeeded', 'validation_passed']);

const recordedSessionStates: readonly ManagedSession['state'][] = [
  'starting', 'ready', 'running', 'cancel_requested', 'stopped', 'outcome_unknown', 'failed',
];
const recordedTurnLabels = {
  idle: 'idle', pending: 'queuedMessage', running: 'running', cancel_requested: 'cancel_requested',
  completed: 'completed', ended: 'ended', failed: 'failed', outcome_unknown: 'outcome_unknown',
} as const satisfies Record<string, keyof typeof sessionMessages.en>;

function recordedSessionState(session: ManagedSession, language: Language) {
  const t = sessionMessages[language];
  const process = recordedSessionStates.includes(session.state) ? t[session.state] : `${t.unknown}: ${session.state}`;
  const turn = session.turn_state;
  if (!turn) return process;
  if (turn === 'cancelled') return `${process} · ${language === 'zh' ? '本轮已取消' : 'Turn cancelled'}`;
  if (turn === 'interrupted') return `${process} · ${language === 'zh' ? '本轮已中断' : 'Turn interrupted'}`;
  const turnLabel = Object.hasOwn(recordedTurnLabels, turn)
    ? t[recordedTurnLabels[turn as keyof typeof recordedTurnLabels]]
    : `${t.unknown}: ${turn}`;
  return `${process} · ${turnLabel}`;
}

export function App() {
  const [language, setLanguage] = useState<Language>(initialLanguage);
  const t = messages[language], c = canvasMessages[language];
  const [view, setView] = useState<View>('tasks');
  const { active: page, visits: pageVisits, open: openPage, back: backPage, backToBase, clear: clearPages, scrollRef } = useInlineNavigation();
  const [settingsSection, setSettingsSection] = useState<SettingsSection>('general');
  const recovery=useRecoveryCenter(page === 'settings');
  const hostRegistration = useHostRegistrationSession();
  const [taskIntent,setTaskIntent] = useState<{taskId:string;projectId:string|undefined;action:TaskAction;key:string}|null>(null);
  const [detailsTaskId,setDetailsTaskId]=useState<string>();
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
  const [tasksFresh, setTasksFresh] = useState(false);
  const [sessionsFresh, setSessionsFresh] = useState(false);
  const [eventsFresh, setEventsFresh] = useState(false);
  const [eventGapObserved, setEventGapObserved] = useState(false);
  const [eventSequence, setEventSequence] = useState<number | null>(null);
  const [focusedEventId, setFocusedEventId] = useState<string>();
  const [focusedEventRequest, setFocusedEventRequest] = useState(0);
  const eventRequestCursor = useRef<number | undefined>(undefined);
  const [selectedId, setSelectedId] = useState<string>();
  const [status, setStatus] = useState<'connecting' | 'connected' | 'disconnected'>('connecting');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [sendBusy,setSendBusy]=useState(false);
  const sendBusyRef=useRef(false);
  const changeSendBusy=useCallback((value:boolean)=>{sendBusyRef.current=value;setSendBusy(value);},[]);
  const [goal, setGoal] = useState('');
  const [quickSummary, setQuickSummary] = useState('');
  const [quickSummaryProject,setQuickSummaryProject] = useState<string|undefined>(undefined);
  const [conversionSummary, setConversionSummary] = useState('');
  const [conversionProject,setConversionProject] = useState<string|undefined>(undefined);
  const [recoveredMessages, setRecoveredMessages] = useState<TaskMessageReceipt[]>([]);
  const [messageRecoveryUnavailable, setMessageRecoveryUnavailable] = useState(false);
  const [uncertainSessions, setUncertainSessions] = useState<Set<string>>(() => new Set());
  const createKey = useRef<string | null>(null);
  const messageKeys = useRef(new Map<string, string>());
  const refreshSequence = useRef(0);
  const composer = useRef<HTMLTextAreaElement>(null);
  const taskTitle = useRef<HTMLHeadingElement>(null);
  const returningTask = useRef<string | undefined>(undefined);
  const selected = tasks.find(task => task.id === selectedId);
  const visibleTaskId = selected?.id;
  const closeTask = useCallback(() => {
    returningTask.current = selectedId;
    setSelectedId(undefined);
  }, [selectedId]);
  useLayoutEffect(() => {
    if (visibleTaskId) {
      if (scrollRef.current) scrollRef.current.scrollTop = 0;
      taskTitle.current?.focus({ preventScroll: true });
    } else if (returningTask.current) {
      const cards = scrollRef.current?.querySelectorAll<HTMLButtonElement>('.task-card');
      const card = Array.from(cards ?? []).find(item => item.dataset.taskId === returningTask.current);
      returningTask.current = undefined;
      card?.focus();
    }
  }, [visibleTaskId, scrollRef]);
  useEffect(()=>{setTaskIntent(null);setDetailsTaskId(undefined);},[selectedId]);
  const selectedSessions = sessions.filter(session => session.task_id === selectedId);
  const readySessions=selectedSessions.filter(session=>session.state==='ready'&&session.project_id===selected?.project_id);
  const readySession=!messageRecoveryUnavailable&&selected&&taskAllowsExecution(selected)&&readySessions.length===1?readySessions[0]:undefined;
  const uncertainTaskSend = selectedSessions.some(session => uncertainSessions.has(session.id));
  const uncertainCurrent=useRef(uncertainSessions);uncertainCurrent.current=uncertainSessions;
  const selectionGeneration=useRef({id:selectedId,epoch:0});
  if(selectionGeneration.current.id!==selectedId)selectionGeneration.current={id:selectedId,epoch:selectionGeneration.current.epoch+1};
  const sendLock=useRef(false);
  const currentTask=useRef(selected);currentTask.current=selected;
  const currentSessions=useRef(sessions);currentSessions.current=sessions;
  const currentStatus=useRef(status);currentStatus.current=status;
  const currentGoal=useRef(goal);currentGoal.current=goal;
  const selectedConnection = connections.find(record => record.id === readySession?.connection_id);
  const stateText = (state: string) => Object.hasOwn(t, state) ? t[state as keyof typeof t] : `${t.unknown}: ${state}`;
  const refresh = useCallback(async (silent = false) => {
    const request = ++refreshSequence.current;
    setTasksFresh(false); setSessionsFresh(false); setEventsFresh(false);
    if (!silent) setStatus('connecting');
    try {
      const snapshot = await api.snapshot();
      if (request !== refreshSequence.current) return;
      const messageRecovery = snapshot.task_message_recovery;
      if (messageRecovery) {
        setMessageRecoveryUnavailable('error' in messageRecovery);
        if (!('error' in messageRecovery)) {
          setRecoveredMessages(messageRecovery.recovered);
          setUncertainSessions(new Set([
            ...messageRecovery.pending_sessions,
            ...messageRecovery.recovered.map(receipt => receipt.session_id),
          ]));
        }
      }
      setTasks(snapshot.tasks); setTasksFresh(true); setStatus('connected');
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
        setEventSequence(batch.sequence);
        setEvents(earlierEventWindow(batch.events, []).events); setWindowReplacement(null); setByteLimitedPage(!!batch.byte_limited); setEarliestCursor(cursor);
        setNextEventCursor(batch.next_cursor); setEventPageMore(batch.next_cursor < batch.sequence);
        setEventsFresh(!batch.gap);
        if (batch.gap) { setEventGapObserved(true); failures.push(messages[language].eventGap); }
      }
      if (projectRecords.status === 'fulfilled') setProjects(projectRecords.value);
      if (nextConnections.status === 'fulfilled') setConnections(nextConnections.value);
      if (nextSessions.status === 'fulfilled') { setSessions(nextSessions.value); setSessionsFresh(true); }
      setError(failures.join(' '));
    } catch (e) {
      if (request !== refreshSequence.current) return;
      setStatus('disconnected'); setError(e instanceof Error ? e.message : String(e));
    }
  }, [language]);
  useEffect(() => { void refresh(); }, [refresh]);
  useEffect(() => {
    if (status !== 'connected' || busy || sendBusy) return;
    if (!selectedId && !tasks.some(task => activeStates.has(task.state) || task.state === 'cancel_requested')) return;
    const timer = setTimeout(() => void refresh(true), 1500);
    return () => clearTimeout(timer);
  }, [tasks, selectedId, status, busy, sendBusy, refresh]);
  useEffect(() => {
    document.documentElement.lang = language === 'zh' ? 'zh-CN' : 'en';
    try { localStorage.setItem('agentbaton.language', language); } catch { /* UI storage failure never changes execution authority. */ }
  }, [language]);
  const newTask = useCallback(() => {
    setView('tasks'); setSelectedId(undefined); setGoal(''); setTab('conversation'); clearPages(); createKey.current = null;
    setTimeout(() => composer.current?.focus(), 0);
  }, [clearPages]);
  useEffect(() => {
    function shortcut(event: KeyboardEvent) {
      if (event.defaultPrevented || document.querySelector('[role="dialog"][aria-modal="true"]')) return;
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') { event.preventDefault(); setQuery(''); openPage('search'); return; }
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'n') { event.preventDefault(); newTask(); return; }
      if (event.key === 'Escape' && page) { event.preventDefault(); backPage(); return; }
      if (event.key === 'Escape' && selectedId && !(event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement)) closeTask();
    }
    document.addEventListener('keydown', shortcut);
    return () => document.removeEventListener('keydown', shortcut);
  }, [page, selectedId, newTask, openPage, backPage, closeTask]);
  function showError(e: unknown) {
    setError((e instanceof Error ? e.message : String(e)) + (e instanceof DaemonError && e.sideEffectState === 'unknown' ? ` ${t.sideEffectUnknown}` : ''));
  }
  async function createTask(event?: React.SubmitEvent<HTMLFormElement>, summary?: string, projectOverride?: string) {
    event?.preventDefault();
    const text = (summary ?? goal).trim();
    if (!text || busy || status !== 'connected') return;
    setBusy(true); setError(''); createKey.current ??= crypto.randomUUID();
    try {
      const targetProject = projectOverride ?? projectId;
      const task = targetProject ? await api.create(text, createKey.current, targetProject) : await api.create(text, createKey.current);
      createKey.current = null; setGoal(''); if (summary !== undefined) { setQuickSummary(''); setQuickSummaryProject(undefined); }
      setSelectedId(task.id); setView('tasks'); setTab('conversation'); clearPages();
      await refresh();
    } catch (e) { showError(e); }
    finally { setBusy(false); }
  }
  async function sendTaskText(observedTask:Task,text:string,clearComposer=false) {
    const observed=readySession;
    if (!observed || !Number.isSafeInteger(observed.revision) || observed.revision<1 || currentTask.current?.id!==observedTask.id || !text.trim() || busy || sendBusyRef.current || sendLock.current || status!=='connected' || uncertainTaskSend) return;
    const selectionEpoch=selectionGeneration.current.epoch;
    sendLock.current=true;changeSendBusy(true);setError('');
    const taskStamp=(task:Task)=>JSON.stringify([task.id,task.project_id,task.revision,task.spec_revision,task.state,task.goal]);
    const sessionStamp=(session:ManagedSession)=>JSON.stringify([session.id,session.task_id,session.project_id,session.workspace_id,session.connection_id,session.grant_id,session.native_session_id,session.revision,session.state]);
    const fingerprint=JSON.stringify([taskStamp(observedTask),sessionStamp(observed),text]);
    const key=messageKeys.current.get(fingerprint)??crypto.randomUUID();messageKeys.current.set(fingerprint,key);
    let dispatched=false;
    try {
      const [task,session]=await Promise.all([api.taskGet(observedTask.id),sessionApi.sessionsGet(observed.id)]);
      const now=currentTask.current,nowSession=currentSessions.current.find(row=>row.id===observed.id);
      if (selectionGeneration.current.epoch!==selectionEpoch || !now || !nowSession || currentSessions.current.some(row=>row.task_id===observedTask.id&&uncertainCurrent.current.has(row.id)) || currentStatus.current!=='connected' || taskStamp(now)!==taskStamp(observedTask) || taskStamp(task)!==taskStamp(observedTask) || sessionStamp(nowSession)!==sessionStamp(observed) || sessionStamp(session)!==sessionStamp(observed) || session.task_id!==task.id || session.project_id!==task.project_id || !taskAllowsExecution(task) || session.state!=='ready') throw new Error(taskActionsMessages[language].changed);
      dispatched=true;
      const result=await sessionApi.sessionsSend(observed,text,key);
      if (!result || result.id!==observed.id || result.task_id!==observedTask.id || result.project_id!==observed.project_id || result.connection_id!==observed.connection_id || result.workspace_id!==observed.workspace_id || result.grant_id!==observed.grant_id || observed.native_session_id!==null&&result.native_session_id!==observed.native_session_id || !Number.isSafeInteger(result.revision) || result.revision<=observed.revision) throw new Error(taskActionsMessages[language].uncertain);
      messageKeys.current.delete(fingerprint);
      if(clearComposer&&selectionGeneration.current.epoch===selectionEpoch&&currentTask.current?.id===observedTask.id&&currentGoal.current===text)setGoal('');
      await acknowledgeTaskMessage(result.id, key).catch(() => { /* Refresh will show the retained receipt. */ });
      await refresh(true);
    } catch (e) {
      if(dispatched&&(!(e instanceof DaemonError)||e.code==='OUTCOME_UNKNOWN'||!['none','not_started'].includes(e.sideEffectState)))setUncertainSessions(current=>new Set([...current,observed.id]));
      if(selectionGeneration.current.epoch===selectionEpoch&&currentTask.current?.id===observedTask.id)showError(e);
    } finally {sendLock.current=false;changeSendBusy(false);}
  }
  async function sendTaskMessage(event:React.SubmitEvent<HTMLFormElement>) {
    event.preventDefault();if(selected)await sendTaskText(selected,goal,true);
  }
  function navigateTask(task:Task,action:TaskAction,opener:HTMLButtonElement) {
    if(busy||selected?.id!==task.id||selected.project_id!==task.project_id)return;
    setTaskIntent({taskId:task.id,projectId:task.project_id,action,key:crypto.randomUUID()});
    if(action==='workspace')openPage('execution', opener);
    else {setDetailsTaskId(task.id);setTab('details');clearPages();}
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
    setEventsFresh(false);
    try {
      const cursor = Math.max(0, earliestCursor - 100), batch = await api.events(cursor);
      if (request !== refreshSequence.current) return;
      if (batch.gap) { setEventGapObserved(true); setError(t.eventGap); return; }
      const older = batch.events.filter(event => event.sequence <= earliestCursor);
      if (!older.length) { setEventGapObserved(true); setError(t.eventGap); return; }
      const window = earlierEventWindow(older, events);
      setEventSequence(batch.sequence);
      setEvents(window.events); setWindowReplacement(window.reason); setByteLimitedPage(!!batch.byte_limited); setEarliestCursor(cursor); eventRequestCursor.current = cursor;
      setEventsFresh(true);
      if (window.replaced || batch.byte_limited) { setNextEventCursor(batch.next_cursor); setEventPageMore(batch.next_cursor < batch.sequence); }
    } catch (e) { showError(e); }
    finally { setBusy(false); }
  }
  async function loadNextEvents() {
    setBusy(true);
    const request = ++refreshSequence.current;
    setEventsFresh(false);
    try {
      const cursor = nextEventCursor, batch = await api.events(cursor);
      if (request !== refreshSequence.current) return;
      if (batch.gap || (batch.next_cursor <= cursor && batch.sequence > cursor)) { setEventGapObserved(true); setError(t.eventGap); return; }
      setEventSequence(batch.sequence);
      setEvents(earlierEventWindow(batch.events, []).events); setWindowReplacement(null); setByteLimitedPage(!!batch.byte_limited); setEarliestCursor(cursor); setNextEventCursor(batch.next_cursor); setEventPageMore(batch.next_cursor < batch.sequence); eventRequestCursor.current = cursor;
      setEventsFresh(true);
    } catch (e) { showError(e); }
    finally { setBusy(false); }
  }
  function openTask(id: string) { eventRequestCursor.current = undefined; setSelectedId(id); setGoal(''); setTab('conversation'); setView('tasks'); clearPages(); createKey.current = null; }
  function viewNotificationEvent(eventId: string) { setFocusedEventId(eventId); setFocusedEventRequest(current => current + 1); setView('history'); setSelectedId(undefined); clearPages(); }
  function navigate(next: View) {
    if (page && next === view) {
      backToBase();
      return;
    }
    if (next !== 'history') eventRequestCursor.current = undefined;
    setView(next); clearPages(); setSelectedId(undefined); setGoal(''); createKey.current = null;
  }
  const filtered = tasks.filter(task => (!projectId || task.project_id === projectId) && (filter === 'all' || (filter === 'active' ? activeStates.has(task.state) : filter === 'attention' ? attentionStates.has(task.state) : filter === 'drafts' ? task.state === 'draft' : finishedStates.has(task.state))));
  const taskEvents = selected ? events.filter(event => event.aggregate_id === selected.id || selectedSessions.some(session => session.id === event.aggregate_id)) : [];
  const conversationEvents = taskEvents.filter(event => sessionEventText(event));
  const displayedTurns = new Set(conversationEvents
    .filter(event => event.kind === 'session.message_queued')
    .map(event => (event.payload as { turn_id?: string } | null)?.turn_id));
  const recoveredForTask = recoveredMessages.filter(receipt => receipt.task_id === selected?.id);
  const recoveryLabels = language === 'zh'
    ? { received: '已找回任务消息回执', acknowledge: '确认已查看', review: '请确认已查看找回的回执，再发送下一条消息。', pending: '有任务消息等待核对回执。刷新可重新查询。', unavailable: '任务消息回执暂不可用。请刷新后再发送。' }
    : { received: 'Task message receipt recovered', acknowledge: 'Mark as seen', review: 'Mark the recovered receipt as seen before sending another message.', pending: 'A task message is awaiting its receipt. Refresh to check again.', unavailable: 'Task message recovery is unavailable. Refresh before sending.' };
  async function markMessageSeen(receipt: TaskMessageReceipt) {
    try {
      await acknowledgeTaskMessage(receipt.session_id, receipt.request_key);
      setRecoveredMessages(current => current.filter(row => row.session_id !== receipt.session_id || row.request_key !== receipt.request_key));
      if (currentTask.current?.id === receipt.task_id) {
        setGoal(text => text === receipt.text ? '' : text);
      }
      await refresh(true);
    } catch (failure) {
      showError(failure);
    }
  }
  const showingCanvas = view === 'tasks';
  const showComposer = showingCanvas && (!selected || tab === 'conversation');
  const nav: [Exclude<View, 'quick'>, IconName, string][] = [['tasks', 'grid', t.tasks], ['capabilities', 'capabilities', t.capabilities], ['history', 'history', t.history]];
  const projectName = (id: string | undefined) => projects.find(project => project.id === id)?.name ?? t.localDrafts;
  const intentTask=taskIntent&&selectedId===taskIntent.taskId?tasks.find(task=>task.id===taskIntent.taskId&&task.project_id===taskIntent.projectId):undefined;
  const pageTitles: Record<InlinePage, string> = {
    standalone: standaloneConversationMessages[language].title, projects: c.projectSettings,
    execution: c.execution, search: c.searchTasks, conversion: c.convertTitle, settings: c.settings,
  };
  const eventPager = <>{windowReplacement && <p role="status">{windowReplacement === 'bytes' ? t.windowBudget : t.windowLimit}</p>}{byteLimitedPage && <p role="status">{t.bytePageLimit}</p>}{eventPageMore && <div className="event-page-controls"><p className="hint">{t.eventPageMore}</p><Button onClick={() => void loadNextEvents()} disabled={busy || status !== 'connected'}>{t.nextEvents}</Button></div>}</>;
  return <AppShell className="shell" navbar={{ width: { base: 50, xs: 68 }, breakpoint: 0 }} padding={0}>
    <AppShell.Navbar component="aside" className="sidebar">
      <div className="sidebar-top">
        <Button className="new-button" aria-label={c.newTask} title={c.newTask} aria-keyshortcuts="Meta+N Control+N" onClick={newTask}><Icon name="plus" /></Button>
        <Button className="nav-item" aria-label={c.search} title={c.search} aria-keyshortcuts="Meta+K Control+K" aria-current={page === 'search' ? 'page' : undefined} onClick={event => { setQuery(''); openPage('search', event.currentTarget); }}><Icon name="search" /></Button>
      </div>
      <nav aria-label={language === 'zh' ? '主导航' : 'Main navigation'}>
        {nav.map(([item, icon, label]) => <Button className="nav-item" key={item} aria-label={label} aria-current={!page && view === item ? 'page' : undefined} onClick={() => navigate(item)} title={label}><Icon name={icon} /></Button>)}
        <Button className="nav-item" aria-label={c.quick} title={c.quick} aria-current={view === 'quick' && !page ? 'page' : undefined} onClick={() => navigate('quick')}><Icon name="chat" /></Button>
      </nav>
      <div className="sidebar-bottom">
        <TaskNotifications tasks={tasks} sessions={sessions} events={events} language={language} observation={{ connected: status === 'connected', tasksFresh, sessionsFresh, eventsFresh, eventSequence, eventLimited: eventGapObserved || earliestCursor > 0 || eventPageMore || byteLimitedPage || windowReplacement !== null }} onOpenTask={openTask} onViewEvent={viewNotificationEvent} />
        <div className="sidebar-refresh">
          <Button type="button" className="nav-item" aria-label={t.refresh} aria-describedby="connection-status"
            title={t.refresh + ' · ' + t[status]} disabled={busy} onClick={() => void refresh()}><Icon name="refresh" /></Button>
          <span role="status" id="connection-status" className={'connection ' + status}>
            <span className="status-dot" aria-hidden="true" /><span className="sr-only">{t[status]}</span>
          </span>
        </div>
        <Button className="nav-item" aria-label={c.projectSettings} title={projectId ? c.projectSettings + ' · ' + projectName(projectId) : c.projectSettings} aria-current={page === 'projects' ? 'page' : undefined} onClick={event => openPage('projects', event.currentTarget, !selected)}><Icon name="folder" /></Button>
        <Button className="nav-item" data-testid="settings-nav" aria-label={c.settings} title={c.settings} aria-current={page === 'settings' ? 'page' : undefined} onClick={event => { setSettingsSection('general'); openPage('settings', event.currentTarget, true); }}><Icon name="settings" /></Button>
      </div>
    </AppShell.Navbar>
    <AppShell.Main className="main"><div ref={scrollRef} tabIndex={-1} className="page-scroll"><div className="canvas">
      {error && <Alert className="error" role="alert" color="red" title={t.error}>{error}</Alert>}
      {status === 'disconnected' && <section className="disconnected-panel"><Title order={2}>{t.disconnected}</Title><Button onClick={() => void refresh()}>{t.reconnect}</Button></section>}
      <div className="inline-base" hidden={page !== null}>
      <RecoveryBanner language={language} controller={recovery} onOpen={opener => { setSettingsSection('recovery'); openPage('settings', opener); }} />
      {messageRecoveryUnavailable && <p role="status">{recoveryLabels.unavailable}</p>}
      {!selected && uncertainSessions.size > recoveredMessages.length && <p role="status">{recoveryLabels.pending}</p>}
      {!selected && recoveredMessages.map(receipt => (
        <p key={receipt.turn_id} role="status">
          <Button type="button" onClick={() => openTask(receipt.task_id)}>{recoveryLabels.received}</Button>
        </p>
      ))}
      {showingCanvas && !selected && <><header className="page-heading"><Title order={1}>{t.tasks}</Title></header><div className="filter-row" aria-label={language === 'zh' ? '任务状态筛选' : 'Task status filters'}>{(['all', 'active', 'attention', 'drafts', 'done'] as const).map(item => <Button key={item} aria-pressed={filter === item} onClick={() => setFilter(item)}>{c[item]}</Button>)}</div>{tasks.length === 0 ? <div className="canvas-empty"><Icon name="task" /><p>{t.empty}</p></div> : filtered.length === 0 ? <p className="canvas-empty">{c.noMatches}</p> : <div className="task-grid">{filtered.map(task => {
        const session = sessions.find(record => record.task_id === task.id), connection = connections.find(record => record.id === session?.connection_id);
        return <Button className="task-card" data-testid="task-card" data-task-id={task.id} key={task.id} onClick={() => openTask(task.id)} aria-label={`${c.openTask}: ${task.goal}`}><div className="card-top"><span className={`task-state state-${task.state}`}><span className="status-dot" />{stateText(task.state)}</span></div><Title order={2}>{task.goal}</Title><div className="card-footer"><span><Icon name="folder" />{projectName(task.project_id)}</span>{connection && <span>{connection.name}</span>}</div></Button>;
      })}</div>}</>}
      {showingCanvas && selected && <section className="task-thread" data-testid="task-thread" data-task-id={selected.id}><header className="page-heading">
        <div className="page-title"><Button type="button" className="icon-button" aria-label={c.back} title={c.back} onClick={closeTask}><Icon name="back" /></Button><Title order={1} ref={taskTitle} tabIndex={-1}>{selected.goal}</Title></div>
        <span className={`task-state state-${selected.state}`}><span className="status-dot" />{stateText(selected.state)}</span>
      </header>
      <Tabs value={tab} keepMountedMode="display-none" onChange={value => {
        if (value !== 'conversation' && value !== 'artifacts' && value !== 'details') return;
        setTab(value);
        if (value === 'details') setDetailsTaskId(selected.id);
      }}>
        <Tabs.List className="thread-tabs" aria-label={language === 'zh' ? '任务内容' : 'Task content'}>
          {(['conversation', 'artifacts', 'details'] as const).map(item => <Tabs.Tab key={item} value={item}
            id={'tab-' + item} aria-controls={'panel-' + item}>{c[item]}</Tabs.Tab>)}
        </Tabs.List>
        <Tabs.Panel value="conversation" id="panel-conversation" aria-labelledby="tab-conversation">
        {tab === 'conversation' && <><div className="conversation">{conversationEvents.map(event => <article className={event.kind === 'session.message_queued' ? 'user-message' : 'agent-message'} key={event.event_id}><p>{sessionEventText(event)}</p></article>)}{recoveredForTask.map(receipt => (
          <article className="user-message" key={receipt.turn_id}>
            {!displayedTurns.has(receipt.turn_id) && <p>{receipt.text}</p>}
            <p role="status">{recoveryLabels.received}</p>
            <Button type="button" onClick={() => void markMessageSeen(receipt)}>{recoveryLabels.acknowledge}</Button>
          </article>
        ))}</div>{!selectedSessions.length && <p className="thread-note">{selected.state === 'draft' ? t.noExecution : t.noAttempt}</p>}{selectedSessions.map(session => <p className="session-observation" key={session.id}>{c.sessionState}: {recordedSessionState(session, language)}</p>)}<TaskActions task={selected} language={language} busy={busy} sendBusy={sendBusy} connected={status==='connected'} canSend={!!readySession} recipient={readySession?`${selectedConnection?.name??readySession.connection_id} · ${readySession.id}`:undefined} multipleSessions={readySessions.length>1} uncertain={uncertainTaskSend} receiptReviewHint={recoveredForTask.length ? recoveryLabels.review : undefined} onNavigate={navigateTask} onSend={task=>void sendTaskText(task,task.goal)} /><TaskPlanSummary key={selected.id} task={selected} language={language} connected={status === 'connected'} /><Disclosure className="observable-events"><DisclosureLabel>{c.eventDetails}</DisclosureLabel><EventList events={taskEvents} empty={t.noEvents} /></Disclosure>{eventPager}{earliestCursor > 0 && <p className="hint">{t.moreEvents}</p>}</>}
        </Tabs.Panel>
        <Tabs.Panel value="artifacts" id="panel-artifacts" aria-labelledby="tab-artifacts">
        {tab === 'artifacts' && <><ArtifactValidation key={selected.id} task={selected} sessions={sessions} language={language} connected={status === 'connected'} /><DataContractValidation key={`data-${selected.id}`} task={selected} sessions={sessions} language={language} connected={status === 'connected'} /><CommandValidation key={`command-${selected.id}`} task={selected} sessions={sessions} language={language} connected={status === 'connected'} /></>}
        </Tabs.Panel>
        <Tabs.Panel value="details" id="panel-details" aria-labelledby="tab-details">
        {(tab === 'details'||detailsTaskId===selected.id) && <div className="task-details" hidden={tab!=='details'}><dl><dt>{t.taskId}</dt><dd>{selected.id}</dd><dt>{t.project}</dt><dd>{projectName(selected.project_id)}</dd><dt>{t.revision}</dt><dd>{selected.revision}</dd><dt>{t.updated}</dt><dd>{selected.updated_at}</dd></dl><Button onClick={() => void cancelTask(selected)} disabled={busy || status !== 'connected' || !['draft','running','outcome_unknown'].includes(selected.state)}>{t.cancel}</Button><p className="hint">{t.cancelHelp}</p><TaskCodeWorkspace key={`code-workspace-${selected.id}`} task={selected} language={language} connected={status === 'connected'} /><GoalPlanning key={`goal-${selected.id}`} navigation={intentTask?.id===selected.id&&taskIntent?.action!=='workspace'?{target:taskIntent!.action==='review'?'execution':'plan',requestId:taskIntent!.key}:undefined} task={selected} language={language} connected={status === 'connected'} /><TaskIntegration key={`integration-${selected.id}`} task={selected} language={language} connected={status === 'connected'} /><FactoryRuntime key={`factory-${selected.id}`} task={selected} language={language} connected={status === 'connected'} />
          {tab === 'details' && status !== 'disconnected' && <TaskExperienceProposal task={selected} language={language}
            project={projects.find(project => project.id === selected.project_id)} selectedProjectId={projectId} />}
        </div>}
        </Tabs.Panel>
      </Tabs></section>}
      {view === 'capabilities' && <CapabilitiesPage language={language} connected={status === 'connected'}
        projects={projects} projectId={projectId} sessions={sessions} sessionsFresh={sessionsFresh} refreshScope={() => void refresh(true)} />}
      {view === 'history' && <><div className="page-heading"><Title order={1}>{t.history}</Title></div><EventList events={events} empty={t.noEvents} focusedEventId={focusedEventId} focusRequest={focusedEventRequest} />{eventPager}{earliestCursor > 0 && <><p>{events.length >= 500 ? t.windowLimit : t.moreEvents}</p><Button onClick={() => void loadEarlier()} disabled={busy || status !== 'connected'}>{t.earlier}</Button></>}</>}
      {view === 'quick' && <><header className="page-heading"><Title order={1}>{c.quick}</Title><Button type="button" disabled={busy} onClick={event => openPage('standalone', event.currentTarget)}>{standaloneConversationMessages[language].entry}</Button></header><QuickConversation language={language} connected={status === 'connected'} busy={busy||sendBusy||messageRecoveryUnavailable} tasks={tasks} sessions={sessions} connections={connections} events={events} uncertainSessions={uncertainSessions} onUncertain={id=>setUncertainSessions(current=>new Set([...current,id]))} onBusy={changeSendBusy} refresh={()=>refresh(true)} summary={quickSummary} summaryProject={quickSummaryProject} onSummary={(text,sourceProject)=>{setQuickSummary(text);if(sourceProject!==undefined)setQuickSummaryProject(sourceProject);}} onConvert={(summary,sourceProject,opener)=>{ const targetProject=sourceProject === undefined ? projectId : sourceProject === 'local' ? '' : sourceProject; setConversionProject(targetProject); setConversionSummary(summary); createKey.current=null; openPage('conversion', opener); }} /></>}
      </div>
      {pageVisits.map(visit => <section key={visit.page} className="inline-page" data-page={visit.page} hidden={page !== visit.page}
        aria-labelledby={`inline-title-${visit.page}`}>
        <header className="page-heading">
          <div className="page-title">
            {visit.showBack && <Button type="button" className="icon-button" aria-label={c.backPage} title={c.backPage} onClick={backPage}><Icon name="back" /></Button>}
            <Title order={1} id={`inline-title-${visit.page}`} tabIndex={-1}>{pageTitles[visit.page]}</Title>
          </div>
          {visit.page === 'projects' && projectId && <Button type="button" onClick={() => { setProjectId(''); createKey.current = null; }}>{c.allProjects}</Button>}
        </header>
        <div className="inline-page-content">
          {visit.page === 'projects' && <Projects projects={projects} selectedId={projectId} onSelect={id => { setProjectId(id); createKey.current = null; }} refresh={refresh} connected={status === 'connected'} language={language} />}
          {visit.page === 'standalone' && <StandaloneConversations language={language} embedded connected={status==='connected'} onBusy={setBusy} onConvert={(summary,sourceProject,opener)=>{setConversionProject(sourceProject==='local'?'':sourceProject);setConversionSummary(summary);createKey.current=null;openPage('conversion', opener);}} />}
          {visit.page === 'execution' && (taskIntent?.action==='workspace'&&!intentTask?<p>{taskActionsMessages[language].missing}</p>:<ManagedSessions embedded sharedSendBusy={sendBusy||messageRecoveryUnavailable} onSendBusyChange={changeSendBusy} uncertainSessions={uncertainSessions} onUncertain={id=>setUncertainSessions(current=>new Set([...current,id]))} key={taskIntent?.action==='workspace'?taskIntent.taskId:'execution'} initialTaskId={taskIntent?.action==='workspace'?intentTask?.id:selected?.id} language={language} connected={status === 'connected'} projects={projects} tasks={taskIntent?.action==='workspace'&&intentTask?[intentTask]:tasks} projectId={taskIntent?.action==='workspace'?intentTask?.project_id??'':selected?.project_id ?? projectId} onProjectChange={id => { setProjectId(id); createKey.current = null; }} refreshTasks={refresh} />)}
          {visit.page === 'settings' && <SettingsPanel language={language} onLanguage={setLanguage} status={status}
            initialSection={settingsSection} recovery={recovery} hostRegistration={hostRegistration} />}
          {visit.page === 'search' && <><label className="sr-only" htmlFor="task-search">{c.searchTasks}</label><Input id="task-search" value={query} onChange={event => setQuery(event.target.value)} placeholder={c.searchPlaceholder} data-page-autofocus /><ul className="search-results">{tasks.filter(task => task.goal.toLocaleLowerCase().includes(query.toLocaleLowerCase())).map(task => <li key={task.id}><Button className="record-button" onClick={() => openTask(task.id)}><Icon name="task" /><span>{task.goal}</span><span className="task-state">{stateText(task.state)}</span></Button></li>)}</ul>{!tasks.some(task => task.goal.toLocaleLowerCase().includes(query.toLocaleLowerCase())) && <p>{c.noMatches}</p>}</>}
          {visit.page === 'conversion' && <><label htmlFor="conversion-summary">{c.summary}</label><Textarea id="conversion-summary" rows={5} value={conversionSummary} maxLength={8192} onChange={event => { setConversionSummary(event.target.value); createKey.current = null; }} /><div className="dialog-actions"><Button className="primary" disabled={busy || status !== 'connected' || !conversionSummary.trim()} onClick={() => void createTask(undefined, conversionSummary, conversionProject)}>{c.createDraft}</Button></div></>}
        </div>
      </section>)}
    </div></div>
    {showingCanvas && <div className="composer-wrap" hidden={page !== null || !showComposer}><Form className="composer" data-testid="goal-composer" onSubmit={selected ? sendTaskMessage : createTask}><label className="sr-only" htmlFor="goal">{selected ? c.message : t.goal}</label><Textarea ref={composer} data-testid="goal-input" id="goal" disabled={busy || sendBusy || (!!selected && (!readySession || uncertainTaskSend))} value={goal} maxLength={selected ? 16384 : 8192} onChange={event => { setGoal(event.target.value); createKey.current = null; }} onKeyDown={event => { if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) { event.preventDefault(); event.currentTarget.form?.requestSubmit(); } }} placeholder={selected ? c.message : t.goal} required rows={2} /><div className="composer-controls"><div className="composer-left"><label className="composer-project"><Icon name="folder" /><span className="sr-only">{t.project}</span><Select disabled={busy || !!selected} value={selected?.project_id ?? projectId} onChange={selectedValue => { setProjectId(selectedValue); createKey.current = null; }}><SelectOption value="">{t.localDrafts}</SelectOption>{projects.map(project => <SelectOption key={project.id} value={project.id} disabled={project.archived}>{project.name}{project.archived ? ` (${t.archived})` : ''}</SelectOption>)}</Select></label>{selected && <Button type="button" className="text-button execution-choice" aria-label={c.execution} title={c.execution} onClick={event => {setTaskIntent(null);openPage('execution', event.currentTarget);}}><Icon name="capabilities" /><span>{selectedConnection?.name ?? c.execution}</span></Button>}</div><Button type="submit" className="send-button primary" aria-label={selected ? c.send : t.create} disabled={busy || sendBusy || status !== 'connected' || !goal.trim() || (selected ? !readySession || uncertainTaskSend : projects.some(project => project.id === projectId && project.archived))}><Icon name="send" /></Button></div></Form>{selected && (recoveredForTask.length > 0 || uncertainTaskSend || !readySession) && <p className="composer-foot" role="status">{recoveredForTask.length ? recoveryLabels.review : uncertainTaskSend ? c.unsent : c.sessionRequired}</p>}</div>}
    </AppShell.Main>
  </AppShell>;
}
function EventList({ events, empty, focusedEventId, focusRequest = 0 }: { events: JournalEvent[]; empty: string; focusedEventId?: string; focusRequest?: number }) {
  useEffect(() => { if (focusedEventId) document.getElementById(`journal-event-${focusedEventId}`)?.scrollIntoView?.({ block: 'nearest' }); }, [focusedEventId, focusRequest]);
  return <div className="events">{events.length ? <ol>{events.map(event => <li id={`journal-event-${event.event_id}`} key={event.event_id}><Disclosure open={event.event_id === focusedEventId || undefined}><DisclosureLabel><span>{event.kind}</span> <time>{event.occurred_at}</time> <span>#{event.sequence}</span></DisclosureLabel><Code block>{JSON.stringify(event.payload, null, 2)}</Code></Disclosure></li>)}</ol> : <p>{empty}</p>}</div>;
}
