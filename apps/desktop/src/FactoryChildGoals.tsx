import { useCallback, useEffect, useRef, useState } from 'react';
import { api, DaemonError, taskAllowsExecution, type Task } from './api';
import { sessionApi, type ExecutionGrant } from './session-api';
import { factoryApi, factoryChildGoalApi, factoryChildPlanApi } from './factory-api';
import { factoryDetailsShape, factoryRunSupported, factorySame, factoryScopeShape, factorySafeFingerprint } from './factory-guards';
import { childContextShape, childViewShape, verifyChildRaw } from './factory-child-plan';
import { childGoalEvidenceContext, childGoalHistoryShape, childGoalOriginShape, childGoalRefSame, childGoalExecutionPlanReady, childGoalPlanMatchesGraph, childGoalSavedReferencesMatch, childGoalReferenceShape, childGoalScopeFingerprint, childGoalSummaryMeetsFloor, childGoalSummaryShape, childGoalViewShape } from './factory-child-goal';
import { goalGraphApi } from './goal-graph-api';
import { graphCanAdvanceView, graphCanReviewOwner, graphDecisionShape, graphDetailsMeetFloor, graphReceiptFitsFloor, graphRunCanCancel, graphRunPolling, graphViewConsistent } from './goal-graph-guards';
import { GoalGraphExecutionHistory } from './GoalGraphExecutionHistory';
import { graphGraphLabel } from './goal-graph-labels';
import { childGoalMessages } from './factory-child-goal-i18n';
import type { Language } from './i18n';
import type { GraphOwnerDecisionResult, GraphRunDetails } from './goal-graph-types';
import type { FactoryScope, FactoryTaskHandle, FactoryRun, FactoryRunDetails, FactoryChildPlanningContext, FactoryChildPlanView, FactoryChildGoalSummary, FactoryChildGoalView, FactoryChildGoalSubmission } from './factory-types';

type Props={scope:FactoryScope;target:FactoryTaskHandle;run:FactoryRun;selected:FactoryChildPlanView|null;language:Language;connected:boolean;parentReady:boolean;parentRefreshOnly?:boolean;parentReadIdentity?:string};
// Local read-state provenance only. It never enters an IPC approval/request.
const parentIdentity=(p:Props)=>factorySafeFingerprint([p.parentReadIdentity??'',p.scope,p.target,p.run,p.selected]);
const parentReadReady=(p:Props)=>p.parentReady||p.parentRefreshOnly===true;
type Receipt={runId:string;revision:number;decisionId?:string;approvalId?:string;decision?:'accepted'|'rejected'};
type Intent={kind:'start'|'cancel'|'advance'|'owner'|'reject';key:string;scopeId:string;taskId:string;factoryRunId:string;scopeRevision:number;goalRunId:string|null;goalRevision:number;request:FactoryChildGoalSubmission|null;nodeId:string|null;planRevision:number;planDigest:string;ledger:number;unknown:boolean;reconciled:boolean;receipt?:Receipt};
type Floor={summary:FactoryChildGoalSummary;details:GraphRunDetails|null;origin:string|null};
type Memory={active:number;pending:Intent|null;scope:string;scopeRevision:number;factoryRevision:number;taskRevision:number;taskFingerprint:string;floors:Map<string,Floor>};
const memories=new Map<string,Memory>();
const uncertain=(error:unknown)=>!(error instanceof DaemonError)||error.code==='OUTCOME_UNKNOWN'||!['none','not_started'].includes(error.sideEffectState);
type ReadSummary={scope:FactoryScope;family:FactoryRunDetails;target:FactoryTaskHandle;task:Task;rows:FactoryChildGoalSummary[];grant:ExecutionGrant|null};
// Narrowing needs a fresh existing-run summary, not artifact/Owner input reads.
// Only a validated, known start receipt can yield its UI acknowledgement slot.
function acknowledgedStart(op:Intent|null,read:ReadSummary,runId?:string):boolean {
 const receipt=op?.receipt,request=op?.request;
 if(!op||op.kind!=='start'||op.unknown||!receipt||!request||op.scopeId!==read.scope.id||op.taskId!==read.target.task_id||op.factoryRunId!==read.family.run.id||op.scopeRevision>read.scope.revision||request.task_id!==read.target.task_id||request.expected_factory_run_revision>read.family.run.revision)return false;
 const row=read.rows.find(item=>item.goal.run.id===receipt.runId);
 return !!row&&(!runId||runId===receipt.runId)&&row.goal.run.revision>=receipt.revision&&childGoalRefSame(row.plan,request.plan);
}
const cancelIdentity=(p:Props)=>factorySafeFingerprint([childGoalScopeFingerprint(p.scope),p.scope.project_id,p.target.task_id,p.target.root_task_id,p.target.parent_task_id,p.target.factory_scope_id,p.run.id,p.run.scope_id]);

type Snapshot={scope:FactoryScope;family:FactoryRunDetails;target:FactoryTaskHandle;task:Task;context:FactoryChildPlanningContext;head:FactoryChildPlanView|null;rows:FactoryChildGoalSummary[];view:FactoryChildGoalView|null;initial:FactoryChildPlanView|null;adopted:FactoryChildPlanView|null;grant:ExecutionGrant|null};
type PollSource={parent:string;snapshot:Snapshot};
type ReadonlyPollRetry={stamp:number;source:PollSource};
const exactReadonlyBusy=(error:unknown)=>error instanceof DaemonError&&error.code==='WORKSPACE_BUSY'&&error.retryable===true&&error.sideEffectState==='not_started';
const sameActivePollingGoal=(next:FactoryChildGoalSummary,known:FactoryChildGoalSummary)=>next.goal.run.id===known.goal.run.id&&graphRunPolling(next.goal)&&childGoalRefSame(next.plan,known.plan)&&next.goal.run.cancel_requested===known.goal.run.cancel_requested;
// Old evidence authorizes only the next bounded read. It cannot make a stale
// view fresh, establish a new run, or relax any existing write/Owner gate.
const snapshotPollingGoal=(v:Snapshot)=>!!v.view&&graphRunPolling(v.view.summary.goal)&&v.rows.some(row=>row.goal.run.id===v.view!.summary.goal.run.id&&graphRunPolling(row.goal));
const pollingSnapshotReady=(v:Snapshot,p:Props)=>parentReadReady(p)&&snapshotPollingGoal(v)&&!v.scope.revoked&&v.scope.expires_at_ms>Date.now()&&!!v.grant&&!v.grant.revoked&&v.grant.revision===v.scope.root_grant_revision&&v.grant.expires_at_ms>Date.now()&&!v.family.run.cancel_requested&&!['cancelled','completed','blocked','outcome_unknown'].includes(v.family.run.state)&&!v.target.cancel_requested&&taskAllowsExecution(v.task)&&v.task.spec_revision===v.context.goal.spec_revision;

export function FactoryChildGoals(props:Props){
 const t=childGoalMessages[props.language],[open,setOpen]=useState(false),[started,setStarted]=useState(false);
 if(!props.scope.selection.capabilities.child_goal_execution)return <p>{t.notAuthorized}</p>;
 return <details data-testid="factory-child-goals" onToggle={e=>{setOpen(e.currentTarget.open);if(e.currentTarget.open)setStarted(true);}}><summary>{t.title} · {props.target.task_id}</summary>{started&&<ChildGoalPanel key={`${props.scope.id}:${props.target.task_id}`} {...props} open={open}/>}</details>;
}
function ChildGoalPanel(props:Props&{open:boolean}){
 const {language,connected,open}=props,t=childGoalMessages[language],identity=`${props.scope.id}:${props.target.task_id}`;
 const [saved]=useState(()=>{const prior=memories.get(identity);if(prior)return prior;if(memories.size>=64){const disposable=[...memories].find(([,entry])=>entry.active===0&&entry.pending===null);if(disposable)memories.delete(disposable[0]);}const item:Memory={active:0,pending:null,scope:childGoalScopeFingerprint(props.scope),scopeRevision:props.scope.revision,factoryRevision:0,taskRevision:0,taskFingerprint:'',floors:new Map()};if(memories.size<64)memories.set(identity,item);return item;});
 const capacity=memories.get(identity)!==saved;
 const [summary,setSummary]=useState<ReadSummary|null>(null),[summaryFresh,setSummaryFresh]=useState(false),[data,setData]=useState<Snapshot|null>(null),[pending,setPending]=useState(saved.pending),[fresh,setFresh]=useState(false),[busy,setBusy]=useState(false),[loading,setLoading]=useState(false),[cancelReading,setCancelReading]=useState(false),[error,setError]=useState(''),[tick,setTick]=useState(0),[paused,setPaused]=useState(false);
 const summaryRead=useRef<{stamp:number;value:ReadSummary}|null>(null);
 const pollSource=useRef<PollSource|null>(null),readonlyPollRetry=useRef<ReadonlyPollRetry|null>(null),pollAuthority=useRef<{stamp:number;same:boolean|null}|null>(null);
 const current=useRef(props),mounted=useRef(true),epoch=useRef(0),writing=useRef(false),writeOwner=useRef<string|null>(null),cancelReadInFlight=useRef(false),reads=useRef(0),choice=useRef('');current.current=props;
 useEffect(()=>{mounted.current=true;++saved.active;return()=>{mounted.current=false;--saved.active;++epoch.current;};},[saved]);
 function remember(op:Intent|null){saved.pending=op;if(mounted.current)setPending(op);}
 const readSummary=useCallback(async(stamp:number):Promise<ReadSummary|null>=>{
   const p=current.current;
   const [scope,family,all,grants]=await Promise.all([factoryApi.get(p.scope.id),factoryApi.run(p.scope.id),api.snapshot(),sessionApi.grantsList(p.scope.project_id)]);
   if(!mounted.current||stamp!==epoch.current)return null;
   const root=all.tasks.find(task=>task.id===scope.root_task_id),task=all.tasks.find(task=>task.id===p.target.task_id);
   if(!root||!task||!factoryScopeShape(scope,root)||childGoalScopeFingerprint(scope)!==saved.scope||childGoalScopeFingerprint(p.scope)!==saved.scope||scope.revision<saved.scopeRevision||!factoryDetailsShape(family,root,scope.id,scope)||family.run.id!==p.run.id||family.run.revision<saved.factoryRevision||task.revision<saved.taskRevision||task.revision===saved.taskRevision&&saved.taskFingerprint!==JSON.stringify(task))throw new Error(t.invalid);
   const target=family.tasks.find(task=>task.task_id===p.target.task_id);
   if(!target||target.parent_task_id!==p.target.parent_task_id||target.root_task_id!==scope.root_task_id||!scope.bindings.child_goal_execution||!Array.isArray(grants))throw new Error(t.invalid);
   const source=pollSource.current,grant=grants.find(g=>g.id===scope.root_grant_id)??null;
   if(source&&pollAuthority.current?.stamp===stamp){
    const known=source.snapshot;
    pollAuthority.current.same=factorySame(scope,known.scope)&&factorySame(grant,known.grant)&&task.id===known.task.id&&task.project_id===known.task.project_id&&task.spec_revision===known.task.spec_revision&&task.goal===known.task.goal&&taskAllowsExecution(task)&&family.run.id===known.family.run.id&&!family.run.cancel_requested&&!['cancelled','completed','blocked','outcome_unknown'].includes(family.run.state)&&!target.cancel_requested;
   }
   const rows=await factoryChildGoalApi.list(scope.id,target.task_id);
   if(!childGoalHistoryShape(rows,scope,target,family,null,task))throw new Error(t.invalid);
   for(const [id,floor] of saved.floors){const row=rows.find(row=>row.goal.run.id===id);if(!row||!childGoalSummaryMeetsFloor(row,floor.summary))throw new Error(t.invalid);}
   if(source&&pollAuthority.current?.stamp===stamp){
    const known=source.snapshot.view,row=rows.find(item=>item.goal.run.id===known?.summary.goal.run.id);
    pollAuthority.current.same=pollAuthority.current.same===true&&!!known&&!!row&&sameActivePollingGoal(row,known.summary);
   }
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
   return trusted;
 },[saved,t]);
 const refresh=useCallback(async(manual=true):Promise<Snapshot|null>=>{
  if(manual){reads.current=0;setPaused(false);}const stamp=++epoch.current,parent=parentIdentity(current.current),source=pollSource.current;
  readonlyPollRetry.current=null;pollAuthority.current={stamp,same:null};setFresh(false);setSummaryFresh(false);setError('');
  if(!current.current.connected){setError(t.disconnected);return null;}setLoading(true);
  try{
   const trusted=await readSummary(stamp);if(!trusted)return null;
   const {scope,family,target,task,rows}=trusted;
   const context=await factoryChildPlanApi.context(scope.id,target.task_id);
   if(!mounted.current||stamp!==epoch.current)return null;
   if(!childContextShape(context,scope,target)||context.run_id!==family.run.id)throw new Error(t.invalid);
   if(source&&pollAuthority.current?.stamp===stamp)pollAuthority.current.same=pollAuthority.current.same===true&&factorySame(context,source.snapshot.context);
   let head:FactoryChildPlanView|null=null;
   try{head=await factoryChildPlanApi.get(scope.id,target.task_id);if(!mounted.current||stamp!==epoch.current)return null;if(!childViewShape(head,context,scope)||head.reference.revision!==context.current_plan_revision)throw new Error(t.invalid);await verifyChildRaw(head);}
   catch(e){if(!(context.current_plan_revision===0&&e instanceof DaemonError&&e.code==='NOT_FOUND'&&e.sideEffectState==='not_started'))throw e;}
   if(!mounted.current||stamp!==epoch.current)return null;
   if(source&&pollAuthority.current?.stamp===stamp)pollAuthority.current.same=pollAuthority.current.same===true&&factorySame(head,source.snapshot.head);
   const op=saved.pending,id=op?.receipt?.runId??op?.goalRunId??(rows.some(row=>row.goal.run.id===choice.current)?choice.current:rows.at(-1)?.goal.run.id);
   let view:FactoryChildGoalView|null=null,initial:FactoryChildPlanView|null=null,adopted:FactoryChildPlanView|null=null;
   if(id){
    view=await factoryChildGoalApi.get(scope.id,target.task_id,id);
    if(!mounted.current||stamp!==epoch.current)return null;
    const listed=rows.find(row=>row.goal.run.id===id),floor=saved.floors.get(id);
    if(!listed||!childGoalViewShape(view,scope,target,family,context,task)||!childGoalSummaryMeetsFloor(view.summary,listed)||floor?.origin&&floor.origin!==JSON.stringify(view.origin)||floor?.details&&!graphDetailsMeetFloor(view.details,{summary:floor.summary.goal,details:floor.details}))throw new Error(t.invalid);
    if(source&&pollAuthority.current?.stamp===stamp)pollAuthority.current.same=pollAuthority.current.same===true&&!!source.snapshot.view&&sameActivePollingGoal(view.summary,source.snapshot.view.summary);
    initial=await factoryChildPlanApi.get(scope.id,target.task_id,view.origin.initial_plan.revision);
    if(!mounted.current||stamp!==epoch.current)return null;
    if(source&&pollAuthority.current?.stamp===stamp)pollAuthority.current.same=pollAuthority.current.same===true&&factorySame(initial,source.snapshot.initial);
    adopted=childGoalRefSame(view.origin.initial_plan,view.summary.plan)?initial:await factoryChildPlanApi.get(scope.id,target.task_id,view.summary.plan.revision);
    for(const [plan,ref] of [[initial,view.origin.initial_plan],[adopted,view.summary.plan]] as const){if(!childViewShape(plan,context,scope)||!childGoalRefSame(plan.reference,ref))throw new Error(t.invalid);await verifyChildRaw(plan);}
    if(!childGoalSavedReferencesMatch(view,initial,adopted))throw new Error(t.invalid);
   }
   if(!mounted.current||stamp!==epoch.current)return null;
   const nextRows=rows.map(row=>row.goal.run.id===view?.summary.goal.run.id?view.summary:row);
   for(const row of nextRows){const before=saved.floors.get(row.goal.run.id);saved.floors.set(row.goal.run.id,{summary:structuredClone(row),details:view?.summary.goal.run.id===row.goal.run.id?structuredClone(view.details):before?.details??null,origin:view?.summary.goal.run.id===row.goal.run.id?JSON.stringify(view.origin):before?.origin??null});}
   saved.scopeRevision=scope.revision;saved.factoryRevision=family.run.revision;saved.taskRevision=task.revision;saved.taskFingerprint=JSON.stringify(task);choice.current=id??'';
   const snapshot:Snapshot={scope,family,target,task,context,head,rows:nextRows,view,initial,adopted,grant:trusted.grant};setData(snapshot);setFresh(true);
   pollSource.current=parentIdentity(current.current)===parent&&snapshotPollingGoal(snapshot)?{parent,snapshot}:null;
   if(op){
    const r=op.receipt,detail=view?.details;
    const decision=r?.decisionId?detail?.owner_decisions.find(item=>item.id===r.decisionId):null;
    const matches=!!r&&!!view&&view.summary.goal.run.id===r.runId&&view.summary.goal.run.revision>=r.revision&&(!r.decisionId||!!decision&&decision.run_id===r.runId&&decision.node_id===op.nodeId&&decision.plan_revision===op.planRevision&&decision.plan_digest===op.planDigest&&decision.approval_id===r.approvalId&&decision.decision===r.decision&&decision.source===(r.decision==='accepted'?'local_owner_confirmation':'local_owner_rejection'));
    if(matches)remember(null);else remember({...op,reconciled:true});
   }
   return snapshot;
  }catch(e){if(mounted.current&&stamp===epoch.current){
   const eligible=exactReadonlyBusy(e)&&source!==null&&pollSource.current===source&&source.parent===parent&&parentIdentity(current.current)===parent&&current.current.connected&&!saved.pending&&!writing.current&&!cancelReadInFlight.current&&pollAuthority.current?.stamp===stamp&&pollAuthority.current.same!==false&&pollingSnapshotReady(source.snapshot,current.current);
   readonlyPollRetry.current=eligible&&reads.current<20?{stamp,source}:null;if(!eligible)pollSource.current=null;
   if(eligible&&reads.current>=20)setPaused(true);
   setFresh(false);setError(`${t.invalid} ${e instanceof Error?e.message:''}`);
  }return null;}
  finally{if(mounted.current&&stamp===epoch.current){setLoading(false);setTick(n=>n+1);}}
 },[identity,connected,saved,t,readSummary]);
 useEffect(()=>{if(open)void refresh();},[open,refresh,props.selected?.reference.id,props.selected?.reference.revision]);
 const pollParent=parentIdentity(props);
 useEffect(()=>{
  const source=pollSource.current,retry=readonlyPollRetry.current,stamp=epoch.current,recovering=!fresh;
  if(!open||!connected||loading||busy||pending||!source||source.parent!==pollParent||!snapshotPollingGoal(source.snapshot)||recovering&&!(retry?.stamp===stamp&&retry.source===source&&pollingSnapshotReady(source.snapshot,current.current)))return;
  if(reads.current>=20){setPaused(true);return;}
  const timer=setTimeout(()=>{
   if(!mounted.current||stamp!==epoch.current||pollSource.current!==source||parentIdentity(current.current)!==pollParent||!current.current.connected||saved.pending||writing.current||cancelReadInFlight.current||recovering&&!pollingSnapshotReady(source.snapshot,current.current))return;
   ++reads.current;void refresh(false);
  },1000);return()=>clearTimeout(timer);
 },[open,connected,fresh,loading,busy,pending,data,tick,refresh,pollParent,props.parentReady,props.parentRefreshOnly]);
 const disabled=capacity||!connected||!fresh||busy||loading||cancelReading||!!pending;
 const checkedSummary=summaryRead.current;
 const cancelDisabled=capacity||!connected||!summaryFresh||busy||cancelReading||!checkedSummary||checkedSummary.stamp!==epoch.current||!!pending&&!acknowledgedStart(pending,checkedSummary.value);
 const live=(v:Snapshot)=>v.task.spec_revision===v.context.goal.spec_revision&&parentReadReady(current.current)&&!v.scope.revoked&&v.scope.expires_at_ms>Date.now()&&!!v.grant&&!v.grant.revoked&&v.grant.revision===v.scope.root_grant_revision&&v.grant.expires_at_ms>Date.now()&&v.grant.scope.max_children>0&&factoryRunSupported(v.family.run)&&!v.family.run.cancel_requested&&!['cancelled','outcome_unknown'].includes(v.family.run.state)&&!v.target.cancel_requested&&taskAllowsExecution(v.task);
 const selected=current.current.selected;
 const sameHead=!!data?.head&&!!selected&&childGoalRefSame(selected.reference,data.head.reference)&&selected.proposal.nodes.length<=(data.scope.bindings.child_goal_execution?.policy.max_nodes??0)&&childGoalExecutionPlanReady(selected,data.context);
 const canStart=!disabled&&!!data&&live(data)&&sameHead&&data.rows.length===0;
 const canAdvance=!disabled&&!!data?.view&&live(data)&&sameHead&&graphCanAdvanceView(data.view.details)&&selected!.reference.revision>data.view.summary.plan.revision;
 function intent(kind:Intent['kind'],v:Snapshot,plan:FactoryChildPlanView|null,nodeId:string|null=null):Intent{
  return {kind,key:crypto.randomUUID(),scopeId:v.scope.id,taskId:v.target.task_id,factoryRunId:v.family.run.id,scopeRevision:v.scope.revision,goalRunId:v.view?.summary.goal.run.id??null,goalRevision:v.view?.summary.goal.run.revision??0,request:plan?{task_id:v.target.task_id,plan:structuredClone(plan.reference),expected_factory_run_revision:v.family.run.revision}:null,nodeId,planRevision:v.view?.details.run.plan_revision??plan?.reference.revision??0,planDigest:v.view?.details.run.plan_digest??plan?.reference.plan_digest??'',ledger:v.view?.details.ledger_revision??0,unknown:false,reconciled:false};
 }
 async function send(op:Intent,v:Snapshot|ReadSummary){
  if(writing.current||capacity||!current.current.connected||saved.pending&&saved.pending.key!==op.key)return;
  writing.current=true;writeOwner.current=op.key;setBusy(true);setError('');++epoch.current;setFresh(false);remember({...op,reconciled:false});
  const releaseWrite=()=>{if(writeOwner.current===op.key){writeOwner.current=null;writing.current=false;if(mounted.current)setBusy(false);}};
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
   // A validated receipt ends the write. The remaining read must not block
   // stopping that exact acknowledged run or unlock a newer in-flight write.
   releaseWrite();
   if(mounted.current)await refresh();
  }catch(e){
   if(saved.pending?.key!==op.key)return;
   if(op.unknown||uncertain(e)){remember({...op,unknown:true,reconciled:false});if(mounted.current)setError(t.pending);}
   else{remember(null);if(mounted.current)setError(`${t.denied} ${e instanceof DaemonError?`${e.code} · ${e.sideEffectState} · `:''}${e instanceof Error?e.message:''}`);}
  }finally{releaseWrite();}
 }
 async function act(kind:Intent['kind'],nodeId:string|null=null){
  try{
  if(disabled||saved.pending)return;
  const chosen=current.current.selected,original=data?.view,parent=parentIdentity(current.current);const next=await refresh();if(!next||saved.pending||writing.current)return;
  if(parentIdentity(current.current)!==parent){setError(t.stale);return;}
  if(kind==='start'||kind==='advance'){
   if(!live(next)||!chosen||!current.current.selected||!childGoalRefSame(chosen.reference,current.current.selected.reference)||!next.head||!childGoalRefSame(chosen.reference,next.head.reference)||!childGoalReferenceShape(chosen.reference,next.scope,next.target,next.family)||!childViewShape(chosen,next.context,next.scope)||chosen.proposal.nodes.length>(next.scope.bindings.child_goal_execution?.policy.max_nodes??0)||!childGoalExecutionPlanReady(chosen,next.context)){setError(t.stale);return;}
   await verifyChildRaw(chosen);
   if(parentIdentity(current.current)!==parent||!live(next)){setError(t.stale);return;}
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
  const before=summaryRead.current;
  if(cancelDisabled||cancelReadInFlight.current||writing.current||!before||before.stamp!==epoch.current||saved.pending&&!acknowledgedStart(saved.pending,before.value,runId))return;
  const identityBefore=cancelIdentity(current.current),specBefore=before.value.task.spec_revision;
  const stamp=++epoch.current;cancelReadInFlight.current=true;setCancelReading(true);setFresh(false);setSummaryFresh(false);setLoading(true);setError('');
  try{
   const v=await readSummary(stamp);
   if(!v||!mounted.current||stamp!==epoch.current)return;
   if(!current.current.connected||cancelIdentity(current.current)!==identityBefore||v.task.spec_revision!==specBefore||writing.current||saved.pending&&!acknowledgedStart(saved.pending,v,runId)){setSummaryFresh(false);setError(t.stale);return;}
   const row=v.rows.find(item=>item.goal.run.id===runId);if(!row||!graphRunCanCancel(row.goal))return;
   // Only this known receipt was confirmed by this fresh exact-run summary.
   // Unknown writes, Owner decisions and other pending operations never yield.
   if(saved.pending)remember(null);
   setLoading(false);
   await send({kind:'cancel',key:crypto.randomUUID(),scopeId:v.scope.id,taskId:v.target.task_id,factoryRunId:v.family.run.id,scopeRevision:v.scope.revision,goalRunId:row.goal.run.id,goalRevision:row.goal.run.revision,request:null,nodeId:null,planRevision:row.goal.run.plan_revision,planDigest:row.goal.run.plan_digest,ledger:row.goal.ledger_revision,unknown:false,reconciled:false},v);
  }catch(e){if(mounted.current&&stamp===epoch.current){setSummaryFresh(false);setError(`${t.invalid} ${e instanceof Error?e.message:''}`);}}
  finally{cancelReadInFlight.current=false;if(mounted.current){setCancelReading(false);if(stamp===epoch.current)setLoading(false);}}
 }
 function recover(){
  const op=saved.pending;if(!op||!op.unknown||!op.reconciled||writing.current||cancelReadInFlight.current||capacity||!current.current.connected)return;
  if(op.kind==='cancel'){
   const trusted=summaryRead.current;if(summaryFresh&&trusted&&trusted.stamp===epoch.current)void send(op,trusted.value);
  }else if(fresh&&!loading&&data)void send(op,data);
 }
 const view=data?.view;
 // Read-only renderer diagnostics, never input to any action or authority check.
 return <section aria-label={t.title} data-testid="factory-child-goal-panel" aria-busy={busy||loading}
  data-cg-diagnostic="owner-ui-v4" data-cg-epoch={epoch.current} data-cg-fresh={fresh} data-cg-loading={loading}
  data-cg-pending-kind={pending?.kind??'none'} data-cg-pending-unknown={pending?String(pending.unknown):'none'}
  data-cg-capacity={capacity} data-cg-connected={connected} data-cg-busy={busy} data-cg-cancel-reading={cancelReading}
  data-cg-disabled={disabled} data-cg-has-view={!!view} data-cg-summary-fresh={summaryFresh}
  data-cg-parent-ready={props.parentReady} data-cg-parent-refresh-only={props.parentRefreshOnly===true}>
  <p>{t.separate}</p><p>{t.remaining}</p><p>{t.cancelHint}</p>{summary&&<p>{summary.scope.id} · r{summary.scope.revision} · {summary.scope.revoked?(language==='zh'?'已撤销':'revoked'):summary.scope.expires_at_ms<=Date.now()?(language==='zh'?'已到期':'expired'):(language==='zh'?'范围尚未到期':'scope not expired')} · {summary.scope.expires_at_ms}</p>}{capacity&&<p>{t.limited}</p>}
  <button type="button" disabled={!connected||busy||loading} onClick={()=>void refresh()}>{t.refresh}</button>
  {error&&<p role="alert">{error}</p>}{pending&&<><p role="status">{pending.unknown?t.pending:t.ack}</p>{pending.unknown&&<button type="button" disabled={!connected||busy||cancelReading||capacity||!pending.reconciled||(pending.kind==='cancel'?(!summaryFresh||!summary||!checkedSummary||checkedSummary.stamp!==epoch.current):(!fresh||loading||!data))} onClick={recover}>{t.recover}</button>}</>}{paused&&<p>{t.paused}</p>}
  <details><summary>{t.selected}</summary>{selected?<><pre>{JSON.stringify(selected.reference,null,2)}</pre><pre aria-label={t.raw}>{selected.plan_json}</pre></>:<p>{t.stale}</p>}</details>
  {data&&selected&&!childGoalExecutionPlanReady(selected,data.context)&&<p role="status">{t.incomplete}</p>}
  <button type="button" className="primary" disabled={!canStart} onClick={()=>void act('start')}>{t.start}</button><button type="button" disabled={!canAdvance} onClick={()=>void act('advance')}>{t.advance}</button>
  {summary&&!summary.rows.length&&<p>{t.empty}</p>}
  {summary&&<details open><summary>{t.history}</summary><ol>{summary.rows.map(row=><li key={row.goal.run.id}><span>{graphGraphLabel(row.goal.run.state,language)} · {row.goal.run.id} · r{row.goal.run.revision}</span><pre>{JSON.stringify(row.plan,null,2)}</pre><button type="button" disabled={disabled} onClick={()=>{choice.current=row.goal.run.id;void refresh();}}>{t.inspect}</button>{graphRunCanCancel(row.goal)&&<button type="button" disabled={cancelDisabled} onClick={()=>void cancel(row.goal.run.id)}>{t.cancel}</button>}</li>)}</ol></details>}
  {view&&data&&<>
   <details><summary>{t.scope}</summary><pre>{JSON.stringify({id:data.scope.id,approval_id:data.scope.approval_id,contract_digest:data.scope.contract_digest,selection:data.scope.selection},null,2)}</pre></details>
   <details><summary>{t.origin}</summary><dl className="goal-graph-facts"><dt>{t.approval}</dt><dd>{view.origin.original_native_approval_id}</dd><dt>{t.rootTask}</dt><dd>{view.origin.factory_root_task_id}</dd><dt>{t.parent}</dt><dd>{view.origin.parent_task_id??'—'} · {view.origin.ancestors.join(' ← ')}</dd><dt>{t.grantRef}</dt><dd>{view.origin.root_grant_id} · r{view.origin.root_grant_revision}</dd><dt>{t.digest}</dt><dd>{view.origin.factory_scope_digest} · {view.summary.execution_policy_digest}</dd></dl><pre>{JSON.stringify(view.origin,null,2)}</pre><details><summary>{t.binding}</summary><pre>{JSON.stringify(view.origin.execution_binding,null,2)}</pre></details></details>
   <details><summary>{t.initial}</summary><pre>{JSON.stringify(view.origin.initial_plan,null,2)}</pre><pre aria-label={t.initial}>{data.initial?.plan_json}</pre></details><details><summary>{t.adopted}</summary><pre>{JSON.stringify(view.summary.plan,null,2)}</pre><pre aria-label={t.adopted}>{data.adopted?.plan_json}</pre></details>
   {view.details.run.state==='failed'&&(data.scope.bindings.child_goal_execution?.policy.root_budget.reworks??0)>0&&<p>{t.failedGate}</p>}
   <GoalGraphExecutionHistory view={view.details} language={language} task={data.task} scope={childGoalEvidenceContext(view.summary,data.context)}/>
   <details><summary>{t.details}</summary><pre>{JSON.stringify({...view.details,validations:view.details.validations.map(item=>item.observation_json===undefined?item:(({observation:_derived,...rest})=>rest)(item))},null,2)}</pre></details>
   {view.details.nodes.filter(node=>node.plan_revision===view.details.run.plan_revision&&node.node.kind==='owner_approval'&&node.state==='awaiting_owner').map(node=><div className="goal-graph-actions" key={node.node.id}><span>{node.node.goal}</span><button type="button" disabled={disabled||!live(data)||!graphCanReviewOwner(view.details,node)} onClick={()=>void act('owner',node.node.id)} data-cg-owner-control="review">{t.owner}</button><button type="button" disabled={disabled||!live(data)||!graphCanReviewOwner(view.details,node)} onClick={()=>void act('reject',node.node.id)}>{t.reject}</button></div>)}
  </>}
  <p>{t.noReplay}</p><p>{t.notAccepted}</p>
 </section>;
}
