import type { Task } from './api';
import type { GoalScope } from './goal-api';
import type { Language } from './i18n';
import type { GraphArtifactReference,GraphRunDetails } from './goal-graph-types';
import { graphTaskAcceptanceMatchesTask } from './goal-graph-guards';
import { graphGraphLabel } from './goal-graph-labels';
import { goalGraphMessages } from './goal-graph-i18n';
import './goal-graph.css';
export function GoalGraphArtifact({value,language}:{value:GraphArtifactReference;language:Language}){
  const t=goalGraphMessages[language];
  return <dl className="goal-graph-facts"><dt>{t.artifact}</dt><dd>{value.artifact_id}</dd><dt>{t.digest}</dt><dd><code>{value.sha256}</code></dd><dt>{t.bytes}</dt><dd>{value.bytes}</dd><dt>{t.root}</dt><dd>{value.root.canonical_root}</dd><dt>{t.details}</dt><dd>{value.relative_path}</dd><dt>{t.attempt}</dt><dd>{value.producer_attempt_id}</dd></dl>;
}
// Read-only presentation. Shape/freshness/action gates live at the caller;
// no artifact path is opened or sent back as authority from this component.
export function GoalGraphExecutionHistory({view,language,task,scope}:{view:GraphRunDetails;language:Language;task?:Task;scope?:GoalScope}){
  const t=goalGraphMessages[language],count=(value:number|null)=>value===null?t.notMeasured:String(value);
  const accepted=view.graph_contract_version===1&&view.task_accepted===true&&view.run.task_accepted===true&&view.run.state==='accepted'&&!!task&&!!scope&&graphTaskAcceptanceMatchesTask(view.task_acceptance,task,scope,view);
  return <section data-testid="goal-graph-history" data-run-id={view.run.id} className="goal-graph-history" aria-label={t.history}>
    <p role="status">{accepted?t.accepted:t.acceptancePending}</p><p>{t.noAccept}</p>
    <dl className="goal-graph-facts"><dt>{t.plan}</dt><dd>{view.run.plan_revision}</dd><dt>{t.revision}</dt><dd>{view.run.revision}</dd><dt>{t.ledger}</dt><dd>{view.ledger_revision}</dd></dl>
    <ol className="goal-graph-nodes">{view.nodes.map(record=><li data-testid="goal-graph-node" data-node-id={record.node.id} data-plan-revision={record.plan_revision} key={JSON.stringify([record.plan_revision,record.node.id,record.attempt_id??record.job_id])}><details><summary>{record.node.id} · {graphGraphLabel(record.state,language)} · {t.plan} {record.plan_revision}</summary><p>{record.node.goal}</p>{record.node.dependencies.length>0&&<p>← {record.node.dependencies.join(' · ')}</p>}{record.blocked_reason&&<p>{record.blocked_reason}</p>}<dl className="goal-graph-facts"><dt>{t.attempt}</dt><dd>{record.attempt_id??'—'}</dd><dt>{t.session}</dt><dd>{record.session_id??'—'}</dd><dt>{t.stop}</dt><dd>{record.execution_kind==='owner'?t.noWorker:record.stop_confirmed?t.yes:t.no}</dd><dt>{t.actual}</dt><dd>{count(record.actual_runtime_ms)} ms</dd><dt>{t.output}</dt><dd>{count(record.actual_output_bytes)}</dd></dl><details><summary>{t.reserved}</summary><dl className="goal-graph-facts">{Object.entries(record.node.reservation).map(([key,value])=><div key={key}><dt>{key}</dt><dd>{value===null?t.notMeasured:value}</dd></div>)}</dl><p>{t.chargedHint}</p></details><p>{t.outputHint}</p><p>{t.cancelHint}</p></details></li>)}</ol>
    <details><summary>{t.candidate} · {view.candidates.length}</summary>{view.candidates.map(candidate=><article key={candidate.id}><h4>{candidate.node_id} · {candidate.id}</h4><p>{t.plan} {candidate.plan_revision} · {t.attempt} {candidate.attempt_id}</p>{candidate.artifacts.map(artifact=><GoalGraphArtifact key={artifact.artifact_id} value={artifact} language={language}/>)}<details><summary>{t.details}</summary><pre>{JSON.stringify(candidate,null,2)}</pre></details></article>)}</details>
    <details><summary>{t.validations} · {view.validations.length}</summary>{view.validations.map(check=><article key={check.id}><h4>{check.node_id} · {graphGraphLabel(check.state,language)}</h4><p>{check.scope} · {check.correctness}</p><dl className="goal-graph-facts"><dt>{t.definition}</dt><dd>{check.rule_id} · <code>{check.definition_digest}</code></dd><dt>{t.stop}</dt><dd>{check.stop_confirmed?t.yes:t.no}</dd></dl><p>{t.noAccept}</p><details><summary>{t.details}</summary><pre>{JSON.stringify(check,null,2)}</pre></details></article>)}</details>
    <details><summary>{t.decisions} · {view.owner_decisions.length}</summary>{view.owner_decisions.map(decision=><article key={decision.id}><h4>{decision.node_id} · {graphGraphLabel(decision.decision,language)}</h4><dl className="goal-graph-facts"><dt>{t.plan}</dt><dd>{decision.plan_revision}</dd><dt>{t.criterion}</dt><dd><code>{decision.criterion_digest}</code></dd></dl>{decision.candidate_refs.map(ref=><GoalGraphArtifact key={ref.candidate_id+':'+ref.artifact_id} value={ref} language={language}/>)}<details><summary>{t.details}</summary><pre>{JSON.stringify(decision,null,2)}</pre></details></article>)}</details>
  </section>;
}
