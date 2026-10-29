import { useCallback, useEffect, useRef, useState } from 'react';
import { api, DaemonError, taskAllowsExecution, type Task } from './api';
import { sessionApi, type ExecutionGrant } from './session-api';
import { factoryApi, factoryChildGoalApi, factoryChildPlanApi } from './factory-api';
import { factoryDetailsShape, factoryRunSupported, factorySame, factoryScopeShape } from './factory-guards';
import { childContextShape, childViewShape, verifyChildRaw } from './factory-child-plan';
import { childGoalEvidenceContext, childGoalHistoryShape, childGoalOriginShape, childGoalRefSame, childGoalPlanMatchesGraph, childGoalSavedReferencesMatch, childGoalReferenceShape, childGoalScopeFingerprint, childGoalSummaryMeetsFloor, childGoalSummaryShape, childGoalViewShape } from './factory-child-goal';
import { goalGraphApi } from './goal-graph-api';
import { graphCanAdvanceView, graphCanReviewOwner, graphDecisionShape, graphDetailsMeetFloor, graphReceiptFitsFloor, graphRunCanCancel, graphRunPolling, graphViewConsistent } from './goal-graph-guards';
import { GoalGraphExecutionHistory } from './GoalGraphExecutionHistory';
import { graphGraphLabel } from './goal-graph-labels';
import { childGoalMessages } from './factory-child-goal-i18n';
import type { Language } from './i18n';
import type { GraphOwnerDecisionResult, GraphRunDetails } from './goal-graph-types';
import type { FactoryScope, FactoryTaskHandle, FactoryRun, FactoryRunDetails, FactoryChildPlanningContext, FactoryChildPlanView, FactoryChildGoalSummary, FactoryChildGoalView, FactoryChildGoalSubmission } from './factory-types';

type Props={scope:FactoryScope;target:FactoryTaskHandle;run:FactoryRun;selected:FactoryChildPlanView|null;language:Language;connected:boolean;parentReady:boolean};
type Receipt={runId:string;revision:number;decisionId?:string;approvalId?:string;decision?:'accepted'|'rejected'};
type Intent={kind:'start'|'cancel'|'advance'|'owner'|'reject';key:string;scopeId:string;taskId:string;factoryRunId:string;scopeRevision:number;goalRunId:string|null;goalRevision:number;request:FactoryChildGoalSubmission|null;nodeId:string|null;planRevision:number;planDigest:string;ledger:number;unknown:boolean;reconciled:boolean;receipt?:Receipt};
type Floor={summary:FactoryChildGoalSummary;details:GraphRunDetails|null;origin:string|null};
type Memory={pending:Intent|null;scope:string;scopeRevision:number;factoryRevision:number;taskRevision:number;taskFingerprint:string;floors:Map<string,Floor>};
const memories=new Map<string,Memory>();
const uncertain=(error:unknown)=>!(error instanceof DaemonError)||error.code==='OUTCOME_UNKNOWN'||!['none','not_started'].includes(error.sideEffectState);
type ReadSummary={scope:FactoryScope;family:FactoryRunDetails;target:FactoryTaskHandle;task:Task;rows:FactoryChildGoalSummary[];grant:ExecutionGrant|null};
type Snapshot={scope:FactoryScope;family:FactoryRunDetails;target:FactoryTaskHandle;task:Task;context:FactoryChildPlanningContext;head:FactoryChildPlanView|null;rows:FactoryChildGoalSummary[];view:FactoryChildGoalView|null;initial:FactoryChildPlanView|null;adopted:FactoryChildPlanView|null;grant:ExecutionGrant|null};

export function FactoryChildGoals(props:Props){
 const t=childGoalMessages[props.language],[open,setOpen]=useState(false),[started,setStarted]=useState(false);
 if(!props.scope.selection.capabilities.child_goal_execution)return <p>{t.notAuthorized}</p>;
 return <details data-testid="factory-child-goals" onToggle={e=>{setOpen(e.currentTarget.open);if(e.currentTarget.open)setStarted(true);}}><summary>{t.title} · {props.target.task_id}</summary>{started&&<ChildGoalPanel key={`${props.scope.id}:${props.target.task_id}`} {...props} open={open}/>}</details>;
}
function ChildGoalPanel(props:Props&{open:boolean}){
 const {language,connected,open}=props,t=childGoalMessages[language],identity=`${props.scope.id}:${props.target.task_id}`;
 const [saved]=useState(()=>{const prior=memories.get(identity);if(prior)return prior;const item:Memory={pending:null,scope:childGoalScopeFingerprint(props.scope),scopeRevision:props.scope.revision,factoryRevision:0,taskRevision:0,taskFingerprint:'',floors:new Map()};if(memories.size<64)memories.set(identity,item);return item;});
 const capacity=memories.get(identity)!==saved;
 const [summary,setSummary]=useState<ReadSummary|null>(null),[summaryFresh,setSummaryFresh]=useState(false),[data,setData]=useState<Snapshot|null>(null),[pending,setPending]=useState(saved.pending),[fresh,setFresh]=useState(false),[busy,setBusy]=useState(false),[loading,setLoading]=useState(false),[error,setError]=useState(''),[tick,setTick]=useState(0),[paused,setPaused]=useState(false);
 const summaryRead=useRef<{stamp:number;value:ReadSummary}|null>(null);
 const current=useRef(props),mounted=useRef(true),epoch=useRef(0),writing=useRef(false),reads=useRef(0),choice=useRef('');current.current=props;
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;++epoch.current;};},[]);
 function remember(op:Intent|null){saved.pending=op;if(mounted.current)setPending(op);}
 const refresh=useCallback(async(manual=true):Promise<Snapshot|null>=>{
  if(manual){reads.current=0;setPaused(false);}const stamp=++epoch.current;setFresh(false);setSummaryFresh(false);setError('');
  if(!current.current.connected){setError(t.disconnected);return null;}setLoading(true);
  try{
   const p=current.current;
   const [scope,family,all,grants]=await Promise.all([factoryApi.get(p.scope.id),factoryApi.run(p.scope.id),api.snapshot(),sessionApi.grantsList(p.scope.project_id)]);
   if(!mounted.current||stamp!==epoch.current)return null;
   const root=all.tasks.find(task=>task.id===scope.root_task_id),task=all.tasks.find(task=>task.id===p.target.task_id);
   if(!root||!task||!factoryScopeShape(scope,root)||childGoalScopeFingerprint(scope)!==saved.scope||childGoalScopeFingerprint(p.scope)!==saved.scope||scope.revision<saved.scopeRevision||!factoryDetailsShape(family,root,scope.id)||family.run.id!==p.run.id||family.run.revision<saved.factoryRevision||task.revision<saved.taskRevision||task.revision===saved.taskRevision&&saved.taskFingerprint!==JSON.stringify(task))throw new Error(t.invalid);
   const target=family.tasks.find(task=>task.task_id===p.target.task_id);
   if(!target||target.parent_task_id!==p.target.parent_task_id||target.root_task_id!==scope.root_task_id||!scope.bindings.child_goal_execution||!Array.isArray(grants))throw new Error(t.invalid);
   const rows=await factoryChildGoalApi.list(scope.id,target.task_id);
   if(!childGoalHistoryShape(rows,scope,target,family,null,task))throw new Error(t.invalid);
   for(const [id,floor] of saved.floors){const row=rows.find(row=>row.goal.run.id===id);if(!row||!childGoalSummaryMeetsFloor(row,floor.summary))throw new Error(t.invalid);}
   if(!mounted.current||stamp!==epoch.current)return null;
   const trusted:ReadSummary={scope,family,target,task,rows,grant:grants.find(g=>g.id===scope.root_grant_id)??null};summaryRead.current={stamp,value:trusted};setSummary(trusted);setSummaryFresh(true);
   for(const row of rows){const floor=saved.floors.get(row.goal.run.id);saved.floors.set(row.goal.run.id,{summary:structuredClone(row),details:floor?.details??null,origin:floor?.origin??null});}
   saved.scopeRevision=scope.revision;saved.factoryRevision=family.run.revision;saved.taskRevision=task.revision;saved.taskFingerprint=JSON.stringify(task);
   const narrowing=saved.pending;
   if(narrowing?.kind==='cancel'){
    const receipt=narrowing.receipt,row=receipt&&rows.find(row=>row.goal.run.id===receipt.runId);
    if(receipt&&row&&row.goal.run.revision>=receipt.revision&&row.goal.run.cancel_requested)remember(null);
    else remember({...narrowing,reconciled:true});
   }
   const context=await factoryChildPlanApi.context(scope.id,target.task_id);
   if(!childContextShape(context,scope,target)||context.run_id!==family.run.id)throw new Error(t.invalid);
   let head:FactoryChildPlanView|null=null;
   try{head=await factoryChildPlanApi.get(scope.id,target.task_id);if(!childViewShape(head,context,scope)||head.reference.revision!==context.current_plan_revision)throw new Error(t.invalid);await verifyChildRaw(head);}
   catch(e){if(!(context.current_plan_revision===0&&e instanceof DaemonError&&e.code==='NOT_FOUND'&&e.sideEffectState==='not_started'))throw e;}
   const op=saved.pending,id=op?.receipt?.runId??op?.goalRunId??(rows.some(row=>row.goal.run.id===choice.current)?choice.current:rows.at(-1)?.goal.run.id);
   let view:FactoryChildGoalView|null=null,initial:FactoryChildPlanView|null=null,adopted:FactoryChildPlanView|null=null;
   if(id){
    view=await factoryChildGoalApi.get(scope.id,target.task_id,id);
    const listed=rows.find(row=>row.goal.run.id===id),floor=saved.floors.get(id);
    if(!listed||!childGoalViewShape(view,scope,target,family,context,task)||!childGoalSummaryMeetsFloor(view.summary,listed)||floor?.origin&&floor.origin!==JSON.stringify(view.origin)||floor?.details&&!graphDetailsMeetFloor(view.details,{summary:floor.summary.goal,details:floor.details}))throw new Error(t.invalid);
    initial=await factoryChildPlanApi.get(scope.id,target.task_id,view.origin.initial_plan.revision);
    adopted=childGoalRefSame(view.origin.initial_plan,view.summary.plan)?initial:await factoryChildPlanApi.get(scope.id,target.task_id,view.summary.plan.revision);
    for(const [plan,ref] of [[initial,view.origin.initial_plan],[adopted,view.summary.plan]] as const){if(!childViewShape(plan,context,scope)||!childGoalRefSame(plan.reference,ref))throw new Error(t.invalid);await verifyChildRaw(plan);}
    if(!childGoalSavedReferencesMatch(view,initial,adopted))throw new Error(t.invalid);
   }
   if(!mounted.current||stamp!==epoch.current)return null;
   const nextRows=rows.map(row=>row.goal.run.id===view?.summary.goal.run.id?view.summary:row);
   for(const row of nextRows){const before=saved.floors.get(row.goal.run.id);saved.floors.set(row.goal.run.id,{summary:structuredClone(row),details:view?.summary.goal.run.id===row.goal.run.id?structuredClone(view.details):before?.details??null,origin:view?.summary.goal.run.id===row.goal.run.id?JSON.stringify(view.origin):before?.origin??null});}
   saved.scopeRevision=scope.revision;saved.factoryRevision=family.run.revision;saved.taskRevision=task.revision;saved.taskFingerprint=JSON.stringify(task);choice.current=id??'';
   const snapshot:Snapshot={scope,family,target,task,context,head,rows:nextRows,view,initial,adopted,grant:grants.find(g=>g.id===scope.root_grant_id)??null};setData(snapshot);setFresh(true);
   if(op){
    const r=op.receipt,detail=view?.details;
    const decision=r?.decisionId?detail?.owner_decisions.find(item=>item.id===r.decisionId):null;
    const matches=!!r&&!!view&&view.summary.goal.run.id===r.runId&&view.summary.goal.run.revision>=r.revision&&(!r.decisionId||!!decision&&decision.run_id===r.runId&&decision.node_id===op.nodeId&&decision.plan_revision===op.planRevision&&decision.plan_digest===op.planDigest&&decision.approval_id===r.approvalId&&decision.decision===r.decision&&decision.source===(r.decision==='accepted'?'local_owner_confirmation':'local_owner_rejection'));
    if(matches)remember(null);else remember({...op,reconciled:true});
   }
   return snapshot;
  }catch(e){if(mounted.current&&stamp===epoch.current){setFresh(false);setError(`${t.invalid} ${e instanceof Error?e.message:''}`);}return null;}
  finally{if(mounted.current&&stamp===epoch.current){setLoading(false);setTick(n=>n+1);}}
 },[identity,connected,saved,t]);
 useEffect(()=>{if(open)void refresh();},[open,refresh,props.selected?.reference.id,props.selected?.reference.revision]);
 useEffect(()=>{if(!open||!connected||!fresh||loading||busy||pending||!data?.rows.some(row=>graphRunPolling(row.goal)))return;if(reads.current>=20){setPaused(true);return;}const timer=setTimeout(()=>{++reads.current;void refresh(false);},1000);return()=>clearTimeout(timer);},[open,connected,fresh,loading,busy,pending,data,tick,refresh]);
 const disabled=capacity||!connected||!fresh||busy||loading||!!pending;
 const cancelDisabled=capacity||!connected||!summaryFresh||busy||loading||!!pending;
 const live=(v:Snapshot)=>v.task.spec_revision===v.context.goal.spec_revision&&current.current.parentReady&&!v.scope.revoked&&v.scope.expires_at_ms>Date.now()&&!!v.grant&&!v.grant.revoked&&v.grant.revision===v.scope.root_grant_revision&&v.grant.expires_at_ms>Date.now()&&v.grant.scope.max_children>0&&factoryRunSupported(v.family.run)&&!v.family.run.cancel_requested&&!['cancelled','outcome_unknown'].includes(v.family.run.state)&&!v.target.cancel_requested&&taskAllowsExecution(v.task);
 const selected=current.current.selected;
 const sameHead=!!data?.head&&!!selected&&childGoalRefSame(selected.reference,data.head.reference)&&selected.proposal.nodes.length<=(data.scope.bindings.child_goal_execution?.policy.max_nodes??0);
 const canStart=!disabled&&!!data&&live(data)&&sameHead&&data.rows.length===0;
 const canAdvance=!disabled&&!!data?.view&&live(data)&&sameHead&&graphCanAdvanceView(data.view.details)&&selected!.reference.revision>data.view.summary.plan.revision;
 function intent(kind:Intent['kind'],v:Snapshot,plan:FactoryChildPlanView|null,nodeId:string|null=null):Intent{
  return {kind,key:crypto.randomUUID(),scopeId:v.scope.id,taskId:v.target.task_id,factoryRunId:v.family.run.id,scopeRevision:v.scope.revision,goalRunId:v.view?.summary.goal.run.id??null,goalRevision:v.view?.summary.goal.run.revision??0,request:plan?{task_id:v.target.task_id,plan:structuredClone(plan.reference),expected_factory_run_revision:v.family.run.revision}:null,nodeId,planRevision:v.view?.details.run.plan_revision??plan?.reference.revision??0,planDigest:v.view?.details.run.plan_digest??plan?.reference.plan_digest??'',ledger:v.view?.details.ledger_revision??0,unknown:false,reconciled:false};
 }
 async function send(op:Intent,v:Snapshot|ReadSummary){
  if(writing.current||capacity||!current.current.connected||saved.pending&&saved.pending.key!==op.key)return;
  writing.current=true;setBusy(true);setError('');++epoch.current;setFresh(false);remember({...op,reconciled:false});
  try{
   let result:FactoryChildGoalSummary|GraphOwnerDecisionResult;
   if(op.kind==='owner'||op.kind==='reject'){
    const args={run_id:op.goalRunId!,node_id:op.nodeId!,expected_plan_revision:op.planRevision,expected_ledger_revision:op.ledger};
    result=await (op.kind==='owner'?goalGraphApi.approveNode(args,op.goalRevision,op.key):goalGraphApi.rejectNode(args,op.goalRevision,op.key));
    if(!graphDecisionShape(result,op.goalRunId!,op.nodeId!,op.planRevision,op.goalRevision,op.kind==='owner'?'accepted':'rejected'))throw new Error(t.invalid);
    remember({...op,reconciled:false,receipt:{runId:result.run_id,revision:result.run_revision,decisionId:result.owner_decision_id,approvalId:result.approval_id,decision:op.kind==='owner'?'accepted':'rejected'}});
   }else{
    result=op.kind==='start'?await factoryChildGoalApi.start(op.scopeId,op.request!,op.scopeRevision,op.key):op.kind==='advance'?await factoryChildGoalApi.advancePlan(op.scopeId,op.goalRunId!,op.request!,op.goalRevision,op.scopeRevision,op.key):await factoryChildGoalApi.cancel(op.scopeId,op.taskId,op.goalRunId!,op.goalRevision,op.scopeRevision,op.key);
    if(!childGoalSummaryShape(result,v.scope,v.target,v.family,'context' in v?v.context:null,v.task)||op.request&&!childGoalRefSame(result.plan,op.request.plan)||op.kind!=='start'&&(result.goal.run.id!==op.goalRunId||result.goal.run.revision<=op.goalRevision)||op.kind==='cancel'&&!result.goal.run.cancel_requested)throw new Error(t.invalid);
    const before=saved.floors.get(result.goal.run.id);if(before&&!graphReceiptFitsFloor(result.goal,before.summary.goal,op.unknown))throw new Error(t.invalid);
    if(!before||childGoalSummaryMeetsFloor(result,before.summary))saved.floors.set(result.goal.run.id,{summary:structuredClone(result),details:before?.details??null,origin:before?.origin??null});
    remember({...op,reconciled:false,receipt:{runId:result.goal.run.id,revision:result.goal.run.revision}});
   }
   if(mounted.current)await refresh();
  }catch(e){
   if(saved.pending?.key!==op.key)return;
   if(op.unknown||uncertain(e)){remember({...op,unknown:true,reconciled:false});if(mounted.current)setError(t.pending);}
   else{remember(null);if(mounted.current)setError(`${t.denied} ${e instanceof DaemonError?`${e.code} · ${e.sideEffectState} · `:''}${e instanceof Error?e.message:''}`);}
  }finally{writing.current=false;if(mounted.current)setBusy(false);}
 }
 async function act(kind:Intent['kind'],nodeId:string|null=null){
  try{
  if(disabled||saved.pending)return;
  const chosen=current.current.selected,original=data?.view;const next=await refresh();if(!next||saved.pending||writing.current)return;
  if(kind==='start'||kind==='advance'){
   if(!live(next)||!chosen||!current.current.selected||!childGoalRefSame(chosen.reference,current.current.selected.reference)||!next.head||!childGoalRefSame(chosen.reference,next.head.reference)||!childGoalReferenceShape(chosen.reference,next.scope,next.target,next.family)||!childViewShape(chosen,next.context,next.scope)||chosen.proposal.nodes.length>(next.scope.bindings.child_goal_execution?.policy.max_nodes??0)){setError(t.stale);return;}
   await verifyChildRaw(chosen);
   if(kind==='start'&&next.rows.length!==0||kind==='advance'&&(!next.view||!graphCanAdvanceView(next.view.details)||chosen.reference.revision<=next.view.summary.plan.revision))return;
   await send(intent(kind,next,chosen),next);
  }else if(kind==='cancel'){
   if(!next.view||!graphRunCanCancel(next.view.summary.goal))return;await send(intent(kind,next,null),next);
  }else{
   if(!live(next)||!next.view||!original||next.view.details.run.id!==original.details.run.id||next.view.details.run.plan_revision!==original.details.run.plan_revision||next.view.details.run.plan_digest!==original.details.run.plan_digest)return;
   const node=next.view.details.nodes.find(item=>item.node.id===nodeId&&item.plan_revision===next.view!.details.run.plan_revision);
   if(!node||!graphCanReviewOwner(next.view.details,node)||!graphViewConsistent(next.view.details))return;
   await send(intent(kind,next,null,nodeId),next);
  }
  }catch(e){if(mounted.current){setFresh(false);setError(`${t.invalid} ${e instanceof Error?e.message:''}`);}}
 }
 async function cancel(runId:string){
  if(cancelDisabled||saved.pending)return;
  const stamp=epoch.current+1;await refresh();const trusted=summaryRead.current;
  if(!trusted||trusted.stamp!==stamp||epoch.current!==stamp||saved.pending||writing.current)return;
  const v=trusted.value,row=v.rows.find(item=>item.goal.run.id===runId);if(!row||!graphRunCanCancel(row.goal))return;
  await send({kind:'cancel',key:crypto.randomUUID(),scopeId:v.scope.id,taskId:v.target.task_id,factoryRunId:v.family.run.id,scopeRevision:v.scope.revision,goalRunId:row.goal.run.id,goalRevision:row.goal.run.revision,request:null,nodeId:null,planRevision:row.goal.run.plan_revision,planDigest:row.goal.run.plan_digest,ledger:row.goal.ledger_revision,unknown:false,reconciled:false},v);
 }
 const view=data?.view;
 return <section aria-label={t.title} data-testid="factory-child-goal-panel" aria-busy={busy||loading}>
  <p>{t.separate}</p><p>{t.remaining}</p><p>{t.cancelHint}</p>{summary&&<p>{summary.scope.id} · r{summary.scope.revision} · {summary.scope.revoked?(language==='zh'?'已撤销':'revoked'):summary.scope.expires_at_ms<=Date.now()?(language==='zh'?'已到期':'expired'):(language==='zh'?'范围尚未到期':'scope not expired')} · {summary.scope.expires_at_ms}</p>}{capacity&&<p>{t.limited}</p>}
  <button type="button" disabled={!connected||busy||loading} onClick={()=>void refresh()}>{t.refresh}</button>
  {error&&<p role="alert">{error}</p>}{pending&&<><p role="status">{pending.unknown?t.pending:t.ack}</p>{pending.unknown&&<button type="button" disabled={!connected||!(pending.kind==='cancel'?summaryFresh:fresh)||loading||busy||capacity||!pending.reconciled||!(pending.kind==='cancel'?summary:data)} onClick={()=>{const proof=pending.kind==='cancel'?summary:data;if(proof)void send(pending,proof);}}>{t.recover}</button>}</>}{paused&&<p>{t.paused}</p>}
  <details><summary>{t.selected}</summary>{selected?<><pre>{JSON.stringify(selected.reference,null,2)}</pre><pre aria-label={t.raw}>{selected.plan_json}</pre></>:<p>{t.stale}</p>}</details>
  <button type="button" className="primary" disabled={!canStart} onClick={()=>void act('start')}>{t.start}</button><button type="button" disabled={!canAdvance} onClick={()=>void act('advance')}>{t.advance}</button>
  {summary&&!summary.rows.length&&<p>{t.empty}</p>}
  {summary&&<details open><summary>{t.history}</summary><ol>{summary.rows.map(row=><li key={row.goal.run.id}><span>{graphGraphLabel(row.goal.run.state,language)} · {row.goal.run.id} · r{row.goal.run.revision}</span><pre>{JSON.stringify(row.plan,null,2)}</pre><button type="button" disabled={disabled} onClick={()=>{choice.current=row.goal.run.id;void refresh();}}>{t.inspect}</button>{graphRunCanCancel(row.goal)&&<button type="button" disabled={cancelDisabled} onClick={()=>void cancel(row.goal.run.id)}>{t.cancel}</button>}</li>)}</ol></details>}
  {view&&data&&<>
   <details><summary>{t.scope}</summary><pre>{JSON.stringify({id:data.scope.id,approval_id:data.scope.approval_id,contract_digest:data.scope.contract_digest,selection:data.scope.selection},null,2)}</pre></details>
   <details><summary>{t.origin}</summary><dl className="goal-graph-facts"><dt>{t.approval}</dt><dd>{view.origin.original_native_approval_id}</dd><dt>{t.rootTask}</dt><dd>{view.origin.factory_root_task_id}</dd><dt>{t.parent}</dt><dd>{view.origin.parent_task_id??'—'} · {view.origin.ancestors.join(' ← ')}</dd><dt>{t.grantRef}</dt><dd>{view.origin.root_grant_id} · r{view.origin.root_grant_revision}</dd><dt>{t.digest}</dt><dd>{view.origin.factory_scope_digest} · {view.summary.execution_policy_digest}</dd></dl><pre>{JSON.stringify(view.origin,null,2)}</pre><details><summary>{t.binding}</summary><pre>{JSON.stringify(view.origin.execution_binding,null,2)}</pre></details></details>
   <details><summary>{t.initial}</summary><pre>{JSON.stringify(view.origin.initial_plan,null,2)}</pre><pre aria-label={t.initial}>{data.initial?.plan_json}</pre></details><details><summary>{t.adopted}</summary><pre>{JSON.stringify(view.summary.plan,null,2)}</pre><pre aria-label={t.adopted}>{data.adopted?.plan_json}</pre></details>
   <GoalGraphExecutionHistory view={view.details} language={language} task={data.task} scope={childGoalEvidenceContext(view.summary,data.context)}/>
   <details><summary>{t.details}</summary><pre>{JSON.stringify({...view.details,validations:view.details.validations.map(item=>item.observation_json===undefined?item:(({observation:_derived,...rest})=>rest)(item))},null,2)}</pre></details>
   {view.details.nodes.filter(node=>node.plan_revision===view.details.run.plan_revision&&node.node.kind==='owner_approval'&&node.state==='awaiting_owner').map(node=><div className="goal-graph-actions" key={node.node.id}><span>{node.node.goal}</span><button type="button" disabled={disabled||!live(data)||!graphCanReviewOwner(view.details,node)} onClick={()=>void act('owner',node.node.id)}>{t.owner}</button><button type="button" disabled={disabled||!live(data)||!graphCanReviewOwner(view.details,node)} onClick={()=>void act('reject',node.node.id)}>{t.reject}</button></div>)}
  </>}
  <p>{t.noReplay}</p><p>{t.notAccepted}</p>
 </section>;
}
