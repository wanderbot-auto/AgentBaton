import { useEffect, useState } from 'react';
import type { JournalEvent, Task } from './api';
import type { ManagedSession } from './session-api';
import type { Language } from './i18n';
import { ContextDialog } from './ContextDialog';
import { aggregateTaskNotifications, NOTIFICATION_LIMITS, retainNotificationEvidence, type NotificationEvidence, type NotificationObservation, type TaskNotification } from './task-notifications-model';
import { taskNotificationMessages } from './task-notifications-i18n';
import './task-notifications.css';

interface TaskNotificationsProps {
  tasks: Task[];
  sessions: ManagedSession[];
  events: JournalEvent[];
  language: Language;
  observation: NotificationObservation;
  onOpenTask: (id: string) => void;
  onViewEvent: (eventId: string) => void;
}
const preferenceKey = 'agentbaton.notifications.doNotDisturb';
const emptyEvidence: NotificationEvidence = { refs: [], limited: false };

/** In-app observation surface only. No OS notification, execution or approval API. */
export function TaskNotifications({ tasks, sessions, events, language, observation, onOpenTask, onViewEvent }: TaskNotificationsProps) {
  const t = taskNotificationMessages[language];
  const [open, setOpen] = useState(false);
  const [page, setPage] = useState(0);
  const [retained, setRetained] = useState<NotificationEvidence>(emptyEvidence);
  const [preference, setPreference] = useState(() => {
    try { return { enabled: localStorage.getItem(preferenceKey) === 'true', windowOnly: false }; }
    catch { return { enabled: false, windowOnly: true }; }
  });
  const evidence = retainNotificationEvidence(retained, tasks, sessions, events);
  useEffect(() => {
    setRetained(previous => retainNotificationEvidence(previous, tasks, sessions, events));
  }, [tasks, sessions, events]);
  const model = aggregateTaskNotifications(tasks, sessions, evidence, observation);
  const pages = Math.max(1, Math.ceil(model.rows.length / NOTIFICATION_LIMITS.rows));
  const currentPage = Math.min(page, pages - 1);
  const attention = !preference.enabled && model.rows.some(row => !row.stale && !['accepted', 'observed'].includes(row.category));
  function setDoNotDisturb(enabled: boolean) {
    let windowOnly = false;
    try { localStorage.setItem(preferenceKey, String(enabled)); } catch { windowOnly = true; }
    setPreference({ enabled, windowOnly });
  }
  return <>
    <button type="button" className="task-notifications-trigger" aria-haspopup="dialog" aria-expanded={open} aria-label={preference.enabled ? t.muted : `${t.trigger} (${model.rows.length})`} data-attention={attention || undefined} onClick={() => setOpen(true)}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M18 8a6 6 0 0 0-12 0v5l-2 3h16l-2-3V8M9 20h6" /></svg>
      {!preference.enabled && model.rows.length > 0 && <span aria-hidden="true">{model.rows.length}</span>}
    </button>
    {open && <ContextDialog title={t.title} closeLabel={t.close} onClose={() => setOpen(false)}>
      <div className="task-notifications">
        <label className="task-notifications-dnd"><input type="checkbox" checked={preference.enabled} onChange={event => setDoNotDisturb(event.target.checked)} />{t.dnd}</label>
        <p className="hint">{t.dndHelp}</p>
        {preference.windowOnly && <p role="status">{t.windowOnly}</p>}
        {(!observation.connected || !observation.tasksFresh) && <p className="hint">{t.stale}</p>}
        {!observation.sessionsFresh && <p className="hint">{t.sessionsStale}</p>}
        {!observation.eventsFresh && <p className="hint">{t.eventsStale}</p>}
        {(model.limited || observation.eventLimited || !observation.eventsFresh || !observation.connected) && <p className="hint">{t.limited}</p>}
        {!model.rows.length ? <p>{t.empty}</p> : <ul className="task-notifications-list">{model.rows.slice(currentPage * NOTIFICATION_LIMITS.rows, (currentPage + 1) * NOTIFICATION_LIMITS.rows).map(row => <Notice key={row.taskId} row={row} language={language} observation={observation} events={events} onOpenTask={id => { setOpen(false); onOpenTask(id); }} onViewEvent={id => { setOpen(false); onViewEvent(id); }} />)}</ul>}
        {pages > 1 && <Pager page={currentPage} pages={pages} language={language} onChange={setPage} />}
      </div>
    </ContextDialog>}
  </>;
}

function Pager({ page, pages, language, onChange }: { page: number; pages: number; language: Language; onChange: (page: number) => void }) {
  const t = taskNotificationMessages[language];
  return <div className="task-notifications-pager"><button type="button" disabled={page === 0} onClick={() => onChange(page - 1)}>{t.previous}</button><span>{t.page} {page + 1} / {pages}</span><button type="button" disabled={page + 1 >= pages} onClick={() => onChange(page + 1)}>{t.next}</button></div>;
}

function Notice({ row, language, observation, events, onOpenTask, onViewEvent }: { row: TaskNotification; language: Language; observation: NotificationObservation; events: JournalEvent[]; onOpenTask: (id: string) => void; onViewEvent: (id: string) => void }) {
  const t = taskNotificationMessages[language];
  const [sourcePage, setSourcePage] = useState(0);
  const [sessionPage, setSessionPage] = useState(0);
  const sourcePages = Math.max(1, Math.ceil(row.sources.length / NOTIFICATION_LIMITS.sources));
  const sessionPages = Math.max(1, Math.ceil(row.sessions.length / NOTIFICATION_LIMITS.sources));
  const currentSourcePage = Math.min(sourcePage, sourcePages - 1), currentSessionPage = Math.min(sessionPage, sessionPages - 1);
  return <li data-testid="task-notification" data-task-id={row.taskId}>
    <div className="task-notifications-row"><h3>{row.goal || row.taskId}</h3><button type="button" onClick={() => onOpenTask(row.taskId)}>{t.openTask}</button></div>
    <p>{t[row.category]}</p>
    <p className="hint">{row.stale ? t.stale : row.category === 'observed' ? t.historical : t.fresh}</p>
    <details><summary>{t.sources} ({row.sources.length})</summary>
      <p className="hint">{t.sourceHelp}</p>
      <p>{t.taskProjection}: <code>{row.taskId}</code> · {t.revision} {row.taskRevision} · {t.state}: <code>{row.taskState}</code> · {t.updated}: {row.updatedAt}</p>
      {row.sessions.length > 0 && <><p className="hint">{t.sessionOnly}{!observation.sessionsFresh || !observation.connected ? ` ${t.sessionsStale}` : ''}</p><ul>{row.sessions.slice(currentSessionPage * NOTIFICATION_LIMITS.sources, (currentSessionPage + 1) * NOTIFICATION_LIMITS.sources).map(session => <li key={session.id}>{t.sessionProjection}: <code>{session.id}</code> · {t.revision} {session.revision} · {t.state}: <code>{session.state}{session.turnState ? ` / ${session.turnState}` : ''}</code> · {t.updated}: {session.updatedAt}</li>)}</ul>{sessionPages > 1 && <Pager page={currentSessionPage} pages={sessionPages} language={language} onChange={setSessionPage} />}</>}
      <h3>{t.events}</h3><p className="hint">{t.eventOnly}</p>
      {row.sources.length ? <ol>{row.sources.slice(currentSourcePage * NOTIFICATION_LIMITS.sources, (currentSourcePage + 1) * NOTIFICATION_LIMITS.sources).map(source => {
        const loaded = events.slice(-NOTIFICATION_LIMITS.events).some(event => event.event_id === source.eventId && event.aggregate_id === (source.sessionId ?? source.taskId) && event.sequence === source.sequence && event.revision === source.revision && event.kind === source.kind && event.source === source.source);
        return <li key={source.eventId}><details><summary><code>{source.kind}</code> · #{source.sequence}</summary><dl><dt>{t.eventId}</dt><dd>{source.eventId}</dd><dt>{t.aggregate}</dt><dd>{source.sessionId ?? source.taskId}</dd><dt>{t.origin}</dt><dd>{source.source || '—'}</dd><dt>{t.revision}</dt><dd>{source.revision}</dd><dt>{t.updated}</dt><dd>{source.occurredAt}</dd></dl>{loaded ? <button type="button" onClick={() => onViewEvent(source.eventId)}>{t.viewEvent}</button> : <p className="hint">{t.outsideWindow}</p>}</details></li>;
      })}</ol> : <p className="hint">{t.noEvents}</p>}
      {sourcePages > 1 && <Pager page={currentSourcePage} pages={sourcePages} language={language} onChange={setSourcePage} />}
    </details>
  </li>;
}
