import { useCallback, useEffect, useRef, useState } from 'react';
import { DaemonError, type Task } from './api';
import { goalApi, safeInteger, type GoalPlan, type GoalScope } from './goal-api';
import { goalExecutionApi as api, goalRunShape, goalRunViewShape, goalRunFloor, goalRunMeetsFloor, goalRunNeedsReconciliation, goalRunCanCancel, goalRunPolling, goalRunStates, type GoalRun, type GoalRunView, type GoalRunSelection, type GoalPlanAdvance, type GoalRunFloor } from './goal-execution-api';
import { goalExecutionMessages } from './goal-execution-i18n';
import type { Language } from './i18n';
import './goal-execution.css';

type Props={task:Task;scope:GoalScope;latest:GoalPlan|null;language:Language;connected:boolean;approvalAllowed:boolean};
type Floors=[string,GoalRunFloor][];
type Pending={kind:'approve'|'cancel'|'advance';key:string;scopeId:string;rootGrantId:string;runId?:string;revision?:number;selection?:GoalRunSelection;advance?:GoalPlanAdvance;planDigest?:string;receipt?:{id:string;revision:number};floors:Floors;ledger?:number;uncertain:boolean;reconciled:boolean};
// UI-process recovery only. This is neither a durable outbox nor a background writer.
const intents=new Map<string,Pending>();
const uncertain=(failure:unknown)=>!(failure instanceof DaemonError)||failure.code==='OUTCOME_UNKNOWN'||!['none','not_started'].includes(failure.sideEffectState);
const mergeReceiptFloor=(floors:Map<string,GoalRunFloor>,run:GoalRun)=>{
  const previous=floors.get(run.id);
  if(!previous||run.revision>=previous.revision&&goalRunMeetsFloor(run,previous))floors.set(run.id,goalRunFloor(run));
};
const storedPlanInScope=(plan:GoalPlan|null,task:Task,scope:GoalScope)=>!!plan&&plan.scope_id===scope.id&&plan.state==='validated_proposal'&&plan.dispatched===false&&/^[a-f0-9]{64}$/.test(plan.digest)&&!!plan.proposal&&plan.proposal.schema_version===1&&safeInteger(plan.proposal.revision,1)&&plan.proposal.expected_revision===plan.proposal.revision-1&&plan.proposal.scope_revision===scope.authority_revision&&Array.isArray(plan.proposal.nodes)&&plan.proposal.nodes.length>0&&plan.proposal.nodes.length<=256&&!!plan.proposal.goal&&(['project_id','root_task_id','task_id','spec_revision','input_revision']as const).every(key=>plan.proposal.goal[key]===scope.goal[key])&&scope.goal.task_id===task.id&&scope.goal.project_id===task.project_id;
export function GoalExecution(props:Props){
  const [started,setStarted]=useState(false),[expanded,setExpanded]=useState(false);
  return <section className="goal-execution" data-testid="goal-execution"><details onToggle={event=>{const open=event.currentTarget.open;setExpanded(open);if(open)setStarted(true);}}><summary><h3>{goalExecutionMessages[props.language].title}</h3></summary>{started&&<ExecutionPanel key={`${props.task.id}:${props.scope.id}`} {...props} expanded={expanded}/>}</details></section>;
}
function ExecutionPanel({task,scope,latest,language,connected,approvalAllowed,expanded}:Props&{expanded:boolean}){
  const t=goalExecutionMessages[language],saved=intents.get(task.id);
  const [runs,setRuns]=useState<GoalRun[]>([]),[view,setView]=useState<GoalRunView|null>(null),[pending,setPending]=useState<Pending|null>(saved??null);
  const [fresh,setFresh]=useState(false),[loading,setLoading]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState(''),[paused,setPaused]=useState(false),[readVersion,setReadVersion]=useState(0);
  const mounted=useRef(true),sequence=useRef(0),writing=useRef(false),observed=useRef(new Map<string,GoalRunFloor>(saved?.floors??[])),selected=useRef(''),reads=useRef(0),ledger=useRef(saved?.ledger??0);
  const context=useRef({task,scope,latest,approvalAllowed});context.current={task,scope,latest,approvalAllowed};
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;++sequence.current;};},[]);
  const refresh=useCallback(async(manual=true)=>{
    if(manual){reads.current=0;setPaused(false);}setFresh(false);setError('');
    const cached=intents.get(task.id);
    if(cached){const blocked={...cached,reconciled:false};intents.set(task.id,blocked);setPending(blocked);}
    if(!connected){setError(t.disconnected);return;}
    if(cached&&cached.scopeId!==scope.id){setError(t.foreignPending);return;}
    const current=++sequence.current;setLoading(true);
    try{
      const rows=await api.list(task.id);
      if(!mounted.current||current!==sequence.current)return;
      if(!Array.isArray(rows)||rows.length>256||rows.some(row=>!goalRunShape(row,context.current.task))||new Set(rows.map(row=>row.id)).size!==rows.length)throw new Error(t.scope);
      const scoped=rows.filter(row=>row.scope_id===scope.id);
      if(scoped.some(row=>!goalRunShape(row,context.current.task,context.current.scope)))throw new Error(t.scope);
      const op=intents.get(task.id);
      ledger.current=Math.max(ledger.current,op?.ledger??0);
      for(const [id,floor]of op?.floors??[])if(!observed.current.has(id)||observed.current.get(id)!.revision<floor.revision)observed.current.set(id,floor);
      for(const [id,floor]of observed.current){const row=rows.find(row=>row.id===id);if(!row||!goalRunMeetsFloor(row,floor))throw new Error(t.scope);}
      if(op&&(op.kind==='cancel'||op.kind==='advance')){
        const target=scoped.find(row=>row.id===op.runId);
        if(!target||target.revision<op.revision!)throw new Error(t.scope);
      }
      if(op?.receipt&&!scoped.some(row=>row.id===op.receipt!.id&&row.revision>=op.receipt!.revision))throw new Error(t.scope);
      const inspectId=op?.receipt?.id??(op&&(op.kind==='cancel'||op.kind==='advance')?op.runId:undefined)??(scoped.some(row=>row.id===selected.current)?selected.current:scoped[0]?.id);
      let detail:GoalRunView|null=null;
      if(inspectId){
        detail=await api.get(inspectId);
        if(!mounted.current||current!==sequence.current)return;
        const listed=scoped.find(row=>row.id===inspectId);
        if(!listed||!goalRunViewShape(detail,context.current.task,context.current.scope)||detail.run.id!==inspectId||detail.ledger_revision<ledger.current||!goalRunMeetsFloor(detail.run,goalRunFloor(listed)))throw new Error(t.scope);
        const floor=observed.current.get(inspectId);if(floor&&!goalRunMeetsFloor(detail.run,floor))throw new Error(t.scope);
      }
      if(!mounted.current||current!==sequence.current)return;
      const currentRows=rows.map(row=>detail?.run.id===row.id?detail.run:row);
      for(const row of currentRows)observed.current.set(row.id,goalRunFloor(row));
      if(detail)ledger.current=detail.ledger_revision;
      selected.current=inspectId??'';setRuns(currentRows);setView(detail);setFresh(true);
      if(op){
        const receipt=op.receipt;
        const acknowledged=receipt?currentRows.find(row=>row.id===receipt.id&&row.revision>=receipt.revision):undefined;
        const cancelled=op.kind==='cancel'&&currentRows.some(row=>row.id===op.runId&&row.revision>op.revision!&&row.cancel_requested);
        if(acknowledged&&(op.kind!=='cancel'||acknowledged.cancel_requested)||cancelled){intents.delete(task.id);setPending(null);}
        else{const reconciled={...op,floors:[...observed.current],ledger:ledger.current,reconciled:true};intents.set(task.id,reconciled);setPending(reconciled);}
      }else setPending(null);
    }catch(failure){if(mounted.current&&current===sequence.current){setFresh(false);setError(failure instanceof Error?failure.message:t.error);}}
    finally{if(mounted.current&&current===sequence.current){setLoading(false);setReadVersion(value=>value+1);}}
  },[connected,task.id,scope.id,t.scope,t.error,t.disconnected,t.foreignPending]);
  useEffect(()=>{void refresh();},[refresh]);
  const active=runs.some(goalRunPolling);
  useEffect(()=>{
    if(!expanded||!connected||!fresh||loading||busy||pending||!active)return;
    if(reads.current>=20){setPaused(true);return;}
    const timer=setTimeout(()=>{++reads.current;void refresh(false);},500);return()=>clearTimeout(timer);
  },[expanded,connected,fresh,loading,busy,pending,active,readVersion,refresh]);
  const disabled=!connected||!fresh||loading||busy;
  const pendingForeign=!!pending&&pending.scopeId!==scope.id;
  const canRecover=!!pending&&pending.uncertain&&pending.reconciled&&!disabled&&!pendingForeign;
  const canApprove=!disabled&&!pending&&approvalAllowed&&storedPlanInScope(latest,task,scope)&&!runs.some(goalRunNeedsReconciliation);
  const canAdvanceView=(detail:GoalRunView)=>!detail.run.cancel_requested&&goalRunStates.includes(detail.run.state)&&!['outcome_unknown','cancelled','failed','cancel_requested'].includes(detail.run.state)&&!detail.nodes.some(record=>record.session_id!==null&&!record.stop_confirmed||['reserved','running','stopping','outcome_unknown'].includes(record.state));
  const canAdvance=!disabled&&!pending&&approvalAllowed&&!!view&&canAdvanceView(view)&&!!latest&&storedPlanInScope(latest,task,scope)&&latest.proposal.revision>view.run.plan_revision;
  async function mutate(op:Pending){
    if(!intents.has(task.id)&&intents.size>=32){if(mounted.current)setError(t.pendingLimit);return;}
    const stored={...op,ledger:Math.max(ledger.current,op.ledger??0),reconciled:false};intents.set(task.id,stored);if(mounted.current){setPending(stored);setError('');}++sequence.current;
    try{
      const result=op.kind==='approve'?await api.approve(op.selection!,op.key):op.kind==='advance'?await api.advancePlan(op.advance!,op.revision!,op.key):await api.cancel(op.runId!,op.revision!,op.key);
      if(!goalRunShape(result,task)||result.scope_id!==op.scopeId||result.root_grant_id!==op.rootGrantId||(op.kind==='approve'&&(result.plan_revision!==op.selection!.expected_plan_revision||result.plan_digest!==op.planDigest))||(op.kind==='advance'&&(result.id!==op.runId||result.revision<=op.revision!||result.plan_revision!==op.advance!.expected_plan_revision||result.plan_digest!==op.planDigest||result.cancel_requested))||(op.kind==='cancel'&&(result.id!==op.runId||result.revision<=op.revision!||!result.cancel_requested)))throw new Error(t.scope);
      const current=intents.get(task.id);if(current?.key!==op.key)return;
      mergeReceiptFloor(observed.current,result);
      const acknowledged={...current,receipt:{id:result.id,revision:result.revision},floors:[...observed.current],reconciled:false};intents.set(task.id,acknowledged);
      if(mounted.current){setPending(acknowledged);await refresh();}
    }catch(failure){
      const current=intents.get(task.id);if(current?.key!==op.key)return;
      if(op.uncertain||uncertain(failure)){
        const held={...op,floors:[...observed.current],ledger:ledger.current,uncertain:true,reconciled:false};intents.set(task.id,held);
        if(mounted.current){setPending(held);setFresh(false);setError(t.pending);}
      }else{
        intents.delete(task.id);
        if(mounted.current){setPending(null);setError(`${failure instanceof DaemonError&&failure.code==='PERMISSION_DENIED'?t.denied:t.error} ${failure instanceof Error?failure.message:''}`);if(failure instanceof DaemonError&&['INPUT_CHANGED','STALE_REVISION'].includes(failure.code))setFresh(false);}
      }
    }
  }
  async function approve(){
    if(!canApprove||writing.current||intents.has(task.id)||!latest)return;
    writing.current=true;setBusy(true);setError('');++sequence.current;
    const initial={scopeId:scope.id,revision:latest.proposal.revision,digest:latest.digest};
    try{
      const preview=await goalApi.preview(scope.id,latest.proposal.revision);
      const now=context.current;
      if(!mounted.current)return;
      if(!preview||!storedPlanInScope(now.latest,now.task,now.scope)||now.scope.id!==initial.scopeId||now.latest?.proposal.revision!==initial.revision||now.latest.digest!==initial.digest||!now.approvalAllowed||preview.scope_id!==initial.scopeId||preview.plan_id!==latest.proposal.plan_id||preview.plan_revision!==initial.revision||preview.read_only!==true||preview.dispatched!==false||preview.reservation_made!==false||!safeInteger(preview.journal_sequence,1)||preview.journal_sequence<ledger.current||(preview.decision!==null&&(preview.decision.plan_digest!==initial.digest||preview.decision.expected_plan_revision!==initial.revision||preview.decision.expected_ledger_revision!==preview.journal_sequence)))throw new Error(t.scope);
      ledger.current=preview.journal_sequence;
      const selection={scope_id:scope.id,expected_plan_revision:initial.revision,expected_ledger_revision:preview.journal_sequence};
      await mutate({kind:'approve',key:crypto.randomUUID(),scopeId:scope.id,rootGrantId:scope.root_grant_id,selection,planDigest:initial.digest,floors:[...observed.current],uncertain:false,reconciled:false});
    }catch(failure){if(mounted.current){setFresh(false);setError(failure instanceof Error?failure.message:t.error);}}
    finally{writing.current=false;if(mounted.current)setBusy(false);}
  }
  async function advance(){
    if(!canAdvance||!view||!latest||writing.current||intents.has(task.id))return;
    writing.current=true;setBusy(true);setError('');++sequence.current;
    const expected={runId:view.run.id,planRevision:latest.proposal.revision,digest:latest.digest};
    try{
      const detail=await api.get(expected.runId);
      if(!mounted.current)return;
      const now=context.current;
      if(!goalRunViewShape(detail,now.task,now.scope)||detail.run.id!==expected.runId||detail.ledger_revision<ledger.current||!goalRunMeetsFloor(detail.run,goalRunFloor(view.run))||!canAdvanceView(detail)||now.latest?.proposal.revision!==expected.planRevision||now.latest.digest!==expected.digest||!storedPlanInScope(now.latest,now.task,now.scope)||!now.approvalAllowed||detail.run.plan_revision>=expected.planRevision)throw new Error(t.scope);
      ledger.current=detail.ledger_revision;
      const selection={run_id:detail.run.id,expected_plan_revision:expected.planRevision,expected_ledger_revision:detail.ledger_revision};
      await mutate({kind:'advance',key:crypto.randomUUID(),scopeId:scope.id,rootGrantId:scope.root_grant_id,runId:detail.run.id,revision:detail.run.revision,advance:selection,planDigest:expected.digest,floors:[...observed.current],uncertain:false,reconciled:false});
    }catch(failure){if(mounted.current){setFresh(false);setError(failure instanceof Error?failure.message:t.error);}}
    finally{writing.current=false;if(mounted.current)setBusy(false);}
  }
  async function cancel(run:GoalRun){
    if(disabled||pending||writing.current||!goalRunCanCancel(run))return;
    writing.current=true;setBusy(true);
    try{await mutate({kind:'cancel',key:crypto.randomUUID(),scopeId:scope.id,rootGrantId:scope.root_grant_id,runId:run.id,revision:run.revision,floors:[...observed.current],uncertain:false,reconciled:false});}
    finally{writing.current=false;if(mounted.current)setBusy(false);}
  }
  async function recover(){
    if(!canRecover||!pending||writing.current)return;writing.current=true;setBusy(true);
    try{await mutate(pending);}finally{writing.current=false;if(mounted.current)setBusy(false);}
  }
  const label=(state:string)=>goalRunStates.includes(state)?t[state as 'queued']:t.unknown+': '+state;
  const count=(value:number|null)=>value===null?t.notMeasured:String(value);
  const selectedRuns=runs.filter(run=>run.scope_id===scope.id);
  return <div aria-busy={busy||loading}><p>{t.hint}</p><p>{t.advanceHint}</p><details><summary>{t.boundaryTitle}</summary><p>{t.boundaries}</p><p>{t.cancelHint}</p><p>{t.noAccept}</p></details>
    <div className="goal-execution-actions"><button type="button" onClick={()=>void refresh()} disabled={!connected||busy||loading}>{t.refresh}</button><button type="button" className="primary" onClick={()=>void approve()} disabled={!canApprove}>{t.approve}</button>{latest&&view&&latest.proposal.revision>view.run.plan_revision&&<button type="button" disabled={!canAdvance} onClick={()=>void advance()}>{t.advance}</button>}{pending?.uncertain&&<button type="button" disabled={!canRecover} onClick={()=>void recover()}>{t.recover}</button>}</div>
    {!approvalAllowed||!latest?.proposal.nodes.length?<p>{t.unavailable}</p>:null}{error&&<p role={expanded?'alert':undefined} className="error">{error}</p>}{pending&&<p role={expanded?'status':undefined}>{pending.uncertain?t.pending:t.acknowledged}</p>}{paused&&<p>{t.pollPaused}</p>}
    {!selectedRuns.length&&<p>{t.empty}</p>}{selectedRuns.map(run=><details key={run.id} open={view?.run.id===run.id}><summary>{label(run.state)} · {run.id}</summary><dl><dt>{t.run}</dt><dd>{run.id}</dd><dt>{t.plan}</dt><dd>{run.plan_revision}</dd><dt>{t.digest}</dt><dd><code>{run.plan_digest}</code></dd><dt>{t.revision}</dt><dd>{run.revision}</dd></dl><p>{t.noAccept}</p>{run.cancel_requested&&<p>{t.cancelHint}</p>}<div className="goal-execution-actions"><button type="button" disabled={disabled||!!pending} onClick={()=>{selected.current=run.id;void refresh();}}>{t.inspect}</button>{goalRunCanCancel(run)&&<button type="button" disabled={disabled||!!pending} onClick={()=>void cancel(run)}>{t.cancel}</button>}</div></details>)}
    {view&&<section aria-label={t.history}><h4>{t.history}</h4><p>{t.ledger}: {view.ledger_revision}</p><p>{t.measuredHint}</p><p>{t.outputHint}</p>{view.nodes.map(record=><details key={JSON.stringify([record.plan_revision,record.node.id,record.attempt_id])}><summary>{record.node.id} · {record.state} · {t.plan} {record.plan_revision}</summary><p>{record.node.goal}</p><dl><dt>{t.attempt}</dt><dd>{record.attempt_id??'—'}</dd><dt>{t.session}</dt><dd>{record.session_id??'—'}</dd><dt>{t.stop}</dt><dd>{record.stop_confirmed?t.yes:t.no}</dd></dl><div className="goal-execution-meter"><div><h4>{t.reserved}</h4><dl>{(['runtime_ms','output_bytes','tokens','cost_microunits']as const).map(key=><div key={key}><dt>{key==='runtime_ms'?t.runtime:key==='output_bytes'?t.output:key==='tokens'?t.tokens:t.cost}</dt><dd>{count(record.node.reservation[key])}</dd></div>)}</dl></div><div><h4>{t.actual}</h4><dl>{(['actual_runtime_ms','actual_output_bytes','tokens','cost_microunits']as const).map(key=><div key={key}><dt>{key==='actual_runtime_ms'?t.runtime:key==='actual_output_bytes'?t.selectedOutput:key==='tokens'?t.tokens:t.cost}</dt><dd>{count(record[key])}</dd></div>)}</dl></div></div></details>)}</section>}
  </div>;
}
