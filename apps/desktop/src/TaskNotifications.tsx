import { Button, Code, Disclosure, DisclosureLabel, Input, Title } from './ui';
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
const emptyEvidence: NotificationEvidence = { refs: [], limited: false, conflictedIds: [], navigationBlocked: false };

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
  const evidence = retainNotificationEvidence(retained, tasks, sessions, events, observation.eventSequence);
  useEffect(() => {
    setRetained(previous => retainNotificationEvidence(previous, tasks, sessions, events, observation.eventSequence));
  }, [tasks, sessions, events, observation.eventSequence]);
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
    <Button type="button" className="task-notifications-trigger" aria-haspopup="dialog" aria-expanded={open} aria-label={preference.enabled ? t.muted : `${t.trigger} (${model.rows.length})`} data-attention={attention || undefined} onClick={() => setOpen(true)}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M18 8a6 6 0 0 0-12 0v5l-2 3h16l-2-3V8M9 20h6" /></svg>
      {!preference.enabled && model.rows.length > 0 && <span className="task-notifications-count" aria-hidden="true">{model.rows.length}</span>}
    </Button>
    <ContextDialog opened={open} title={t.title} closeLabel={t.close} onClose={() => setOpen(false)}>
      <div className="task-notifications">
        <label className="task-notifications-dnd"><Input type="checkbox" checked={preference.enabled} onChange={event => setDoNotDisturb(event.target.checked)} />{t.dnd}</label>
        {preference.windowOnly && <p role="status">{t.windowOnly}</p>}
        {(!observation.connected || !observation.tasksFresh) && <p className="hint">{t.stale}</p>}
        {!observation.sessionsFresh && <p className="hint">{t.sessionsStale}</p>}
        {!observation.eventsFresh && <p className="hint">{t.eventsStale}</p>}
        {(model.limited || observation.eventLimited || !observation.eventsFresh || !observation.connected) && <p className="hint">{t.limited}</p>}
        {!model.rows.length ? <p>{t.empty}</p> : <ul className="task-notifications-list">{model.rows.slice(currentPage * NOTIFICATION_LIMITS.rows, (currentPage + 1) * NOTIFICATION_LIMITS.rows).map(row => <Notice key={row.taskId} row={row} language={language} observation={observation} events={events} evidence={evidence} onOpenTask={id => { setOpen(false); onOpenTask(id); }} onViewEvent={id => { setOpen(false); onViewEvent(id); }} />)}</ul>}
        {pages > 1 && <Pager page={currentPage} pages={pages} language={language} onChange={setPage} />}
      </div>
    </ContextDialog>
  </>;
}

function Pager({ page, pages, language, onChange }: { page: number; pages: number; language: Language; onChange: (page: number) => void }) {
  const t = taskNotificationMessages[language];
  return <div className="task-notifications-pager"><Button type="button" disabled={page === 0} onClick={() => onChange(page - 1)}>{t.previous}</Button><span>{t.page} {page + 1} / {pages}</span><Button type="button" disabled={page + 1 >= pages} onClick={() => onChange(page + 1)}>{t.next}</Button></div>;
}

function Notice({ row, language, observation, events, evidence, onOpenTask, onViewEvent }: { row: TaskNotification; language: Language; observation: NotificationObservation; events: JournalEvent[]; evidence: NotificationEvidence; onOpenTask: (id: string) => void; onViewEvent: (id: string) => void }) {
  const t = taskNotificationMessages[language];
  const [sourcePage, setSourcePage] = useState(0);
  const [sessionPage, setSessionPage] = useState(0);
  const sourcePages = Math.max(1, Math.ceil(row.sources.length / NOTIFICATION_LIMITS.sources));
  const sessionPages = Math.max(1, Math.ceil(row.sessions.length / NOTIFICATION_LIMITS.sources));
  const currentSourcePage = Math.min(sourcePage, sourcePages - 1), currentSessionPage = Math.min(sessionPage, sessionPages - 1);
  return <li data-testid="task-notification" data-task-id={row.taskId}>
    <div className="task-notifications-row"><Title order={3}>{row.goal || row.taskId}</Title><Button type="button" onClick={() => onOpenTask(row.taskId)}>{t.openTask}</Button></div>
    <p>{t[row.category]}</p>
    <p className="hint">{row.stale ? t.stale : row.category === 'observed' ? t.historical : t.fresh}</p>
    <Disclosure><DisclosureLabel>{t.sources} ({row.sources.length})</DisclosureLabel>
      <p className="hint">{t.sourceHelp}</p>
      <p>{t.taskProjection}: <Code>{row.taskId}</Code> · {t.revision} {row.taskRevision} · {t.state}: <Code>{row.taskState}</Code> · {t.updated}: {row.updatedAt}</p>
      {row.sessions.length > 0 && <><p className="hint">{t.sessionOnly}{!observation.sessionsFresh || !observation.connected ? ` ${t.sessionsStale}` : ''}</p><ul>{row.sessions.slice(currentSessionPage * NOTIFICATION_LIMITS.sources, (currentSessionPage + 1) * NOTIFICATION_LIMITS.sources).map(session => <li key={session.id}>{t.sessionProjection}: <Code>{session.id}</Code> · {t.revision} {session.revision} · {t.state}: <Code>{session.state}{session.turnState ? ` / ${session.turnState}` : ''}</Code> · {t.updated}: {session.updatedAt}</li>)}</ul>{sessionPages > 1 && <Pager page={currentSessionPage} pages={sessionPages} language={language} onChange={setSessionPage} />}</>}
      <Title order={3}>{t.events}</Title><p className="hint">{t.eventOnly}</p>
      {row.sources.length ? <ol>{row.sources.slice(currentSourcePage * NOTIFICATION_LIMITS.sources, (currentSourcePage + 1) * NOTIFICATION_LIMITS.sources).map(source => {
        const originals = events.slice(-NOTIFICATION_LIMITS.events).filter(event => event.event_id === source.eventId);
        const conflicting = evidence.navigationBlocked || evidence.conflictedIds.includes(source.eventId) || originals.length > 1;
        const loaded = !conflicting && originals.length === 1 && originals[0].aggregate_id === (source.sessionId ?? source.taskId) && originals[0].sequence === source.sequence && originals[0].revision === source.revision && originals[0].kind === source.kind && originals[0].source === source.source && originals[0].occurred_at === source.occurredAt;
        return <li key={source.eventId}><Disclosure><DisclosureLabel><Code>{source.kind}</Code> · #{source.sequence}</DisclosureLabel><dl><dt>{t.eventId}</dt><dd>{source.eventId}</dd><dt>{t.aggregate}</dt><dd>{source.sessionId ?? source.taskId}</dd><dt>{t.origin}</dt><dd>{source.source || '—'}</dd><dt>{t.revision}</dt><dd>{source.revision}</dd><dt>{t.updated}</dt><dd>{source.occurredAt}</dd></dl>{loaded ? <Button type="button" onClick={() => onViewEvent(source.eventId)}>{t.viewEvent}</Button> : <p className="hint">{conflicting ? t.conflictingSource : t.outsideWindow}</p>}</Disclosure></li>;
      })}</ol> : <p className="hint">{t.noEvents}</p>}
      {sourcePages > 1 && <Pager page={currentSourcePage} pages={sourcePages} language={language} onChange={setSourcePage} />}
    </Disclosure>
  </li>;
}
