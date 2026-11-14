import { useEffect, useRef, useState, type FormEvent } from 'react';
import { api, acknowledgeTaskMessage, DaemonError, taskAllowsExecution, type Task, type JournalEvent } from './api';
import { sessionApi, type ManagedSession, type NamedConnection } from './session-api';
import { sessionEventText } from './ManagedSessions';
import type { Language } from './i18n';
import { quickConversationMessages } from './quick-conversation-i18n';

type Props = { language: Language; connected: boolean; busy: boolean; tasks: Task[]; sessions: ManagedSession[]; connections: NamedConnection[]; events: JournalEvent[]; uncertainSessions: Set<string>; onUncertain: (id:string)=>void; onBusy:(busy:boolean)=>void; refresh:()=>Promise<void>; summary:string; summaryProject?:string; onSummary:(text:string,projectId?:string)=>void; onConvert:(summary:string,projectId?:string)=>void };
const fits = (text:string) => text.length <= 8192 && new TextEncoder().encode(text).length <= 8192;
const sameSession = (a:ManagedSession,b:ManagedSession) => ['id','task_id','project_id','workspace_id','connection_id','grant_id'].every(key => a[key as keyof ManagedSession] === b[key as keyof ManagedSession]);
export function QuickConversation(props: Props) {
  const { language, connected, tasks, sessions, connections, events } = props, t = quickConversationMessages[language];
  const [id,setId] = useState(''), [text,setText] = useState(''), [tick,setTick] = useState(0), [error,setError] = useState('');
  const [fresh,setFresh] = useState<{session:ManagedSession;task:Task}|null>(null), [loading,setLoading] = useState(false);
  const [chosen,setChosen] = useState<Map<string,string>>(new Map()), [includeGoal,setIncludeGoal] = useState(false);
  const floors = useRef(new Map<string,number>());
  const sending = useRef(false), keys = useRef(new Map<string,string>());
  const session = sessions.find(item => item.id === id), task = tasks.find(item => item.id === session?.task_id), connection = connections.find(item => item.id === session?.connection_id);
  useEffect(() => {
    let active = true; setFresh(null); setError('');
    if (!connected || !session || !task || !connection) { setLoading(false); return; }
    setLoading(true);
    void Promise.all([sessionApi.sessionsGet(session.id),api.taskGet(task.id)]).then(([s,taskRecord]) => {
      if (!active) return;
      if (!sameSession(s,session) || !Number.isSafeInteger(s.revision) || s.revision < Math.max(session.revision,floors.current.get(s.id) ?? 0) || taskRecord.id !== task.id || taskRecord.project_id !== s.project_id || taskRecord.revision < task.revision) throw new Error();
      floors.current.set(s.id,s.revision); setFresh({session:s,task:taskRecord});
    }).catch(() => { if (active) setError(t.error); }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [id, session?.revision, task?.revision, connection?.id, connected, tick, t.error]);
  const verified = !!fresh && !!session && !!task && !!connection && sameSession(fresh.session,session) && fresh.session.revision >= session.revision && fresh.task.id === task.id && fresh.task.revision >= task.revision;
  const blocked = !!session && sessions.some(item => item.task_id === session.task_id && props.uncertainSessions.has(item.id));
  const canSend = connected && !props.busy && !loading && verified && fresh!.session.state === 'ready' && taskAllowsExecution(fresh!.task) && !blocked && !!text.trim();
  const conversation = session ? events.filter(event => event.aggregate_id === session.id && sessionEventText(event) !== null) : [];
  async function send(event:FormEvent) {
    event.preventDefault(); if (!canSend || sending.current || !fresh) return;
    const observed = fresh.session, message = text, fingerprint = JSON.stringify([observed.id,observed.revision,message]);
    const key = keys.current.get(fingerprint) ?? crypto.randomUUID(); keys.current.set(fingerprint,key);
    sending.current = true; props.onBusy(true); setError('');
    try {
      const result = await sessionApi.sessionsSend(observed,message,key);
      if (!sameSession(result,observed) || observed.native_session_id !== null && result.native_session_id !== observed.native_session_id || !Number.isSafeInteger(result.revision) || result.revision <= observed.revision) throw new Error();
      floors.current.set(result.id,result.revision); keys.current.delete(fingerprint); setText(''); setFresh(null); setTick(value => value+1);
      await acknowledgeTaskMessage(result.id, key).catch(() => { /* Refresh will show the retained receipt. */ });
    } catch (failure) {
      const uncertain = !(failure instanceof DaemonError) || failure.code === 'OUTCOME_UNKNOWN' || !['none','not_started'].includes(failure.sideEffectState);
      if (uncertain) props.onUncertain(observed.id);
      setFresh(null); setError(uncertain ? t.uncertain : t.failed);
    } finally { sending.current = false; props.onBusy(false); void props.refresh(); }
  }
  function selectContext() {
    if (!session || !task || task.project_id !== session.project_id) { setError(t.error); return; }
    const pieces = [...(includeGoal && task ? [`${t.context}: ${task.goal}`] : []),...chosen.values()];
    if (!pieces.length) { setError(t.noSelection); return; }
    const summary = pieces.join('\n\n');
    if (!fits(summary)) { setError(t.tooLong); return; }
    props.onSummary(summary,session.project_id); setError('');
  }
  return <section className="quick-thread"><h1>{t.title}</h1><p>{t.help}</p>
    <label>{t.choose}<select value={id} disabled={props.busy} onChange={event => { setId(event.target.value); setText(''); setChosen(new Map()); setIncludeGoal(false); setFresh(null); }}><option value="">{t.placeholder}</option>{sessions.filter(s => tasks.some(task => task.id === s.task_id)).map(s => <option key={s.id} value={s.id}>{connections.find(c => c.id === s.connection_id)?.name ?? s.connection_id} · {tasks.find(task => task.id === s.task_id)?.goal} · {s.id}</option>)}</select></label>
    {!sessions.length && <p>{t.empty}</p>}{!session && <button type="button" disabled>{t.noSessionSend}</button>}
    {session && <><p>{t.context}: {task?.goal}</p><p>{t.connection}: {connection?.name ?? t.missing}</p><p>{t.session}: {session.id} · {fresh?.session.id === session.id ? fresh.session.state : session.state}</p><button type="button" disabled={!connected||props.busy||loading} onClick={() => { setTick(n=>n+1); void props.refresh(); }}>{t.refresh}</button>
      <p className="hint">{t.window}</p>{!conversation.length && <p>{t.noMessages}</p>}<ol>{conversation.map(event => { const body = sessionEventText(event)!; const excerpt = `${event.kind === 'session.message_queued' ? t.user : t.agent}: ${body}`; return <li key={event.event_id}><label><input type="checkbox" checked={chosen.has(event.event_id)} onChange={change => setChosen(previous => { const next = new Map(previous); if(change.target.checked)next.set(event.event_id,excerpt);else next.delete(event.event_id);return next; })} />{excerpt}</label></li>; })}</ol>
      <label><input type="checkbox" checked={includeGoal} onChange={event=>setIncludeGoal(event.target.checked)} />{t.include}</label><button type="button" disabled={!session || !task || task.project_id !== session.project_id} onClick={selectContext}>{t.selected}</button>
      <form onSubmit={event=>void send(event)}><label>{t.message}<textarea value={text} onChange={event=>setText(event.target.value)} maxLength={16384} disabled={props.busy||blocked} /></label><button type="submit" disabled={!canSend}>{t.send}</button></form>
      {blocked ? <p role="status">{t.uncertain}</p> : !verified || fresh?.session.state !== 'ready' ? <p>{connection ? t.unavailable : t.missing}</p> : null}
    </>}
    {error && <p role="alert">{error}</p>}
    <label htmlFor="quick-summary">{t.summary}</label><textarea id="quick-summary" value={props.summary} maxLength={8192} rows={6} onChange={event=>props.onSummary(event.target.value)} /><p className="hint">{t.summaryHint}</p>{props.summaryProject !== undefined && <p>{t.project}: {props.summaryProject}</p>}
    {!fits(props.summary) && <p role="alert">{t.tooLong}</p>}<button type="button" className="primary" disabled={props.busy||!connected||!props.summary.trim()||!fits(props.summary)} onClick={()=>props.onConvert(props.summary,props.summaryProject)}>{t.convert}</button>
  </section>;
}
