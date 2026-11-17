// Run from repository root: node docs/execution/operation-performance/benchmark-task-grid.mjs
// Measures lookup work only, not DOM rendering or native IPC.
import assert from 'node:assert/strict';
import { taskGridIndexes } from '../../../apps/desktop/src/task-grid-model.ts';
const tasks = Array.from({ length: 1000 }, (_, i) => ({ id: `t${i}`, project_id: `p${i}` }));
const projects = tasks.map((task, i) => ({ id: task.project_id, name: `P${i}` }));
const sessions = tasks.map((task, i) => ({ task_id: task.id, connection_id: `c${i}` }));
const connections = tasks.map((_, i) => ({ id: `c${i}` }));
const events = tasks.map((task, i) => ({ aggregate_id: task.id, sequence: i + 1, event_id: `e${i}` }));
const original = () => tasks.map(task => {
  const session = sessions.find(row => row.task_id === task.id);
  return [projects.find(p => p.id === task.project_id), connections.find(c => c.id === session?.connection_id), [...events].reverse().find(e => e.aggregate_id === task.id)];
});
const indexed = () => {
  const index = taskGridIndexes(projects, connections, sessions, events);
  return tasks.map(task => [index.projectById.get(task.project_id), index.connectionById.get(index.sessionByTask.get(task.id).connection_id), index.latestEventByTask.get(task.id)]);
};
assert.deepStrictEqual(indexed(), original());
const oldSamples = [], newSamples = [];
for (let round = 0; round < 9; round++) {
  // Alternate order to reduce warmup/order bias; initial equivalence run warms both.
  for (const [fn, result] of round % 2 ? [[indexed, newSamples], [original, oldSamples]] : [[original, oldSamples], [indexed, newSamples]]) {
    const start = performance.now();
    for (let i = 0; i < 20; i++) fn();
    result.push((performance.now() - start) / 20);
  }
}
oldSamples.sort((a,b) => a-b); newSamples.sort((a,b) => a-b);
console.log(JSON.stringify({ fixture: { tasks: 1000, projects: 1000, sessions: 1000, connections: 1000, events: 1000 }, unit: 'milliseconds_per_lookup_batch', equivalent: true, originalMedian: oldSamples[4], indexedMedian: newSamples[4], originalSamples: oldSamples, indexedSamples: newSamples }, null, 2));
