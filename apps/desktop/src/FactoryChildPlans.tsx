import { useCallback, useEffect, useRef, useState } from 'react';
import { DaemonError } from './api';
import type { Language } from './i18n';
import { FactoryChildPlanEditor } from './FactoryChildPlanEditor';
import { FactoryChildGoals } from './FactoryChildGoals';
import { factoryChildPlanApi } from './factory-api';
import { factorySame, factorySafeFingerprint, factoryRunSupported } from './factory-guards';
import { childContextShape, childEditorScope, childEditorLatest, childHistoryShape, childViewShape, parseFactoryChildPlan, verifyChildRaw } from './factory-child-plan';
import { childPlanMessages } from './factory-child-plan-i18n';
import type { FactoryScope, FactoryTaskHandle, FactoryRun, FactoryChildPlanRef, FactoryChildPlanView, FactoryChildPlanningContext } from './factory-types';
type Pending={raw:string;key:string;scopeRevision:number;planRevision:number;unknown:boolean};
type SubmissionNotice={kind:'not_sent'|'existing'|'unknown'}|{kind:'denied';text:string};
type Saved={pending:Pending|null;refs:Map<number,string>;scopeRevision:number;targetRevision:number;head:number;planId:string|null};
const memory=new Map<string,Saved>();
const uncertain=(e:unknown)=>!(e instanceof DaemonError)||e.code==='OUTCOME_UNKNOWN'||!['none','not_started'].includes(e.sideEffectState);
type Props={scope:FactoryScope;target:FactoryTaskHandle;run:FactoryRun;language:Language;connected:boolean;parentReady:boolean;parentRefreshOnly?:boolean;parentReadIdentity?:string;parentReadGeneration?:number};
// This remains a client gate only. Core resolves current permission and CAS.
function newSubmissionAllowed({scope,target,run,parentReady}:Pick<Props,'scope'|'target'|'run'|'parentReady'>){const now=Date.now(),grant=scope.bindings.approved_grant;return scope.selection.capabilities.child_planning&&!!scope.bindings.child_planning&&parentReady&&factoryRunSupported(run)&&['draft','queued','starting','running','blocked_on_live_parent','waiting_child','resuming','candidate','completed','stopping_for_handoff'].includes(target.state)&&!scope.revoked&&scope.expires_at_ms>now&&!grant.revoked&&grant.revision===scope.root_grant_revision&&grant.expires_at_ms>now&&!run.cancel_requested&&!['cancel_requested','cancelled','outcome_unknown'].includes(run.state)&&!target.cancel_requested&&!['cancel_requested','cancelled','outcome_unknown','failed','accepted'].includes(target.state);}
export function FactoryChildPlans(props:Props){const[open,setOpen]=useState(false),[started,setStarted]=useState(false),t=childPlanMessages[props.language];return <details onToggle={e=>{setOpen(e.currentTarget.open);if(e.currentTarget.open)setStarted(true);}}><summary>{t.title} · {props.target.task_id}</summary>{started&&<ChildPlanPanel key={`${props.scope.id}:${props.target.task_id}`} {...props} open={open}/>}</details>;}
function ChildPlanPanel({scope,target,run,language,connected,parentReady,parentRefreshOnly=false,parentReadIdentity='',parentReadGeneration=0,open}:Props&{open:boolean}){
 const t=childPlanMessages[language],identity=`${scope.id}:${target.task_id}`;
 const [saved]=useState(()=>{const prior=memory.get(identity);if(prior)return prior;const value:Saved={pending:null,refs:new Map(),scopeRevision:scope.revision,targetRevision:target.revision,head:0,planId:null};if(memory.size<64)memory.set(identity,value);return value;});
 const capacity=memory.get(identity)!==saved;
 // A new proof record publishes even when cached DTO/token values are equal.
 const [context,setContext]=useState<FactoryChildPlanningContext|null>(null),[history,setHistory]=useState<FactoryChildPlanRef[]>([]),[head,setHead]=useState<FactoryChildPlanView|null>(null),[selected,setSelected]=useState<FactoryChildPlanView|null>(null),[raw,setRaw]=useState(saved.pending?.raw??''),[base,setBase]=useState<number|null>(saved.pending?.planRevision??null),[pending,setPending]=useState<Pending|null>(saved.pending),[fresh,setFresh]=useState(false),[busy,setBusy]=useState(false),[loading,setLoading]=useState(false),[error,setError]=useState(''),[verifiedRead,setVerifiedRead]=useState<{token:string}>({token:''}),[notice,setNotice]=useState<SubmissionNotice|null>(saved.pending?{kind:'unknown'}:null);
 const sourceToken=factorySafeFingerprint([connected,parentReadGeneration,parentReadIdentity,scope,target,run]);
 const mounted=useRef(true),epoch=useRef(0),inflight=useRef(false),reading=useRef(false),trustedRead=useRef('');
 const current=useRef({scope,target,run,connected,parentReady,sourceToken,context,head,verifiedToken:verifiedRead.token,ready:false,loading,changed:false});
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;++epoch.current;};},[]);
 function remember(p:Pending|null,expectedKey?:string){if(expectedKey!==undefined&&saved.pending?.key!==expectedKey)return;saved.pending=p;if(mounted.current)setPending(p);}
 const refresh=useCallback(async()=>{
  const source=current.current,token=source.sourceToken,stamp=++epoch.current;current.current.ready=false;reading.current=true;setFresh(false);setError('');
  const stillCurrent=()=>mounted.current&&stamp===epoch.current&&token===current.current.sourceToken;
  if(!source.connected){reading.current=false;trustedRead.current='';setError(t.disconnected);setLoading(false);return;}setLoading(true);
  try{
   const {scope,target,run}=source;
   if(scope.revision<saved.scopeRevision||target.revision<saved.targetRevision)throw new Error(t.invalid);
   const c=await factoryChildPlanApi.context(scope.id,target.task_id);if(!stillCurrent())return;
   if(!childContextShape(c,scope,target)||c.run_id!==run.id)throw new Error(t.invalid);
   const refs=await factoryChildPlanApi.list(scope.id,target.task_id);if(!stillCurrent())return;
   if(!childHistoryShape(refs,c)||refs.length<saved.head)throw new Error(t.invalid);
   for(const ref of refs){const before=saved.refs.get(ref.revision);if(before&&before!==factorySafeFingerprint(ref))throw new Error(t.invalid);}
   let latest:FactoryChildPlanView|null=null;
   try{latest=await factoryChildPlanApi.get(scope.id,target.task_id);if(!stillCurrent())return;if(!refs.length||!childViewShape(latest,c,scope)||!factorySame(latest.reference,refs.at(-1)))throw new Error(t.invalid);await verifyChildRaw(latest);if(saved.planId!==null&&latest.proposal.plan_id!==saved.planId)throw new Error(t.invalid);}
   catch(e){if(!(refs.length===0&&e instanceof DaemonError&&e.code==='NOT_FOUND'&&e.sideEffectState==='not_started'))throw e;}
   if(!stillCurrent())return;
   saved.planId=latest?.proposal.plan_id??saved.planId;saved.head=refs.length;saved.scopeRevision=scope.revision;saved.targetRevision=target.revision;
   for(const ref of refs)saved.refs.set(ref.revision,factorySafeFingerprint(ref));
   trustedRead.current=factorySafeFingerprint([scope,target,run,c,latest]);
   setContext(c);setHistory(refs);setHead(latest);setSelected(latest);setVerifiedRead({token});setFresh(true);
  }catch(e){if(stillCurrent()){trustedRead.current='';setError(`${t.invalid} ${e instanceof Error?e.message:''}`);}}
  finally{if(mounted.current&&stamp===epoch.current){reading.current=false;setLoading(false);}}
 },[saved,t]);
 // Each original parent cycle reads history, including MCP-only plan changes.
 // Equal clones in the same cycle do not initiate another withdrawal/read.
 useEffect(()=>{if(open)void refresh();},[open,sourceToken,refresh]);
 const writable=newSubmissionAllowed({scope,target,run,parentReady});
 const readIdentity=factorySafeFingerprint([scope,target,run,context,head]);
 const childRefreshOnly=(parentReady||parentRefreshOnly)&&!capacity&&connected&&!busy&&!pending&&(fresh||loading)&&trustedRead.current===readIdentity&&(loading||parentRefreshOnly);
 const changed=!!context&&base!==null&&base!==context.current_plan_revision;
 const ready=fresh&&!reading.current&&verifiedRead.token===sourceToken&&trustedRead.current===readIdentity;
 current.current={scope,target,run,connected,parentReady,sourceToken,context,head,verifiedToken:verifiedRead.token,ready,loading,changed};
 const disabled=capacity||!connected||busy||loading||!ready||!!pending||!writable||changed;
 const noticeText=notice?.kind==='unknown'?t.pending:notice?.kind==='denied'?`${t.denied}: ${notice.text}`:notice?.kind==='existing'?(language==='zh'?'本次未新增提交。请先核对原提交的结果。':'No additional submission was sent. Check the existing submission’s outcome first.'):notice?.kind==='not_sent'?(language==='zh'?'本次未发送计划。请刷新记录并检查草稿后再提交。':'No submission was sent. Refresh the records and review the draft before submitting.'):'';
 async function select(ref:FactoryChildPlanRef){if(!context||loading||busy)return;const stamp=++epoch.current,token=current.current.sourceToken;reading.current=true;setLoading(true);try{const view=await factoryChildPlanApi.get(scope.id,target.task_id,ref.revision);if(!childViewShape(view,context,scope)||!factorySame(ref,view.reference)||view.proposal.plan_id!==head?.proposal.plan_id)throw new Error(t.invalid);await verifyChildRaw(view);if(mounted.current&&stamp===epoch.current&&token===current.current.sourceToken)setSelected(view);}catch(e){if(mounted.current&&stamp===epoch.current){trustedRead.current='';setFresh(false);setError(t.invalid);}}finally{if(mounted.current&&stamp===epoch.current){reading.current=false;setLoading(false);}}}
 async function submit(op:Pending){
  const source=current.current;
  // Recheck the live proof: read refs can publish before React state commits.
  if(inflight.current||reading.current||!source.connected||!source.ready||source.verifiedToken!==source.sourceToken||trustedRead.current!==factorySafeFingerprint([source.scope,source.target,source.run,source.context,source.head])||source.loading||capacity||!source.context||saved.pending!==null&&saved.pending.key!==op.key||(!op.unknown&&(!newSubmissionAllowed(source)||source.changed||op.scopeRevision!==source.scope.revision||op.planRevision!==source.context.current_plan_revision))){setNotice({kind:saved.pending?.unknown?'unknown':inflight.current||saved.pending?'existing':'not_sent'});return;}
  const {scope,target,context,head}=source;
  if(!op.unknown){try{parseFactoryChildPlan(op.raw,context,scope,head);}catch(e){setNotice({kind:'denied',text:`${e instanceof DaemonError?`${e.code} · ${e.sideEffectState} · `:''}${e instanceof Error?e.message:''}`});return;}}
  trustedRead.current='';inflight.current=true;remember({...op,unknown:false});setBusy(true);setError('');setNotice(null);
  try{
   const value=await factoryChildPlanApi.submit(scope.id,target.task_id,op.raw,op.planRevision,op.scopeRevision,op.key);
   if(!childViewShape(value,context,scope)||value.plan_json!==op.raw||value.reference.revision!==op.planRevision+1||value.caller_task_id!==target.task_id||value.caller_session_id!==null)throw new Error(t.invalid);
   await verifyChildRaw(value);if(saved.planId!==null&&value.proposal.plan_id!==saved.planId)throw new Error(t.invalid);
   const known=saved.refs.get(value.reference.revision);if(known&&known!==factorySafeFingerprint(value.reference))throw new Error(t.invalid);
   // An exact-key historical receipt clears only its request. Refresh retrieves
   // the latest head; an older receipt can never lower the observed revision.
   saved.planId=value.proposal.plan_id;saved.refs.set(value.reference.revision,factorySafeFingerprint(value.reference));saved.head=Math.max(saved.head,value.reference.revision);
   remember(null,op.key);setNotice(null);setRaw('');setBase(null);if(mounted.current)await refresh();
  }catch(e){if(op.unknown||uncertain(e)){remember({...op,unknown:true},op.key);if(mounted.current){setFresh(false);setNotice({kind:'unknown'});}}else{remember(null,op.key);if(mounted.current){setFresh(false);setNotice({kind:'denied',text:`${e instanceof DaemonError?`${e.code} · ${e.sideEffectState} · `:''}${e instanceof Error?e.message:''}`});}}}
  finally{inflight.current=false;if(mounted.current)setBusy(false);}
 }
 function apply(text:string){if(disabled||!context)return;setRaw(text);setBase(context.current_plan_revision);setNotice(null);}
 return <section aria-label={t.title}><p>{t.description}</p>{!writable&&<p>{t.readonly}</p>}<button disabled={!connected||busy||loading} onClick={()=>void refresh()}>{t.refresh}</button>{error&&<p role="alert">{error}</p>}{noticeText&&<p role="alert">{noticeText}</p>}{pending&&<><p>{t.pending}</p><button disabled={!connected||!ready||busy||loading||capacity} onClick={()=>void submit({...pending,unknown:true})}>{t.recover}</button></>}
 {context&&<><p>{t.head}: r{context.current_plan_revision} · {context.planning_policy_digest}</p><details><summary>{t.policy}</summary><pre>{JSON.stringify(context.policy,null,2)}</pre></details>
 <FactoryChildPlanEditor recipients={scope.selection.recipients} scope={childEditorScope(context,scope)} latest={childEditorLatest(head)} raw={raw} language={language} disabled={disabled} onApply={apply}/>
 <label>{t.raw}<textarea rows={12} value={raw} maxLength={context.policy.max_plan_bytes} disabled={disabled} onChange={e=>{setRaw(e.target.value);if(!saved.pending)setNotice(null);if(base===null)setBase(context.current_plan_revision);}}/></label><p>{t.local}</p>{changed&&<p role="alert">{t.staleDraft}</p>}<button disabled={disabled||!raw.trim()||context.current_plan_revision>=context.policy.max_revisions} onClick={()=>void submit({raw,key:crypto.randomUUID(),scopeRevision:scope.revision,planRevision:context.current_plan_revision,unknown:false})}>{t.store}</button><button disabled={busy||!!pending} onClick={()=>{setRaw('');setBase(null);setNotice(null);}}>{t.clear}</button>
 <details><summary>{t.history}</summary>{history.length?<ol>{history.map(ref=><li key={ref.id}><button disabled={busy||loading} onClick={()=>void select(ref)}>r{ref.revision} · {ref.plan_digest}</button><p>{ref.id} · seq {ref.receipt_sequence} · {ref.raw_sha256}</p></li>)}</ol>:<p>{t.none}</p>}</details>
 {selected&&<details><summary>{t.selected}: r{selected.reference.revision}</summary><p>{selected.created_at} · {selected.authority_kind}</p><details><summary>{t.receipt}</summary><pre>{JSON.stringify({reference:selected.reference,scope_digest:selected.scope_digest,planning_policy_digest:selected.planning_policy_digest,root_task_id:selected.root_task_id,root_grant_id:selected.root_grant_id,caller_task_id:selected.caller_task_id,caller_session_id:selected.caller_session_id,caller_attempt_id:selected.caller_attempt_id,caller_fencing_token:selected.caller_fencing_token},null,2)}</pre></details><pre aria-label={t.rawHistory}>{selected.plan_json}</pre></details>}</>}
 {scope.selection.capabilities.child_goal_execution&&<FactoryChildGoals scope={scope} target={target} run={run} selected={selected} language={language} connected={connected} parentReady={parentReady&&ready&&!busy&&!pending&&!loading} parentRefreshOnly={childRefreshOnly} parentReadIdentity={factorySafeFingerprint([parentReadIdentity,readIdentity])}/>}
 <p>{t.boundary}</p></section>;
}
