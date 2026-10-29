import { useCallback, useEffect, useRef, useState, type FormEvent } from 'react';
import { api, DaemonError, type Task, type ManagedWorkspace } from './api';
import { sessionApi, type ExecutionGrant, type NamedConnection } from './session-api';
import type { Language } from './i18n';
import { factoryApi } from './factory-api';
import type { FactoryScope, FactoryRun, FactoryRunDetails, FactorySelection } from './factory-types';
import { factoryCanCancel, factoryDetailsShape, factoryGrantShape, factoryRunShape, factoryRunSupported, factorySame, factoryScopeShape, factoryTaskEligible, factorySafeFingerprint, factorySelectionSupported } from './factory-guards';
import { factoryMessages } from './factory-i18n';
import { newFactoryForm, parseFactorySelection, type FactoryForm, type FactoryBudgetForm } from './factory-form';
import { FactoryChildPlans } from './FactoryChildPlans';
import { FactoryChildGoalPolicyForm } from './FactoryChildGoalPolicyForm';
import { childPlanMessages } from './factory-child-plan-i18n';
import './factory-runtime.css';
type Props={task:Task;language:Language;connected:boolean};
type Pending={kind:'approve'|'start'|'cancel'|'revoke';key:string;scopeId:string;runId:string|null;revision:number|null;selection:FactorySelection|null;unknown:boolean};
type Floor={revision:number;fingerprint:string};
type RunFloor=Floor&{runId:string};
type ScopeFloor=Floor&{immutable:string;revoked:boolean};
type Memory={active:number;readOnly:boolean;pending:Pending|null;scopes:Map<string,ScopeFloor>;runs:Map<string,RunFloor>};
const memory=new Map<string,Memory>();
function recall(id:string):Memory{
 let value=memory.get(id);
 if(!value){
  if(memory.size>=64){const disposable=[...memory].find(([,entry])=>entry.active===0&&entry.pending===null);if(disposable)memory.delete(disposable[0]);}
  value={active:0,readOnly:memory.size>=64,pending:null,scopes:new Map(),runs:new Map()};
  if(!value.readOnly)memory.set(id,value);
 }
 return value;
}

const uncertain=(error:unknown)=>!(error instanceof DaemonError)||error.code==='OUTCOME_UNKNOWN'||!['none','not_started'].includes(error.sideEffectState);
const scopeImmutable=(s:FactoryScope)=>factorySafeFingerprint([s.id,s.root_task_id,s.project_id,s.root_grant_id,s.root_grant_revision,s.contract_digest,s.expires_at_ms,s.selection,s.bindings.project.id,s.bindings.project.revision,s.bindings.project.archived,s.bindings.child_planning?.definition_digest??null,s.bindings.child_goal_execution??null]);
const scopeFingerprint=(s:FactoryScope)=>factorySafeFingerprint([scopeImmutable(s),s.revoked]);
function advanceScopeFloor(map:Map<string,ScopeFloor>,value:FactoryScope,historical=false){
 const previous=map.get(value.id),immutable=scopeImmutable(value),fingerprint=scopeFingerprint(value);
 if(previous&&(previous.immutable!==immutable||previous.revoked&&!value.revoked&&value.revision>=previous.revision||value.revision===previous.revision&&fingerprint!==previous.fingerprint||value.revision<previous.revision&&!historical))throw new Error('Foreign, stale or forked Factory scope');
 if(!previous||value.revision>previous.revision)map.set(value.id,{revision:value.revision,fingerprint,immutable,revoked:value.revoked});
}
function advanceRunFloor(map:Map<string,RunFloor>,value:FactoryRun,historical=false){
 const previous=map.get(value.scope_id),fingerprint=factorySafeFingerprint(value);
 if(previous&&(previous.runId!==value.id||value.revision===previous.revision&&fingerprint!==previous.fingerprint||value.revision<previous.revision&&!historical))throw new Error('Foreign, stale or forked Factory run');
 if(!previous||value.revision>previous.revision)map.set(value.scope_id,{revision:value.revision,fingerprint,runId:value.id});
}

export function FactoryRuntime(props:Props){const [open,setOpen]=useState(false),[started,setStarted]=useState(false);return <section className="factory-runtime" data-testid="factory-runtime"><details onToggle={e=>{setOpen(e.currentTarget.open);if(e.currentTarget.open)setStarted(true);}}><summary><h2>{factoryMessages[props.language].title}</h2></summary>{started&&(props.task.project_id==='local'?<p>{factoryMessages[props.language].personal}</p>:<FactoryPanel key={props.task.id} {...props} open={open}/>)}</details></section>;}
function FactoryPanel({task,language,connected,open}:Props&{open:boolean}){
 const t=factoryMessages[language],cp=childPlanMessages[language];const [saved]=useState(()=>recall(task.id));
 const [form,setForm]=useState<FactoryForm>(newFactoryForm),[grants,setGrants]=useState<ExecutionGrant[]>([]),[connections,setConnections]=useState<NamedConnection[]>([]),[workspaces,setWorkspaces]=useState<ManagedWorkspace[]>([]),[scopes,setScopes]=useState<FactoryScope[]>([]),[scopeId,setScopeId]=useState(saved.pending?.scopeId??''),[detail,setDetail]=useState<FactoryRunDetails|null>(null),[run,setRun]=useState<FactoryRun|null>(null),[pending,setPending]=useState<Pending|null>(saved.pending);
 const [fresh,setFresh]=useState(false),[runFresh,setRunFresh]=useState(false),[noUnresolvedRun,setNoUnresolvedRun]=useState(false),[loading,setLoading]=useState(false),[paused,setPaused]=useState(false),[readTick,setReadTick]=useState(0),[busy,setBusy]=useState(false),[error,setError]=useState('');
 const mounted=useRef(true),epoch=useRef(0),inFlight=useRef(false),automaticReads=useRef(0),scopeChoice=useRef(scopeId),trustedRead=useRef('');scopeChoice.current=scopeId;
 useEffect(()=>{mounted.current=true;++saved.active;return()=>{mounted.current=false;--saved.active;++epoch.current;};},[saved]);
 const refresh=useCallback(async(manual=true)=>{
  if(manual){automaticReads.current=0;setPaused(false);}
  setFresh(false);setRunFresh(false);setNoUnresolvedRun(false);
  if(!connected){trustedRead.current='';setError(t.disconnected);return;}
  const request=++epoch.current;setLoading(true);setError('');
  try{
   const [g,c,w,s]=await Promise.all([sessionApi.grantsList(task.project_id!),sessionApi.connectionsList(),api.workspacesList(task.project_id!),factoryApi.list(task.id)]);
   if(!mounted.current||request!==epoch.current)return;
   if(!Array.isArray(g)||g.length>256||g.some(v=>!factoryGrantShape(v,task))||!Array.isArray(c)||c.length>256||c.some(v=>!v||typeof v.id!=='string')||!Array.isArray(w)||w.length>256||w.some(v=>!v||v.project_id!==task.project_id)||!Array.isArray(s)||s.length>256||s.some(v=>!factoryScopeShape(v,task))||new Set(s.map(v=>v.id)).size!==s.length)throw new Error(t.invalid);
   const nextScopes=new Map(saved.scopes),nextRuns=new Map(saved.runs);
   for(const value of s)advanceScopeFloor(nextScopes,value);
   if([...saved.scopes.keys()].some(id=>!s.some(v=>v.id===id)))throw new Error(t.invalid);
   const selected=s.find(v=>v.id===scopeChoice.current)??s[0]??null;
   // Read all scopes for this root before offering replacement authority: a
   // revoked/expired scope is not proof that its old workers or effects closed.
   const rows=await Promise.all(s.map(async value=>{
    try{
     const v=await factoryApi.run(value.id);
     if(!factoryRunShape(v?.run,task,value.id))throw new Error(t.invalid);
     advanceRunFloor(nextRuns,v.run);
     const valid=factoryDetailsShape(v,task,value.id,value);
     return {scopeId:value.id,run:v.run,detail:valid?v:null,read:true,closed:valid&&factoryRunSupported(v.run)&&['completed','cancelled'].includes(v.run.state),problem:valid?'':t.invalid};
    }catch(e){
     if(e instanceof DaemonError&&e.code==='NOT_FOUND'&&e.sideEffectState==='not_started'&&!saved.runs.has(value.id))return {scopeId:value.id,run:null,detail:null,read:true,closed:true,problem:''};
     return {scopeId:value.id,run:null,detail:null,read:false,closed:false,problem:e instanceof Error?e.message:t.invalid};
    }
   }));
   if(!mounted.current||request!==epoch.current)return;
   const choice=rows.find(v=>v.scopeId===selected?.id),problem=rows.find(v=>v.problem)?.problem??'';
   // A later read may be busy without changing this validated business
   // snapshot. This is not an approval and never covers a pending write.
   trustedRead.current=!problem&&choice?.read&&choice.detail?factorySafeFingerprint([task,selected,choice.run,choice.detail,g,c,w]):'';
   saved.scopes=nextScopes;saved.runs=nextRuns;scopeChoice.current=selected?.id??'';
   setGrants(g);setConnections(c);setWorkspaces(w);setScopes(s);setScopeId(selected?.id??'');
   setRun(choice?.run??null);setDetail(choice?.detail??null);setRunFresh(choice?.read??false);
   setNoUnresolvedRun(rows.every(v=>v.closed));setFresh(!problem);setError(problem);
  }catch(e){if(mounted.current&&request===epoch.current){trustedRead.current='';setError(`${t.invalid} ${e instanceof Error?e.message:''}`);}}
  finally{if(mounted.current&&request===epoch.current){setLoading(false);setReadTick(value=>value+1);}}
 },[connected,task.id,task.project_id,task.revision,t.disconnected,t.invalid,saved]);

 useEffect(()=>{if(open)void refresh();},[open,refresh]);
 useEffect(()=>{if(!open||!connected||!fresh||loading||busy||pending||!run||!factoryRunSupported(run)||!['queued','starting','running','waiting_child','resuming','cancel_requested'].includes(run.state))return;if(automaticReads.current>=20){setPaused(true);return;}const timer=setTimeout(()=>{++automaticReads.current;void refresh(false);},1000);return()=>clearTimeout(timer);},[open,connected,fresh,loading,busy,pending,run,readTick,refresh]);
 const scope=scopes.find(s=>s.id===scopeId)??null;const grant=grants.find(g=>g.id===form.grantId);
 const currentGrant=scope&&grants.find(g=>g.id===scope.root_grant_id);
 const liveScope=!!scope&&!scope.revoked&&scope.expires_at_ms>Date.now()&&!!currentGrant&&!currentGrant.revoked&&currentGrant.expires_at_ms>Date.now()&&currentGrant.revision===scope.root_grant_revision&&factorySelectionSupported(scope.selection);
 const parentReadIdentity=factorySafeFingerprint([task,scope,run,detail,grants,connections,workspaces]);
 const parentRefreshOnly=loading&&!busy&&!pending&&!saved.readOnly&&connected&&liveScope&&trustedRead.current===parentReadIdentity;
 const disabled=saved.readOnly||!connected||busy||loading||!fresh||!!pending;
 const canRecover=!!pending&&pending.unknown&&connected&&!busy&&!loading&&(fresh||pending.kind==='cancel'&&runFresh)&&(pending.kind==='approve'||pending.scopeId===scopeId);
 function remember(op:Pending|null){saved.pending=op;if(mounted.current)setPending(op);}
 async function send(op:Pending){
  if(saved.readOnly||inFlight.current||!connected||!(fresh||op.kind==='cancel'&&runFresh))return;
  if(op.kind==='start'&&!op.unknown&&(!scope||scope.id!==op.scopeId||scope.revoked||scope.expires_at_ms<=Date.now()||!currentGrant||currentGrant.revoked||currentGrant.expires_at_ms<=Date.now()||currentGrant.revision!==scope.root_grant_revision||!noUnresolvedRun)){setFresh(false);setError(t.invalid);return;}
  trustedRead.current='';inFlight.current=true;remember({...op,unknown:false});setBusy(true);setError('');
  try{
   if(op.kind==='approve'){
    const value=await factoryApi.approve(op.selection!,op.key);
    if(!factoryScopeShape(value,task)||!factorySame(value.selection,op.selection))throw new Error(t.invalid);advanceScopeFloor(saved.scopes,value,op.unknown);
   }else if(op.kind==='revoke'){
    const value=await factoryApi.revoke(op.scopeId,op.revision!,op.key);
    if(!factoryScopeShape(value,task)||value.id!==op.scopeId||!value.revoked||value.revision<=op.revision!)throw new Error(t.invalid);advanceScopeFloor(saved.scopes,value,op.unknown);
   }else{
    const value=op.kind==='start'?await factoryApi.start(op.scopeId,op.revision!,op.key):await factoryApi.cancel(op.scopeId,op.revision!,op.key);
    if(!factoryRunShape(value,task,op.scopeId)||(op.runId!==null&&value.id!==op.runId)||(op.kind==='cancel'&&(!value.cancel_requested||value.revision<=op.revision!)))throw new Error(t.invalid);advanceRunFloor(saved.runs,value,op.unknown);
   }
   remember(null);if(mounted.current)await refresh();
  }catch(e){if(op.unknown||uncertain(e)){remember({...op,unknown:true});if(mounted.current){setFresh(false);setRunFresh(false);setError(t.pending);}}else{remember(null);if(mounted.current){setFresh(false);setError(`${t.denied} ${e instanceof Error?e.message:''}`);}}}
  finally{inFlight.current=false;if(mounted.current)setBusy(false);}
 }
 function operation(kind:Pending['kind']):Pending{return {kind,key:crypto.randomUUID(),scopeId:scopeId,runId:run?.id??null,revision:kind==='cancel'?run?.revision??null:scope?.revision??null,selection:null,unknown:false};}
 function approve(e:FormEvent){e.preventDefault();if(disabled||!noUnresolvedRun||!factoryTaskEligible(task)||!grant||scopes.some(s=>!s.revoked&&s.expires_at_ms>Date.now()))return;try{void send({...operation('approve'),scopeId:'',runId:null,revision:null,selection:parseFactorySelection(form,task,grant)});}catch(e){setError(e instanceof Error?e.message:t.invalid);}}
 const set=(key:keyof FactoryForm,value:FactoryForm[keyof FactoryForm])=>setForm(f=>({...f,[key]:value,...(key==='planning'&&value===false?{execution:false}:{})}));
 function budgetFields(key:'root'|'project'){return <fieldset><legend>{key==='root'?t.rootBudget:t.projectBudget}</legend><div className="factory-fields">{(Object.keys(form[key]) as (keyof FactoryBudgetForm)[]).map(field=><label key={field}>{t.limits[field]}<input inputMode="numeric" value={form[key][field]} onChange={e=>setForm(f=>({...f,[key]:{...f[key],[field]:e.target.value}}))}/></label>)}</div></fieldset>;}
 return <div className="factory-panel"><p>{t.description}</p>{saved.readOnly&&<p role='alert'>{language==='zh'?'未对账的 Factory 请求已达本窗口上限；请先处理已有请求。':'This window has reached its unresolved Factory request limit; reconcile an existing request first.'}</p>}{task.state==='outcome_unknown'&&<p>{language==='zh'?'原任务结果仍未知，请先对账；不能据此开始新的 Factory 或重置累计预算。':'The original task outcome remains unknown. Reconcile it before a new Factory; consumed budgets are not reset.'}</p>}<button onClick={()=>void refresh()} disabled={!connected||loading||busy}>{t.refresh}</button>{paused&&<p>{t.paused}</p>}{error&&<p role="alert">{error}</p>}{pending&&<><p>{t.pending}</p><button disabled={!canRecover} onClick={()=>void send(pending)}>{t.recover}</button></>}
 <form onSubmit={approve}><fieldset disabled={disabled}><legend>{t.scope}</legend><label>{t.selectGrant}<select value={form.grantId} onChange={e=>setForm(f=>({...f,grantId:e.target.value,connections:[],workspaces:[],execution:false}))}><option value="">—</option>{grants.filter(g=>g.parent_id===null&&g.root_grant_id===g.id&&!g.revoked&&g.expires_at_ms>Date.now()).map(g=><option key={g.id} value={g.id}>{g.id}</option>)}</select></label>
 <label>{t.connections}<select multiple value={form.connections} onChange={e=>set('connections',[...e.target.selectedOptions].map(o=>o.value))}>{connections.filter(c=>grant?.scope.connection_ids.includes(c.id)).map(c=><option key={c.id} value={c.id}>{c.name} · {c.id}</option>)}</select></label>
 <label>{t.workspaces}<select multiple value={form.workspaces} onChange={e=>set('workspaces',[...e.target.selectedOptions].map(o=>o.value))}>{workspaces.filter(w=>grant?.scope.workspace_ids.includes(w.id)).map(w=><option key={w.id} value={w.id}>{w.root} · {w.id}</option>)}</select></label>
 <details><summary>{t.inputs} / {t.outputs}</summary>{(['inputs','outputs'] as const).map(key=><label key={key}>{t[key]}<textarea value={form[key]} onChange={e=>set(key,e.target.value)} placeholder={t.artifactHint}/></label>)}<p>{t.artifactHint}</p></details><label>{t.recipients}<textarea value={form.recipients} onChange={e=>set('recipients',e.target.value)}/></label><div className="factory-fields">{([['maxDepth',t.depth],['maxChildren',t.children],['maxNodes',t.nodes],['ttlMs',t.ttl]] as const).map(([key,label])=><label key={key}>{label}<input inputMode="numeric" value={form[key]} onChange={e=>set(key,e.target.value)}/></label>)}</div>{budgetFields('root')}{budgetFields('project')}<label><input type="checkbox" checked={form.delegate} onChange={e=>set('delegate',e.target.checked)}/>{t.delegate}</label><label><input type="checkbox" checked={form.resume} onChange={e=>set('resume',e.target.checked)}/>{t.resume}</label><details><summary>{cp.policy}</summary><label><input type="checkbox" checked={form.planning??false} onChange={e=>set('planning',e.target.checked)}/>{cp.enable}</label>{form.planning&&<><div className="factory-fields">{([['planningNodes',cp.nodes,'16'],['planningRevisions',cp.revisions,'16'],['planningBytes',cp.bytes,'65536']] as const).map(([key,label,fallback])=><label key={key}>{label}<input inputMode="numeric" value={form[key]??fallback} onChange={e=>set(key,e.target.value)}/></label>)}</div><label>{cp.validators}<textarea value={form.planningValidators??''} onChange={e=>set('planningValidators',e.target.value)} placeholder={cp.validatorHint}/></label><p>{cp.validatorHint}</p><label>{cp.writable}<textarea value={form.planningWritable??''} onChange={e=>set('planningWritable',e.target.value)}/></label><label>{cp.criteria}<textarea value={form.planningCriteria??''} onChange={e=>set('planningCriteria',e.target.value)}/></label><p>{cp.boundary}</p></>}</details><FactoryChildGoalPolicyForm form={form} grant={grant} language={language} onChange={setForm}/><p>{t.unsupported}</p><button className="primary" type="submit" disabled={!grant||!noUnresolvedRun||!factoryTaskEligible(task)||scopes.some(s=>!s.revoked&&s.expires_at_ms>Date.now())}>{t.approve}</button></fieldset></form>
 {!!scopes.length&&<label>{t.scope}<select value={scopeId} onChange={e=>{++epoch.current;setFresh(false);setRunFresh(false);scopeChoice.current=e.target.value;setScopeId(e.target.value);void refresh();}}>{scopes.map(s=><option key={s.id} value={s.id}>{s.id}{s.revoked?' · revoked':''}</option>)}</select></label>}
 {scope&&<>{!scope.selection.capabilities.child_planning&&<p>{cp.noPolicy}</p>}{!factorySelectionSupported(scope.selection)&&<p role="alert">{t.invalid}</p>}<p>{scope.id} · r{scope.revision}</p><p>{t.bounds}</p><details><summary>{language==='zh'?'已批准的委派合同':'Approved delegation contract'}</summary><pre>{JSON.stringify(scope.selection,null,2)}</pre></details><details><summary>{t.rootBudget} / {t.projectBudget}</summary><pre>{JSON.stringify({root:scope.selection.root_budget,project:scope.selection.project_budget},null,2)}</pre></details><p>{t.metadata}</p><button disabled={disabled||scope.revoked} onClick={()=>void send(operation('revoke'))}>{t.revoke}</button><button className="primary" disabled={disabled||!noUnresolvedRun||!liveScope||!runFresh||run!==null||!factoryTaskEligible(task)||scope.selection.expected_task_revision!==task.revision} onClick={()=>void send(operation('start'))}>{t.start}</button>{!run&&runFresh&&<p>{t.noRun}</p>}</>}
 {run&&<section aria-label={t.run}><p>{run.state==='cancel_requested'?t.cancelRequested:factoryRunSupported(run)?run.state:t.unknown} · {run.id} · r{run.revision}</p>{run.last_error&&<p role='status'>{run.last_error.code}: {run.last_error.message} · {run.last_error.side_effect_state}</p>}<button disabled={saved.readOnly||!connected||busy||loading||!!pending||!runFresh||!factoryCanCancel(run)} onClick={()=>void send(operation('cancel'))}>{t.cancel}</button>{detail&&<><h3>{t.tasks}</h3><ul>{detail.tasks.map(item=><li key={item.task_id}>{item.task_id} · {item.state} · {item.attempt_id??'—'}<p>{item.confirmed_stop?t.stopConfirmed:t.stopUnknown} · {item.result_available?t.result:t.notResult}</p>{scope?.selection.capabilities.child_planning&&<FactoryChildPlans scope={scope} target={item} run={run} language={language} connected={connected} parentReady={fresh&&runFresh&&!pending&&liveScope} parentRefreshOnly={parentRefreshOnly} parentReadIdentity={parentReadIdentity}/>}</li>)}</ul><details><summary>{t.waiting}</summary>{detail.waiting_relations.map((r,i)=><p key={`${r.parent_session_id}-${r.child_task_id}-${i}`}>{r.parent_task_id} → {r.child_task_id} · {r.parent_session_id} · {r.parent_stop_confirmed?t.oldParentStopped:t.oldParentUnknown} · {r.handoff_digest??'—'}</p>)}</details></>}</section>}
 <p>{t.context}</p><p>{t.noAcceptance}</p><details><summary>{t.noIsolation}</summary><p>{t.metadata}</p></details></div>;
}
