import type { JournalEvent, Project } from './api';
import type { ManagedSession, NamedConnection } from './session-api';

/** Preserve first-match catalog semantics and last-in-window event semantics. */
export function taskGridIndexes(projects: Project[], connections: NamedConnection[], sessions: ManagedSession[], events: JournalEvent[]) {
  const projectById = new Map<string, Project>(), connectionById = new Map<string, NamedConnection>();
  const sessionByTask = new Map<string, ManagedSession>(), latestEventByTask = new Map<string, JournalEvent>();
  for (const project of projects) if (!projectById.has(project.id)) projectById.set(project.id, project);
  for (const connection of connections) if (!connectionById.has(connection.id)) connectionById.set(connection.id, connection);
  for (const session of sessions) if (!sessionByTask.has(session.task_id)) sessionByTask.set(session.task_id, session);
  for (const event of events) latestEventByTask.set(event.aggregate_id, event);
  return { projectById, connectionById, sessionByTask, latestEventByTask };
}
