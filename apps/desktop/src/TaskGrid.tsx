import { memo, useMemo } from 'react';
import type { JournalEvent, Project, Task } from './api';
import type { ManagedSession, NamedConnection } from './session-api';
import { messages, type Language } from './i18n';
import { canvasMessages } from './canvas-i18n';
import { Icon } from './Icons';
import { taskGridIndexes } from './task-grid-model';

type Props = { tasks: Task[]; projects: Project[]; connections: NamedConnection[]; sessions: ManagedSession[]; events: JournalEvent[]; language: Language; onOpenTask: (id: string) => void };
/** Composer keystrokes do not change this subtree or rebuild its lookups. */
export const TaskGrid = memo(function TaskGrid({ tasks, projects, connections, sessions, events, language, onOpenTask }: Props) {
  const t = messages[language], c = canvasMessages[language];
  const { projectById, connectionById, sessionByTask, latestEventByTask } = useMemo(() => taskGridIndexes(projects, connections, sessions, events), [projects, connections, sessions, events]);
  const stateText = (state: string) => Object.hasOwn(t, state) ? t[state as keyof typeof t] : `${t.unknown}: ${state}`;
  return <div className="task-grid">{tasks.map(task => {
    const session = sessionByTask.get(task.id), connection = session ? connectionById.get(session.connection_id) : undefined;
    const latest = latestEventByTask.get(task.id);
    return <button className="task-card" data-testid="task-card" data-task-id={task.id} key={task.id} onClick={() => onOpenTask(task.id)} aria-label={`${c.openTask}: ${task.goal}`}><div className="card-top"><span className={`task-state state-${task.state}`}><span className="status-dot" />{stateText(task.state)}</span><Icon name="arrow" /></div><h2>{task.goal}</h2><p className="card-preview">{!latest || latest.kind === 'task.created' ? c.recent : latest.kind === 'task.cancelled' ? t.cancelled : c.recordedAction}</p><div className="card-footer"><span><Icon name="folder" />{(task.project_id === undefined ? undefined : projectById.get(task.project_id))?.name ?? t.localDrafts}</span><span>{connection?.name ?? c.noConnection}</span></div></button>;
  })}</div>;
});
