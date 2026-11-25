import { useCallback, useEffect, useRef, useState } from 'react';
import { api, DaemonError, acknowledgeTaskMessage, taskAllowsExecution, type JournalEvent, type Task, type Project, type TaskMessageReceipt } from './api';
import { initialLanguage, messages, type Language } from './i18n';
import { canvasMessages } from './canvas-i18n';
import { Projects } from './Projects';
import { TaskCodeWorkspace } from './TaskCodeWorkspace';
import { TaskExperienceProposal } from './TaskExperienceProposal';
import { Connections } from './Connections';
import { ManagedSkills } from './ManagedSkills';
import { ManagedMcps } from './ManagedMcps';
import { BackgroundService } from './BackgroundService';
import {RecoveryCenter,RecoveryBanner,useRecoveryCenter} from './RecoveryCenter';
import { ManagedSessions, sessionEventText } from './ManagedSessions';
import { sessionApi, type ManagedSession, type NamedConnection } from './session-api';
import { sessionMessages } from './session-i18n';
import { ContextDialog } from './ContextDialog';
import { BackgroundServiceSettings } from './BackgroundServiceSettings';
import { Hosts, useHostRegistrationSession } from './Hosts';
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
import { groupTranscript, taskMessageText, useTaskThreadHistory } from './task-thread-history';
import { goalGraphApi } from './goal-graph-api';
import { graphDetailsShape, graphSummaryMeetsFloor, graphSummaryShape } from './goal-graph-guards';
import { goalExecutionApi, goalRunShape } from './goal-execution-api';
import { goalApi } from './goal-api';

type View = 'work' | 'tasks' | 'capabilities' | 'history' | 'quick';
type Filter = 'all' | 'active' | 'attention' | 'drafts' | 'done';
type Tab = 'conversation' | 'artifacts' | 'details';
type GoalRunTarget = { scopeId: string; runId: string; graph: boolean };
type Dialog = 'projects' | 'execution' | 'search' | 'settings' | 'conversion' | 'standalone' | null;
const activeStates = new Set(['queued', 'running', 'starting', 'ready']);
const attentionStates = new Set(['outcome_unknown', 'unknown', 'cancel_requested', 'failed', 'awaiting_approval', 'awaiting_acceptance', 'blocked']);
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
  const [capability, setCapability] = useState<'connections' | 'skills' | 'mcp'>('connections');
  const [settingsSection, setSettingsSection] = useState<'general' | 'background' | 'recovery' | 'hosts'>('general');
  const recovery=useRecoveryCenter(dialog==='settings');
  const hostRegistration = useHostRegistrationSession();
  const [taskIntent,setTaskIntent] = useState<{taskId:string;projectId:string|undefined;action:TaskAction;key:string;goalRun?:GoalRunTarget}|null>(null);
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
  const [conversionSource,setConversionSource] = useState<string>();
  const [recoveredMessages, setRecoveredMessages] = useState<TaskMessageReceipt[]>([]);
  const [messageRecoveryUnavailable, setMessageRecoveryUnavailable] = useState(false);
  const [uncertainSessions, setUncertainSessions] = useState<Set<string>>(() => new Set());
  const createKey = useRef<string | null>(null);
  const messageKeys = useRef(new Map<string, string>());
  const refreshSequence = useRef(0);
  const composer = useRef<HTMLTextAreaElement>(null);
  const sessionDetails = useRef<HTMLDetailsElement>(null);
  const selected = tasks.find(task => task.id === selectedId);
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
  const stateText = (state: string) => Object.hasOwn(c.phases, state) ? c.phases[state as keyof typeof c.phases]
    : Object.hasOwn(t, state) ? t[state as keyof typeof t] : t.unknown;
  const connectionById = new Map(connections.map(record => [record.id, record]));
  const projectById = new Map(projects.map(project => [project.id, project]));
  const sessionByTask = new Map<string, ManagedSession>();
  const outcomesUnknown = new Set<string>();
  for (const session of sessions) {
    if (session.state === 'outcome_unknown' || session.turn_state === 'outcome_unknown') outcomesUnknown.add(session.task_id);
    const prior = sessionByTask.get(session.task_id);
    if (!prior || session.updated_at > prior.updated_at) sessionByTask.set(session.task_id, session);
  }
  const observedState = (task: Task) => task.state === 'running' && outcomesUnknown.has(task.id) ? 'outcome_unknown' : task.state;
  const recentSession = selected ? sessionByTask.get(selected.id) : undefined;
  const selectedConnection = connectionById.get(readySession?.connection_id ?? recentSession?.connection_id ?? '');
  const sessionRevision = selectedSessions.map(session => `${session.id}:${session.revision}:${session.turn_state}`).join('|');
  const history = useTaskThreadHistory(selectedId, status === 'connected', sessionRevision);
  const [goalPhase, setGoalPhase] = useState<{ taskId: string; state: string; target: GoalRunTarget; artifacts?: string[]; blocker?: string; expired?: boolean }>();
  const selectedTask = useRef(selected);
  selectedTask.current = selected;
  const runningTaskId = selected?.state === 'running' ? selected.id : undefined;
  useEffect(() => {
    if (!runningTaskId || status !== 'connected') return;
    let active = true;
    void Promise.allSettled([goalGraphApi.list(runningTaskId), goalExecutionApi.list(runningTaskId)]).then(async ([graphs, legacy]) => {
      const task = selectedTask.current;
      if (!active || task?.id !== runningTaskId) return;
      const graphRuns = graphs.status === 'fulfilled' && Array.isArray(graphs.value) ? graphs.value.filter(row => graphSummaryShape(row, task)) : [];
      const currentRuns = [
        ...graphRuns.map(row => row.run),
        ...(legacy.status === 'fulfilled' && Array.isArray(legacy.value) ? legacy.value.filter(row => goalRunShape(row, task)) : []),
      ];
      const current = currentRuns.sort((a,b) => b.created_at.localeCompare(a.created_at))[0];
      if (!current || ['cancelled','accepted'].includes(current.state)) { setGoalPhase(undefined); return; }
      const summary = graphRuns.find(row => row.run.id === current.id);
      const target = { scopeId: current.scope_id, runId: current.id, graph: !!summary };
      setGoalPhase({ taskId: task.id, state: current.state, target });
      if (!summary || !['awaiting_acceptance','blocked'].includes(summary.run.state)) return;
      try {
        const [detail, scopes] = await Promise.all([goalGraphApi.get(summary.run.id), goalApi.scopesList(task.id)]);
        const scope = Array.isArray(scopes) ? scopes.find(row => row.id === summary.run.scope_id && row.task_id === task.id && row.project_id === task.project_id) : undefined;
        if (!active || !scope || detail?.run.id !== summary.run.id || detail.run.state !== summary.run.state
          || !graphDetailsShape(detail, task, scope) || !graphSummaryMeetsFloor(detail, summary)) return;
        const artifacts = [...new Set(detail.candidates.filter(item => item.plan_revision === detail.run.plan_revision)
          .flatMap(item => item.artifacts.map(artifact => artifact.relative_path)))].slice(0, 6);
        const blocker = detail.nodes.find(item => item.plan_revision === detail.run.plan_revision && item.blocked_reason)?.blocked_reason ?? undefined;
        setGoalPhase({ taskId: task.id, state: detail.run.state, target, artifacts, blocker, expired: scope.expires_at_ms <= Date.now() });
      } catch { /* Keep the verified run state when its optional detail is unavailable. */ }
    });
    return () => { active = false; };
  }, [runningTaskId, status, eventSequence]);
  const runBrief = runningTaskId && goalPhase?.taskId === runningTaskId ? goalPhase : undefined;
  const taskPhase = selected && (observedState(selected) === 'outcome_unknown' ? 'outcome_unknown' : runBrief?.state ?? selected.state);
  const needsRecovery = taskPhase === 'outcome_unknown' || uncertainTaskSend;
  const canCompose = !!readySession && !needsRecovery && (taskPhase === 'draft' || taskPhase === 'running');
  function openSettings(section: typeof settingsSection = 'general') {
    setSettingsSection(section);
    setDialog('settings');
  }
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
      if (dialog || document.querySelector('[role="dialog"][aria-modal="true"]')) return;
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
  async function createTask(event?: React.SubmitEvent<HTMLFormElement>, summary?: string, projectOverride?: string) {
    event?.preventDefault();
    const text = (summary ?? goal).trim();
    if (!text || busy || status !== 'connected') return;
    setBusy(true); setError(''); createKey.current ??= crypto.randomUUID();
    try {
      const targetProject = projectOverride ?? projectId;
      const task = targetProject ? await api.create(text, createKey.current, targetProject) : await api.create(text, createKey.current);
      createKey.current = null; setGoal(''); if (summary !== undefined) { setQuickSummary(''); setQuickSummaryProject(undefined); }
      setSelectedId(task.id); setView('tasks'); setTab('conversation'); setDialog(null);
      await refresh();
    } catch (e) { showError(e); }
    finally { setBusy(false); }
  }
  async function sendTaskText(observedTask:Task,text:string,clearComposer=false) {
    const observed=readySession;
    if (!canCompose || !observed || !Number.isSafeInteger(observed.revision) || observed.revision<1 || currentTask.current?.id!==observedTask.id || !text.trim() || busy || sendBusyRef.current || sendLock.current || status!=='connected' || uncertainTaskSend) return;
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
  function navigateTask(task:Task,action:TaskAction) {
    if(busy||selected?.id!==task.id||selected.project_id!==task.project_id)return;
    setTaskIntent({taskId:task.id,projectId:task.project_id,action,key:crypto.randomUUID(),goalRun:action==='review'?runBrief?.target:undefined});
    if(action==='workspace')setDialog('execution');
    else {setDetailsTaskId(task.id);setTab('details');setDialog(null);}
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
  function openTask(id: string) { eventRequestCursor.current = undefined; setSelectedId(id); setGoal(''); setTab('conversation'); setView('tasks'); setDialog(null); setMobileNav(false); createKey.current = null; }
  function viewNotificationEvent(eventId: string) { setFocusedEventId(eventId); setFocusedEventRequest(current => current + 1); setView('history'); setSelectedId(undefined); setDialog(null); setMobileNav(false); }
  function navigate(next: View) { if (next !== 'history') eventRequestCursor.current = undefined; setView(next); setSelectedId(undefined); setMobileNav(false); setGoal(''); createKey.current = null; }
  const filtered = tasks.filter(task => {
    const state = observedState(task);
    return (!projectId || task.project_id === projectId) && (filter === 'all'
      || (filter === 'active' ? activeStates.has(state) : filter === 'attention' ? attentionStates.has(state) : filter === 'drafts' ? state === 'draft' : finishedStates.has(state)));
  });
  const taskEvents = selected ? events.filter(event => event.aggregate_id === selected.id || selectedSessions.some(session => session.id === event.aggregate_id)) : [];
  const conversationEvents = history.events;
  const transcript = groupTranscript(conversationEvents, taskMessageText, 'session.message_queued');
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
  const showingCanvas = view === 'work' || view === 'tasks';
  const nav: [Exclude<View, 'quick'>, IconName, string][] = [['work', 'grid', c.space], ['tasks', 'task', t.tasks], ['capabilities', 'capabilities', t.capabilities], ['history', 'history', t.history]];
  const projectName = (id: string | undefined) => (id ? projectById.get(id)?.name : undefined) ?? t.localDrafts;
  const intentTask=taskIntent&&selectedId===taskIntent.taskId?tasks.find(task=>task.id===taskIntent.taskId&&task.project_id===taskIntent.projectId):undefined;
  const dialogTitle = dialog === 'standalone' ? standaloneConversationMessages[language].title : dialog === 'projects' ? c.projectSettings : dialog === 'execution' ? c.execution : dialog === 'search' ? c.searchTasks : dialog === 'conversion' ? c.convertTitle : c.settings;
  const matches = query.trim() ? tasks.filter(task => task.goal.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())) : tasks;
  const eventPager = <>{windowReplacement && <p role="status">{windowReplacement === 'bytes' ? t.windowBudget : t.windowLimit}</p>}{byteLimitedPage && <p role="status">{t.bytePageLimit}</p>}{eventPageMore && <div className="event-page-controls"><p className="hint">{t.eventPageMore}</p><button onClick={() => void loadNextEvents()} disabled={busy || status !== 'connected'}>{t.nextEvents}</button></div>}</>;
  return <div className={`shell${navCollapsed ? ' nav-collapsed' : ''}${mobileNav ? ' mobile-nav-open' : ''}`}>
    <header className="appbar"><div className="brand"><svg className="brandmark" viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M3 19 11 4h5L8 19Zm9 1 7-13h4l-7 13Z" /></svg><span>AgentBaton</span></div><button className="icon-button nav-toggle" aria-label={c.toggleNav} aria-expanded={mobileLayout ? mobileNav : !navCollapsed} onClick={() => { if (mobileLayout) setMobileNav(value => !value); else setNavCollapsed(value => !value); }}><Icon name="panel" /></button><div className="location" /><span className={`connection task-notifications-connection ${status}`} role="status"><span className="status-dot" />{status === 'disconnected' ? c.offline : t[status]}</span><TaskNotifications tasks={tasks} sessions={sessions} events={events} language={language} observation={{ connected: status === 'connected', tasksFresh, sessionsFresh, eventsFresh, eventSequence, eventLimited: eventGapObserved || earliestCursor > 0 || eventPageMore || byteLimitedPage || windowReplacement !== null }} onOpenTask={openTask} onViewEvent={viewNotificationEvent} />{status !== 'disconnected' && <button className="icon-button" aria-label={t.refresh} disabled={busy} onClick={() => void refresh()}><Icon name="refresh" /></button>}</header>
    {mobileNav && <button className="mobile-nav-dismiss" aria-label={c.close} onClick={() => setMobileNav(false)} />}
    <aside className="sidebar"><div className="sidebar-top"><button className="new-button" aria-label={c.newTask} title={`${c.newTask} (⌘N)`} onClick={newTask}><Icon name="plus" /><span>{c.newTask}</span></button><button className="nav-item" aria-label={c.search} title={`${c.search} (⌘K)`} onClick={() => { setQuery(''); setDialog('search'); }}><Icon name="search" /><span>{c.search}</span></button></div><nav aria-label={language === 'zh' ? '主导航' : 'Main navigation'}>{nav.map(([item, icon, label]) => <button className="nav-item" key={item} aria-current={view === item ? 'page' : undefined} onClick={() => navigate(item)} title={label}><Icon name={icon} /><span>{label}</span></button>)}</nav><div className="sidebar-projects"><div className="sidebar-label">{t.projects}</div><button className="nav-item" onClick={() => setDialog('projects')} title={c.projectSettings}><Icon name="plus" /><span>{c.projectSettings}</span></button>{projects.filter(project => !project.archived).map(project => <button className="nav-item" key={project.id} aria-pressed={projectId === project.id} onClick={() => { setProjectId(current => current === project.id ? '' : project.id); navigate('work'); }} title={project.name}><Icon name="folder" /><span>{project.name}</span></button>)}</div><div className="sidebar-bottom"><button className="nav-item" aria-current={view === 'quick' ? 'page' : undefined} onClick={() => navigate('quick')} title={c.quick}><Icon name="chat" /><span>{c.quick}</span></button><button className="nav-item" onClick={() => openSettings()} title={c.settings}><Icon name="settings" /><span>{c.settings}</span></button></div></aside>
    <main className="main"><div className={`page-scroll${showingCanvas ? ' has-composer' : ''}`}><div className="canvas">
      <RecoveryBanner language={language} controller={recovery} onOpen={() => openSettings('recovery')} />
      {messageRecoveryUnavailable && <p role="status">{recoveryLabels.unavailable}</p>}
      {!selected && uncertainSessions.size > recoveredMessages.length && <p role="status">{recoveryLabels.pending}</p>}
      {!selected && recoveredMessages.map(receipt => (
        <p key={receipt.turn_id} role="status">
          <button type="button" onClick={() => openTask(receipt.task_id)}>{recoveryLabels.received}</button>
        </p>
      ))}
      {status === 'disconnected' ? <section className="disconnected-panel">
        <button type="button" className="primary" onClick={() => void refresh()}>{t.reconnect}</button>
        {tasks.length > 0 && <span>{c.cachedTasks}</span>}
        {error && <details><summary>{c.connectedHelp}</summary><pre>{error}</pre></details>}
      </section> : error && <p className="error" role="alert">{error}</p>}
      {showingCanvas && !selected && <>
        <div className="page-heading"><h1>{projectId ? projectName(projectId) : c.yourTasks}</h1></div>
        {(tasks.length > 0 || status === 'connected') && <div className="filter-row" aria-label={c.filters}>
          {(['all', 'active', 'attention', 'drafts', 'done'] as const).map(item =>
            <button key={item} aria-pressed={filter === item} onClick={() => setFilter(item)}>{c[item]}</button>)}
        </div>}
        {filtered.length > 0 ? <div className="task-grid">{filtered.map(task => {
          const session = sessionByTask.get(task.id);
          const state = observedState(task);
          const agent = session ? connectionById.get(session.connection_id)?.name : undefined;
          const progress = session?.turn_state === 'completed' && state === 'running' ? [agent, c.replyRecorded].filter(Boolean).join(' · ') : undefined;
          return <button className="task-card" data-testid="task-card" data-task-id={task.id} key={task.id} onClick={() => openTask(task.id)} aria-label={`${c.openTask}: ${task.goal}`}>
            <div className="card-top"><span className={`task-state state-${state}`}><span className="status-dot" />{stateText(state)}</span></div>
            <h2>{task.goal}</h2>
            {progress && <p className="card-footer">{progress}</p>}
          </button>;
        })}</div> : status === 'connected' && <p className="canvas-empty">
          {filter === 'all' ? projectId ? c.emptyProject : c.emptyTasks : c.noMatches}
          {filter !== 'all' && <button type="button" className="text-button" onClick={() => setFilter('all')}>{c.all}</button>}
        </p>}
      </>}
      {showingCanvas && selected && <section className="task-thread" data-testid="task-thread" data-task-id={selected.id}><div className="thread-back"><button className="text-button" onClick={() => setSelectedId(undefined)}><Icon name="back" />{c.back}</button></div><div className="thread-heading"><h1>{selected.goal}</h1><span className={`task-state state-${taskPhase}`}><span className="status-dot" />{stateText(taskPhase!)}</span></div><div className="thread-tabs" role="tablist" aria-label={language === 'zh' ? '任务内容' : 'Task content'}>{(['conversation', 'artifacts', 'details'] as const).map(item => <button key={item} role="tab" aria-selected={tab === item} tabIndex={tab === item ? 0 : -1} id={`tab-${item}`} aria-controls={`panel-${item}`} onKeyDown={event => {
        const order: Tab[] = ['conversation', 'artifacts', 'details'];
        const index = order.indexOf(item);
        const next = event.key === 'ArrowRight' ? order[(index + 1) % order.length] : event.key === 'ArrowLeft' ? order[(index + order.length - 1) % order.length] : event.key === 'Home' ? order[0] : event.key === 'End' ? order[order.length - 1] : null;
        if (next) { event.preventDefault(); setTab(next); if(next==='details')setDetailsTaskId(selected.id); document.getElementById(`tab-${next}`)?.focus(); }
      }} onClick={() => {setTab(item);if(item==='details')setDetailsTaskId(selected.id);}}>{c[item]}</button>)}</div><div role="tabpanel" id={`panel-${tab}`} aria-labelledby={`tab-${tab}`}>
        {tab === 'conversation' && <>
          {selected.state === 'draft' && !selectedSessions.length ? <button className="primary" disabled={busy || status !== 'connected'} onClick={() => navigateTask(selected, 'workspace')}>{c.configure}</button> : <>
            {taskPhase === 'awaiting_acceptance' || taskPhase === 'awaiting_approval' ? <div className="task-next">
              <div><p>{runBrief?.expired ? c.reviewExpired : c.reviewReason}</p>
                {!!runBrief?.artifacts?.length && <ul className="task-artifacts-brief">{runBrief.artifacts.map(path => <li key={path}><code>{path}</code></li>)}</ul>}
              </div><button className="primary" onClick={() => navigateTask(selected, 'review')}>{c.review}</button>
            </div> : taskPhase === 'blocked' ? <div className="task-next">
              <p>{runBrief?.blocker ?? c.blockedReason}</p><button onClick={() => navigateTask(selected, 'review')}>{c.viewEvidence}</button>
            </div> : needsRecovery ? <div className="task-next">
              <p>{recoveredForTask.length ? recoveryLabels.review : recentSession?.last_error === 'daemon_restart_requires_reconciliation' ? c.taskRestartUnknown : c.taskSessionUnknown}</p>
              <button type="button" onClick={() => {
                void refresh(true);
                if (sessionDetails.current) {
                  sessionDetails.current.open = true;
                  sessionDetails.current.querySelector('summary')?.focus();
                } else if (selected.project_id) navigateTask(selected, 'review');
              }}>{c.reconcile}</button>
            </div> : null}
            {selectedConnection && <p className="thread-agent">{selectedConnection.name}</p>}
            <div className="conversation" aria-busy={history.loading}>
              {history.before !== null && <button className="text-button" type="button" disabled={history.loading || status !== 'connected'} onClick={history.loadEarlier}>{c.olderMessages}</button>}
              {history.detached && <button type="button" disabled={history.loading || status !== 'connected'} onClick={history.loadLatest}>{c.latestMessages}</button>}
              {history.moreRecent && <button type="button" disabled={status !== 'connected'} onClick={history.loadRecent}>{c.newerMessages}</button>}
              {history.error && <div role="alert" className="task-next"><p>{c.historyError}</p><button type="button" disabled={status !== 'connected'} onClick={history.retry}>{t.refresh}</button></div>}
              {history.loading && <p role="status">{c.loadingMessages}</p>}
              {history.replaced && <p role="status">{c.historyWindow}</p>}
              {history.ready && !transcript.length && !recoveredForTask.length && <p>{c.noMessages}</p>}
              {transcript.map(({first, user, text}, index) => !(index === 0 && user && history.before === null && text === selected.goal) &&
                <article className={user ? 'user-message' : 'agent-message'} key={first.event_id}><p>{text}</p></article>)}
              {recoveredForTask.map(receipt => <article className="user-message" key={receipt.turn_id}>
                {!displayedTurns.has(receipt.turn_id) && <p>{receipt.text}</p>}
                <p role="status">{recoveryLabels.received}</p>
                <button type="button" onClick={() => void markMessageSeen(receipt)}>{recoveryLabels.acknowledge}</button>
              </article>)}
            </div>
            {!!selectedSessions.length && <details className="observable-events" ref={sessionDetails}><summary>{c.eventDetails}</summary>
              {selectedSessions.map(session => <p key={session.id}>{recordedSessionState(session, language)} · <code>{session.id}</code>{session.last_error && <span> · {session.last_error}</span>}</p>)}
              {conversationEvents.filter(event => taskMessageText(event) !== sessionEventText(event)).map(event => event.kind === 'session.message_queued' && <pre key={event.event_id}>{sessionEventText(event)}</pre>)}
              <EventList events={taskEvents} empty={t.noEvents} language={language} taskName={() => selected.goal} />
              <button className="text-button" onClick={() => navigate('history')}>{t.history}</button>
            </details>}
            {(selectedSessions.length > 0 || selected.state !== 'draft') && <details className="secondary-section">
              <summary>{c.taskActions}</summary>
              <TaskActions task={selected} language={language} busy={busy} sendBusy={sendBusy} connected={status==='connected'} canSend={canCompose} recipient={readySession ? `${selectedConnection?.name ?? readySession.connection_id} · ${readySession.id}` : undefined} multipleSessions={readySessions.length>1} uncertain={needsRecovery} onNavigate={navigateTask} onSend={task=>void sendTaskText(task,task.goal)} />
            </details>}
          </>}
        </>}
        {tab === 'artifacts' && <>
          <ArtifactValidation key={selected.id} task={selected} sessions={sessions} language={language} connected={status === 'connected'} />
          <details className="secondary-section"><summary>{c.advancedValidation}</summary>
            <DataContractValidation key={`data-${selected.id}`} task={selected} sessions={sessions} language={language} connected={status === 'connected'} />
            <CommandValidation key={`command-${selected.id}`} task={selected} sessions={sessions} language={language} connected={status === 'connected'} />
          </details>
          {selected.state === 'accepted' && status === 'connected' && <TaskExperienceProposal task={selected} language={language} project={projectById.get(selected.project_id ?? '')} selectedProjectId={projectId} />}
        </>}
        {(tab === 'details'||detailsTaskId===selected.id) && <div className="task-details" hidden={tab!=='details'}>
          <div className="task-facts"><span>{projectName(selected.project_id)}</span>{selectedConnection && <span>{selectedConnection.name}</span>}</div>
          {taskIntent?.action !== 'review' && <TaskCodeWorkspace key={`code-workspace-${selected.id}`} task={selected} language={language} connected={status === 'connected'} />}
          <GoalPlanning key={`goal-${selected.id}`} navigation={intentTask?.id===selected.id&&taskIntent?.action!=='workspace'?{target:taskIntent!.action==='review'?'execution':'plan',requestId:taskIntent!.key, goalRun:taskIntent!.goalRun}:undefined} task={selected} language={language} connected={status === 'connected'} />
          {taskIntent?.action === 'review' && <TaskCodeWorkspace key={`review-workspace-${selected.id}`} task={selected} language={language} connected={status === 'connected'} />}
          <TaskPlanSummary key={selected.id} task={selected} language={language} connected={status === 'connected'} />
          {selected.project_id && <details className="secondary-section"><summary>{c.deliveryTools}</summary>
            <TaskIntegration key={`integration-${selected.id}`} task={selected} language={language} connected={status === 'connected'} />
            <FactoryRuntime key={`factory-${selected.id}`} task={selected} language={language} connected={status === 'connected'} />
          </details>}
          {['draft','running','outcome_unknown'].includes(selected.state) && <button disabled={busy || status !== 'connected'} title={t.cancelHelp} onClick={() => void cancelTask(selected)}>{t.cancel}</button>}
          <details className="secondary-section"><summary>{c.advancedDetails}</summary>
            <dl><dt>{t.taskId}</dt><dd><code>{selected.id}</code></dd><dt>{t.revision}</dt><dd>{selected.revision}</dd><dt>{c.sessionState}</dt><dd><code>{selected.state}</code></dd><dt>{t.updated}</dt><dd>{selected.updated_at}</dd></dl>
          </details>
        </div>}
      </div></section>}
      {view === 'capabilities' && <>
        <div className="page-heading"><h1>{t.capabilities}</h1></div>
        <div className="filter-row" aria-label={t.capabilities}>
          {(['connections','skills','mcp'] as const).map(section => <button key={section} aria-pressed={capability === section} onClick={() => setCapability(section)}>{c.sections[section]}</button>)}
        </div>
        {capability === 'connections' && <Connections language={language} connected={status === 'connected'} compact />}
        {capability === 'skills' && <ManagedSkills language={language} connected={status === 'connected'} />}
        {capability === 'mcp' && <ManagedMcps language={language} connected={status === 'connected'} />}
      </>}
      {view === 'history' && <>
        <div className="page-heading"><h1>{t.history}</h1></div>
        <EventList events={events} language={language} empty={t.noEvents} focusedEventId={focusedEventId} focusRequest={focusedEventRequest}
          taskName={id => tasks.find(task => task.id === id || sessions.some(session => session.id === id && session.task_id === task.id))?.goal} />
        {eventPager}
        {earliestCursor > 0 && <button onClick={() => void loadEarlier()} disabled={busy || status !== 'connected'}>{t.earlier}</button>}
      </>}
      {view === 'quick' && <><button type="button" disabled={busy} onClick={()=>setDialog('standalone')}>{standaloneConversationMessages[language].entry}</button><QuickConversation language={language} connected={status === 'connected'} busy={busy||sendBusy||messageRecoveryUnavailable} tasks={tasks} sessions={sessions} connections={connections} uncertainSessions={uncertainSessions} onUncertain={id=>setUncertainSessions(current=>new Set([...current,id]))} onBusy={changeSendBusy} refresh={()=>refresh(true)} summary={quickSummary} summaryProject={quickSummaryProject} onSummary={(text,sourceProject)=>{setQuickSummary(text);if(sourceProject!==undefined)setQuickSummaryProject(sourceProject);}} onConvert={(summary,sourceProject)=>{ const targetProject=sourceProject === undefined ? projectId : sourceProject === 'local' ? '' : sourceProject; setConversionProject(targetProject); setConversionSource(sourceProject); setConversionSummary(summary); createKey.current=null; setDialog('conversion'); }} /></>}
    </div></div>
    {showingCanvas && status === 'connected' && (!selected || tab === 'conversation' && (canCompose || !!goal && needsRecovery)) && <div className="composer-wrap">
      <form className="composer" data-testid="goal-composer" onSubmit={selected ? sendTaskMessage : createTask}>
        <label className="sr-only" htmlFor="goal">{selected ? c.message : t.goal}</label>
        <textarea ref={composer} data-testid="goal-input" id="goal" disabled={busy || sendBusy || (!!selected && !canCompose)} value={goal} maxLength={selected ? 16384 : 8192}
          onChange={event => { setGoal(event.target.value); createKey.current = null; }}
          onKeyDown={event => { if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) { event.preventDefault(); event.currentTarget.form?.requestSubmit(); } }}
          placeholder={selected ? c.messagePlaceholder : c.goalPlaceholder} required rows={2} />
        <div className="composer-controls">
          {!selected && <label className="composer-project"><Icon name="folder" /><span className="sr-only">{t.project}</span>
            <select disabled={busy} value={projectId} onChange={event => { setProjectId(event.target.value); createKey.current = null; }}>
              <option value="">{t.localDrafts}</option>
              {projects.map(project => <option key={project.id} value={project.id} disabled={project.archived}>{project.name}{project.archived ? ` (${t.archived})` : ''}</option>)}
            </select>
          </label>}
          {selected && <span className="sr-only">{selectedConnection?.name}</span>}
          <button type="submit" className="send-button primary" aria-label={selected ? c.send : t.create} title={`${selected ? c.send : t.create} (Enter)`}
            disabled={busy || sendBusy || !goal.trim() || (!!selected && !canCompose) || (!selected && !!projectById.get(projectId)?.archived)}><Icon name="send" /></button>
        </div>
      </form>
    </div>}
    </main>
    {dialog && <ContextDialog title={dialogTitle} closeLabel={c.close} onClose={() => setDialog(null)} wide={dialog === 'projects' || dialog === 'execution' || dialog === 'standalone'}>
      {dialog === 'projects' && <Projects projects={projects} selectedId={projectId} onSelect={id => { setProjectId(id); createKey.current = null; }} refresh={refresh} connected={status === 'connected'} language={language} />}
      {dialog === 'standalone' && <StandaloneConversations language={language} connected={status==='connected'} onBusy={setBusy} onConvert={(summary,sourceProject)=>{setConversionProject(sourceProject==='local'?'':sourceProject);setConversionSource(sourceProject);setConversionSummary(summary);createKey.current=null;setDialog('conversion');}} />}
      {dialog === 'execution' && (taskIntent?.action==='workspace'&&!intentTask?<p>{taskActionsMessages[language].missing}</p>:<ManagedSessions fixedTaskContext={taskIntent?.action === 'workspace' && !!intentTask?.project_id && intentTask.project_id !== 'local'} sharedSendBusy={sendBusy||messageRecoveryUnavailable} onSendBusyChange={changeSendBusy} uncertainSessions={uncertainSessions} onUncertain={id=>setUncertainSessions(current=>new Set([...current,id]))} key={taskIntent?.action==='workspace'?taskIntent.taskId:'execution'} initialTaskId={taskIntent?.action==='workspace'?intentTask?.id:selected?.id} language={language} connected={status === 'connected'} projects={projects} tasks={taskIntent?.action==='workspace'&&intentTask?[intentTask]:tasks} projectId={taskIntent?.action==='workspace'?intentTask?.project_id??'':selected?.project_id ?? projectId} onProjectChange={id => { setProjectId(id); createKey.current = null; }} refreshTasks={refresh} />)}
      {dialog === 'settings' && <>
        <div className="filter-row" aria-label={c.settings}>{(['general','background','recovery','hosts'] as const).map(section =>
          <button key={section} type="button" aria-pressed={settingsSection === section} onClick={() => setSettingsSection(section)}>{c.sections[section]}</button>)}</div>
        {settingsSection === 'general' && <dl className="settings-list">
          <dt>{c.appearance}</dt><dd>{c.light}</dd>
          <dt><label htmlFor="ui-language">{t.language}</label></dt>
          <dd><select id="ui-language" value={language} onChange={event => setLanguage(event.target.value as Language)}><option value="en">English</option><option value="zh">简体中文</option></select></dd>
          <dt>{c.sections.background}</dt><dd>{status === 'disconnected' ? c.offline : t[status]}</dd>
        </dl>}
        {settingsSection === 'background' && <><BackgroundService language={language} connected={status === 'connected'} /><BackgroundServiceSettings language={language} /></>}
        {settingsSection === 'recovery' && <RecoveryCenter language={language} controller={recovery} />}
        {settingsSection === 'hosts' && <Hosts inline language={language} daemonConnected={status === 'connected'} session={hostRegistration} />}
      </>}
      {dialog === 'search' && <>
        <label className="sr-only" htmlFor="task-search">{c.searchTasks}</label>
        <input id="task-search" value={query} onChange={event => setQuery(event.target.value)} placeholder={c.searchPlaceholder} data-dialog-autofocus onKeyDown={event => {
          if (event.key === 'Enter' && !event.nativeEvent.isComposing && matches[0]) { event.preventDefault(); openTask(matches[0].id); }
        }} />
        {matches.length ? <ul className="search-results">{matches.map(task => <li key={task.id}>
          <button onClick={() => openTask(task.id)} title={task.goal}><span>{task.goal}</span><small>{projectName(task.project_id)}</small><span className="task-state">{stateText(observedState(task))}</span></button>
        </li>)}</ul> : <p>{c.noMatches}</p>}
      </>}
      {dialog === 'conversion' && <>{error && <div className="error" role="alert"><strong>{t.error}</strong><p>{error}</p></div>}<label htmlFor="conversion-summary">{c.summary}</label><textarea id="conversion-summary" rows={5} value={conversionSummary} maxLength={8192} onChange={event => { setConversionSummary(event.target.value); createKey.current = null; }} /><label>{t.project}<select value={conversionProject ?? ''} onChange={event => { setConversionProject(event.target.value); createKey.current = null; }}>
        <option value="">{t.localDrafts}</option>{projects.filter(project => !project.archived || project.id === conversionProject).map(project => <option key={project.id} value={project.id} disabled={project.archived}>{project.name}</option>)}
      </select></label>
      {conversionSource !== undefined && (conversionSource === 'local' ? '' : conversionSource) !== conversionProject &&
        <p>{c.convertSource} · {projectName(conversionSource)}. {c.convertCrossProject}</p>}
      <button className="primary" disabled={busy || status !== 'connected' || !conversionSummary.trim() || !!projectById.get(conversionProject ?? '')?.archived} onClick={() => void createTask(undefined, conversionSummary, conversionProject)}>{c.createDraft}</button></>}
    </ContextDialog>}
  </div>;
}
const eventLabels: Record<string, [string, string]> = {
  'task.created': ['Task created', '已创建任务'],
  'task.cancel_requested': ['Stop requested', '已请求停止'],
  'task.cancelled': ['Stop confirmed', '已确认停止'],
  'task.accepted': ['Result accepted', '结果已验收'],
  'project.imported': ['Project imported', '已导入项目'],
  'approval.confirmed': ['Approval recorded', '已记录审批'],
  'session.message_queued': ['Message queued', '消息已入队'],
  'conversation.message_queued': ['Message queued', '消息已入队'],
  'session.update': ['Agent activity', 'Agent 活动'],
  'conversation.update': ['Agent activity', 'Agent 活动'],
  'session.turn_ended': ['Turn ended', '回合已结束'],
};
function EventList({ events, empty, language, taskName, focusedEventId, focusRequest = 0 }: {
  events: JournalEvent[]; empty: string; language: Language; taskName: (id: string) => string | undefined;
  focusedEventId?: string; focusRequest?: number;
}) {
  const list = useRef<HTMLUListElement>(null);
  const lastFocusRequest = useRef<number>(0);
  useEffect(() => {
    if (!focusedEventId || lastFocusRequest.current === focusRequest) return;
    const entry = document.getElementById(`journal-event-${focusedEventId}`);
    if (!entry || !list.current?.contains(entry)) return;
    lastFocusRequest.current = focusRequest;
    const disclosure = entry.closest('details');
    if (disclosure) disclosure.open = true;
    entry.scrollIntoView?.({ block: 'nearest' });
  }, [focusedEventId, focusRequest, events]);
  const zh = language === 'zh';
  const time = new Intl.DateTimeFormat(language, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' });
  const groups: JournalEvent[][] = [];
  for (const event of events) {
    const previous = groups.at(-1);
    if (event.kind.endsWith('.update') && previous?.[0].kind === event.kind && previous[0].aggregate_id === event.aggregate_id) previous.push(event);
    else groups.push([event]);
  }
  return <section className="events">{events.length ? <ul ref={list} className="journal-list">{groups.reverse().map(group => {
    const event = group.at(-1)!;
    const label = eventLabels[event.kind]?.[zh ? 1 : 0] ?? (zh ? '已记录事件' : 'Recorded event');
    const date = new Date(event.occurred_at);
    const name = taskName(event.aggregate_id);
    return <li key={event.event_id}><details><summary>
      <span>{label}{group.length > 1 ? ` ×${group.length}` : ''}</span>{name && <span className="journal-object">{name}</span>}
      <time dateTime={event.occurred_at}>{Number.isNaN(+date) ? '—' : time.format(date)}</time>
    </summary>{group.map(entry => <section id={`journal-event-${entry.event_id}`} key={entry.event_id}>
      <p><code>{entry.kind}</code> <code>#{entry.sequence}</code> <time>{entry.occurred_at}</time></p><pre>{JSON.stringify(entry, null, 2)}</pre>
    </section>)}</details></li>;
  })}</ul> : <p>{empty}</p>}</section>;
}
