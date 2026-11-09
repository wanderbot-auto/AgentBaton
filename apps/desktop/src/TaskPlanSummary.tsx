import { useEffect, useRef, useState } from 'react';
import type { Task } from './api';
import type { Language } from './i18n';
import { goalApi, safeInteger, type GoalPlan, type GoalScope } from './goal-api';
import { goalExecutionApi, goalRunFloor, goalRunMeetsFloor, goalRunShape, goalRunViewShape, type GoalRun, type GoalRunView } from './goal-execution-api';
import { goalGraphApi } from './goal-graph-api';
import { graphSummaryShape, graphDetailsShape, graphSummaryMeetsFloor, graphDetailsMeetFloor, graphViewConsistent, type GraphEvidenceFloor } from './goal-graph-guards';
import type { GraphRunSummary, GraphRunDetails } from './goal-graph-types';
import { taskPlanSummaryMessages } from './task-plan-summary-i18n';

type Props = { task: Task; language: Language; connected: boolean };
const id = (value: unknown): value is string => typeof value === 'string' && /^[A-Za-z0-9._:-]{1,128}$/.test(value);
function scopeMatches(scope: GoalScope, task: Task) {
  return !!scope && id(scope.id) && scope.task_id === task.id && scope.project_id === task.project_id && scope.goal?.task_id === task.id && scope.goal.root_task_id === task.id && scope.goal.project_id === task.project_id && safeInteger(scope.goal.spec_revision, 1) && safeInteger(scope.authority_revision, 1) && typeof scope.revoked === 'boolean' && id(scope.root_grant_id) && id(scope.workspace_id);
}
function planMatches(plan: GoalPlan, scope: GoalScope) {
  const p = plan?.proposal;
  return !!p && plan.scope_id === scope.id && p.schema_version === 1 && safeInteger(p.revision, 1) && p.expected_revision === p.revision - 1 && p.scope_revision === scope.authority_revision && id(p.plan_id) && p.goal && (['task_id','project_id','root_task_id','spec_revision','input_revision'] as const).every(key => p.goal[key] === scope.goal[key]) && typeof plan.digest === 'string' && /^[a-f0-9]{64}$/.test(plan.digest) && plan.state === 'validated_proposal' && plan.dispatched === false && Array.isArray(p.nodes) && p.nodes.length > 0 && p.nodes.length <= 256 && new Set(p.nodes.map(n => n?.id)).size === p.nodes.length && p.nodes.every(n => !!n && id(n.id) && typeof n.goal === 'string' && n.goal.length <= 16384);
}
export function TaskPlanSummary(props: Props) {
  const [open, setOpen] = useState(false);
  return <details className="folded-plan" onToggle={event => setOpen(event.currentTarget.open)}><summary>{taskPlanSummaryMessages[props.language].title}</summary>{open && <PlanSummary key={`${props.task.id}:${props.task.project_id}:${props.task.spec_revision}`} {...props} />}</details>;
}
function PlanSummary({ task, language, connected }: Props) {
  const t = taskPlanSummaryMessages[language];
  const [refresh, setRefresh] = useState(0), [scopes, setScopes] = useState<GoalScope[]>([]), [runs, setRuns] = useState<GoalRun[]>([]);
  const [graphs, setGraphs] = useState<GraphRunSummary[]>([]);
  const legacyFloors = useRef(new Map<string, GoalRun>()), graphFloors = useRef(new Map<string, GraphEvidenceFloor>());
  const [scopeId, setScopeId] = useState(''), [runId, setRunId] = useState('');
  const [catalog, setCatalog] = useState<'loading'|'ready'|'error'>('loading');
  const [detail, setDetail] = useState<{ scopeId: string; runId: string; plan: GoalPlan | null; view: GoalRunView | GraphRunDetails | null; graph: boolean } | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let active = true;
    setCatalog('loading'); setDetail(null); setFailed(false);
    if (!connected) return () => { active = false; };
    void Promise.all([goalApi.scopesList(task.id), goalExecutionApi.list(task.id), goalGraphApi.list(task.id)]).then(([s, r, g]) => {
      if (!active) return;
      if (!Array.isArray(s) || s.length > 256 || s.some(item => !scopeMatches(item,task)) || new Set(s.map(item => item.id)).size !== s.length || !Array.isArray(r) || r.length > 256 || r.some(item => !goalRunShape(item,task) || !s.some(scope => scope.id === item.scope_id && scope.root_grant_id === item.root_grant_id)) || new Set(r.map(item => item.id)).size !== r.length) throw new Error();
      if (!Array.isArray(g) || g.length > 256 || g.some(row => !graphSummaryShape(row,task) || !s.some(scope => scope.id === row.run.scope_id && graphSummaryShape(row,task,scope))) || new Set(g.map(row => row.run.id)).size !== g.length || g.some(row => r.some(run => run.id === row.run.id))) throw new Error();
      for (const [id, floor] of legacyFloors.current) { const row = r.find(item => item.id === id); if (!row || !goalRunMeetsFloor(row,goalRunFloor(floor))) throw new Error(); }
      for (const [id, floor] of graphFloors.current) { const row = g.find(item => item.run.id === id); if (!row || !graphSummaryMeetsFloor(row,floor.summary)) throw new Error(); }
      for (const row of r) legacyFloors.current.set(row.id, row);
      for (const row of g) graphFloors.current.set(row.run.id, { summary: row, details: graphFloors.current.get(row.run.id)?.details ?? null });
      setScopes(s); setRuns(r); setGraphs(g); setScopeId(old => s.some(item => item.id === old) ? old : s[0]?.id ?? ''); setCatalog('ready');
    }).catch(() => { if (active) setCatalog('error'); });
    return () => { active = false; };
  }, [task.id, task.project_id, task.spec_revision, task.revision, connected, refresh]);
  const scope = scopes.find(item => item.id === scopeId);
  const scopeGraphs = graphs.filter(item => item.run.scope_id === scopeId);
  const scopeRuns = [...scopeGraphs.map(item => item.run), ...runs.filter(item => item.scope_id === scopeId)];
  const run = scopeRuns.find(item => item.id === runId) ?? scopeRuns[0];
  const graph = scopeGraphs.find(item => item.run.id === run?.id);
  const selectedRunId = run?.id ?? '';
  useEffect(() => {
    let active = true;
    setDetail(null); setFailed(false);
    if (!connected || catalog !== 'ready' || !scope) return () => { active = false; };
    void (async () => {
      const history = await goalApi.plansList(scope.id);
      if (!Array.isArray(history) || history.length > 256 || history.some(plan => !planMatches(plan,scope)) || new Set(history.map(plan => plan.proposal.revision)).size !== history.length || new Set(history.map(plan => plan.proposal.plan_id)).size > 1) throw new Error();
      const plan = [...history].sort((a,b) => b.proposal.revision-a.proposal.revision)[0] ?? null;
      const view = run ? graph ? await goalGraphApi.get(run.id) : await goalExecutionApi.get(run.id) : null;
      if (!active) return;
      if (view && (!run || view.run.id !== run.id || !history.some(p => p.proposal.revision === view.run.plan_revision && p.digest === view.run.plan_digest))) throw new Error();
      if (view && graph) {
        if (!graphDetailsShape(view,task,scope) || !graphViewConsistent(view) || !graphSummaryMeetsFloor(view,graph) || !graphDetailsMeetFloor(view,graphFloors.current.get(graph.run.id)!)) throw new Error();
        graphFloors.current.set(view.run.id,{ summary: view, details: view });
      } else if (view && run) {
        if (!goalRunViewShape(view as GoalRunView,task,scope) || !goalRunMeetsFloor(view.run as GoalRun,goalRunFloor(run as GoalRun))) throw new Error();
        legacyFloors.current.set(view.run.id,view.run as GoalRun);
      }
      setDetail({ scopeId: scope.id, runId: selectedRunId, plan, view, graph: !!graph });
    })().catch(() => { if (active) setFailed(true); });
    return () => { active = false; };
  }, [scope, run, graph, selectedRunId, connected, catalog, task.id, task.project_id, task.spec_revision, task.revision, refresh]);
  const current = detail?.scopeId === scopeId && detail.runId === selectedRunId ? detail : null;
  const state = (value: string) => Object.hasOwn(t.states,value) ? t.states[value as keyof typeof t.states] : t.unknown;
  const pending = current?.view?.nodes.filter(node => node.plan_revision === current.view!.run.plan_revision && !['candidate','cancelled','accepted','validated'].includes(node.state)) ?? [];
  const needsAttention = current?.view && (!Object.hasOwn(t.states,current.view.run.state) || ['awaiting_acceptance','blocked','failed','outcome_unknown','cancel_requested'].includes(current.view.run.state));
  return <section aria-label={t.title} aria-busy={connected && (catalog === 'loading' || (catalog === 'ready' && !!scope && !current && !failed))}>
    <button type="button" disabled={!connected} onClick={() => setRefresh(n => n+1)}>{t.refresh}</button>
    {!connected ? <p>{t.unavailable}</p> : catalog === 'error' || failed ? <p role="alert">{t.error}</p> : <>
      {catalog === 'loading' && <p>{t.loading}</p>}
      {catalog === 'ready' && !scopes.length && <p>{t.empty}</p>}
      {catalog === 'ready' && !!scopes.length && <label>{t.scope}<select value={scopeId} onChange={event => { setScopeId(event.target.value); setRunId(''); }}>{scopes.map(item => <option key={item.id} value={item.id}>{item.id}</option>)}</select></label>}
      {scope && (scope.revoked || scope.goal.spec_revision !== task.spec_revision) && <p>{t.stale}</p>}
      {!!scopeRuns.length && <label>{t.run}<select value={selectedRunId} onChange={event => setRunId(event.target.value)}>{scopeRuns.map(item => <option key={item.id} value={item.id}>{scopeGraphs.some(g => g.run.id === item.id) ? t.graph : t.legacy} · {item.id}</option>)}</select></label>}
      {catalog === 'ready' && scope && !current && <p>{t.loading}</p>}
      {current && <>{current.plan ? <><p>{t.proposal} · {t.revision} {current.plan.proposal.revision}</p><ol>{current.plan.proposal.nodes.map(node => <li key={node.id}>{node.goal}</li>)}</ol></> : <p>{t.empty}</p>}
        {current.view ? <><p>{current.graph ? t.graph : t.legacy}</p><p>{t.run} · {t.revision} {current.view.run.plan_revision}: {state(current.view.run.state)}</p><ul>{current.view.nodes.map((record,index) => <li key={index}>{record.node.goal} · {t.revision} {record.plan_revision}: {state(record.state)}</li>)}</ul><h3>{t.pending}</h3>{needsAttention && <p>{state(current.view.run.state)}</p>}{pending.length ? <ul>{pending.map((record,index) => <li key={index}>{record.node.goal}: {state(record.state)}</li>)}</ul> : !needsAttention && <p>{t.none}</p>}<p className="hint">{current.graph && current.view.task_accepted ? t.accepted : t.acceptance}</p></> : <p>{t.noRun}</p>}
      </>}
    </>}
  </section>;
}
