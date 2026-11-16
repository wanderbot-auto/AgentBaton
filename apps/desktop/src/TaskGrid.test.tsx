import { useState } from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { TaskGrid } from './TaskGrid';
import { taskGridIndexes } from './task-grid-model';
import type { JournalEvent, Project, Task } from './api';
import type { ManagedSession, NamedConnection } from './session-api';

const task: Task = { id: 't', goal: 'Task goal', project_id: 'p', state: 'draft', revision: 1, created_at: 'now', updated_at: 'now' };
const session: ManagedSession = { id: 's', task_id: 't', project_id: 'p', workspace_id: 'w', connection_id: 'c', grant_id: 'g', state: 'ready', revision: 1, native_session_id: null, created_at: 'now', updated_at: 'now', last_error: null, turn_state: null };
const connection: NamedConnection = { id: 'c', name: 'First connection', provider: 'codex', executable: '/bin/codex', arguments: [], executable_sha256: 'a'.repeat(64), environment_id: 'environment', host_id: 'local', config_revision: '1', identity_ref: 'identity', revision: 1, probe_state: 'ready', approval_id: 'approval', capability_snapshot_id: 'capabilities', identity_status: 'ready', config_status: 'ready', version_evidence: null, created_at: 'now' };
const project: Project = { id: 'p', name: 'First project', identity: { canonical_root: '/source', device: 1, inode: 1, kind: { kind: 'directory' } }, revision: 1, archived: false, created_at: 'now', updated_at: 'now' };
const event: JournalEvent = { sequence: 1, event_id: 'e', aggregate_id: 't', revision: 1, kind: 'task.created', occurred_at: 'now', source: 'user', payload: {} };

it('does not render unchanged cards while typing and still updates cards and navigation', () => {
  let goalReads = 0;
  const observedTask = { ...task, get goal() { goalReads++; return task.goal; } };
  const props = { tasks: [observedTask], projects: [project], connections: [connection], sessions: [session], events: [event], language: 'en' as const, onOpenTask: vi.fn() };
  function Composer() { const [value, setValue] = useState(''); return <><input aria-label="Goal input" value={value} onChange={e => setValue(e.target.value)} /><TaskGrid {...props} /></>; }
  const page = render(<Composer />), before = goalReads;
  for (let index = 0; index < 10; index++) fireEvent.change(screen.getByRole('textbox'), { target: { value: `Goal ${index}` } });
  expect(goalReads).toBe(before);
  fireEvent.click(screen.getByTestId('task-card')); expect(props.onOpenTask).toHaveBeenCalledWith(task.id);
  page.rerender(<TaskGrid {...props} tasks={[{ ...task, goal: 'Updated goal', state: 'cancelled', revision: 2 }]} events={[{ ...event, kind: 'task.cancelled' }]} />);
  expect(screen.getByRole('heading', { name: 'Updated goal' })).toBeInTheDocument();
  expect(screen.getByTestId('task-card')).toHaveTextContent('Stopped (confirmed)');
});

it('preserves first catalog matches, event window order, missing data and unknown states', () => {
  const projects = [project, { ...project, name: 'Second project' }], connections = [connection, { ...connection, name: 'Second connection' }];
  const sessions = [session, { ...session, id: 'second', connection_id: 'other' }];
  // Last array position wins, as in reverse().find(); not the highest sequence.
  const events = [{ ...event, sequence: 9 }, { ...event, sequence: 2, kind: 'task.cancelled' }];
  const index = taskGridIndexes(projects, connections, sessions, events);
  expect(index.projectById.get('p')).toBe(project); expect(index.connectionById.get('c')).toBe(connection);
  expect(index.sessionByTask.get('t')).toBe(session); expect(index.latestEventByTask.get('t')).toBe(events[1]);
  render(<TaskGrid tasks={[task, { ...task, id: 'orphan', project_id: undefined, goal: 'Unknown task', state: 'future_state' }]} projects={projects} connections={connections} sessions={sessions} events={events} language="en" onOpenTask={vi.fn()} />);
  expect(screen.getAllByTestId('task-card')[0]).toHaveTextContent('First project');
  expect(screen.getAllByTestId('task-card')[0]).toHaveTextContent('First connection');
  expect(screen.getAllByTestId('task-card')[0]).toHaveTextContent('Stopped (confirmed)');
  expect(screen.getAllByTestId('task-card')[1]).toHaveTextContent('Unknown status: future_state');
});

it('compares indexed and original card lookup work on 1000 tasks', () => {
  const tasks = Array.from({ length: 1000 }, (_, i) => ({ ...task, id: `t${i}`, project_id: `p${i}` }));
  const projects = tasks.map((t, i) => ({ ...project, id: t.project_id, name: `P${i}` }));
  const sessions = tasks.map((t, i) => ({ ...session, task_id: t.id, connection_id: `c${i}` }));
  const connections = tasks.map((_, i) => ({ ...connection, id: `c${i}` }));
  const events = tasks.map((t, i) => ({ ...event, aggregate_id: t.id, sequence: i + 1, event_id: `e${i}` }));
  const original = () => tasks.map(task => {
    const session = sessions.find(row => row.task_id === task.id);
    return [projects.find(p => p.id === task.project_id), connections.find(c => c.id === session?.connection_id), [...events].reverse().find(e => e.aggregate_id === task.id)];
  });
  const indexed = () => {
    const index = taskGridIndexes(projects, connections, sessions, events);
    return tasks.map(task => [index.projectById.get(task.project_id), index.connectionById.get(index.sessionByTask.get(task.id)!.connection_id), index.latestEventByTask.get(task.id)]);
  };
  expect(indexed()).toEqual(original());
  const samples = (read: () => unknown) => Array.from({ length: 7 }, () => { const start = performance.now(); for (let i = 0; i < 5; i++) read(); return (performance.now() - start) / 5; }).sort((a,b) => a-b);
  const oldTimes = samples(original), newTimes = samples(indexed);
  console.info(JSON.stringify({ benchmark: '1000_task_card_lookups_ms', originalMedian: oldTimes[3], indexedMedian: newTimes[3], originalSamples: oldTimes, indexedSamples: newTimes }));
});
