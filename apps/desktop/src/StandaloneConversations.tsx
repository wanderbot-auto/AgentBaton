import { useEffect, useRef, useState } from 'react';
import { api, DaemonError, type Project, type ManagedWorkspace } from './api';
import { sessionApi, type NamedConnection, type ExecutionGrant, type GrantRequest } from './session-api';
import { conversationApi, conversationMeetsFloor, sameConversation, type Conversation, type ConversationCreate } from './conversation-api';
import { ConversationHistory } from './ConversationHistory';
import { standaloneConversationMessages } from './standalone-conversations-i18n';
import type { Language } from './i18n';

type Intent = { key:string; reconciled:boolean } & (
  {kind:'create'; selection:ConversationCreate} | {kind:'grant'; selection:GrantRequest} |
  {kind:'send'; conversation:Conversation; text:string} | {kind:'cancel'; conversation:Conversation}
);
/** UI-process recovery only. Core stores the durable request/result. No automatic retries. */
export function createConversationUiState() {
  return { pending:null as Intent|null, inFlight:false, floors:new Map<string,Conversation>(), listeners:new Set<()=>void>() };
}
const defaultState=createConversationUiState();
type Props={language:Language;connected:boolean;onConvert:(summary:string,projectId:string)=>void;onBusy?:(busy:boolean)=>void;uiState?:ReturnType<typeof createConversationUiState>};
const validLimit=(n:number,min:number,max:number)=>Number.isSafeInteger(n)&&n>=min&&n<=max;
const byteLength=(s:string)=>new TextEncoder().encode(s).length;
const fitsSummary=(s:string)=>s.length<=8192&&byteLength(s)<=8192;
const ambiguous=(e:unknown)=>!(e instanceof DaemonError)||!['none','not_started'].includes(e.sideEffectState);
export function StandaloneConversations({language,connected,onConvert,onBusy,uiState=defaultState}:Props) {
  const t=standaloneConversationMessages[language], saved=uiState.pending;
  const [,renderState]=useState(0), [tick,setTick]=useState(0);
  const [projectId,setProjectId]=useState(saved?.kind==='create'||saved?.kind==='grant'?saved.selection.project_id:''), [workspaceId,setWorkspaceId]=useState(saved?.kind==='create'||saved?.kind==='grant'?saved.selection.workspace_id:''), [connectionId,setConnectionId]=useState(saved?.kind==='create'||saved?.kind==='grant'?saved.selection.connection_id:'');
  const [grantId,setGrantId]=useState(saved?.kind==='create'?saved.selection.grant_id:''), [title,setTitle]=useState(saved?.kind==='create'?saved.selection.title:'');
  const [attempts,setAttempts]=useState(saved?.kind==='grant'?saved.selection.max_attempts:1), [concurrency,setConcurrency]=useState(saved?.kind==='grant'?saved.selection.max_concurrency:1), [duration,setDuration]=useState(saved?.kind==='grant'?saved.selection.max_duration_ms/1000:60), [ack,setAck]=useState(false);
  const [projects,setProjects]=useState<Project[]>([]), [connections,setConnections]=useState<NamedConnection[]>([]), [workspaces,setWorkspaces]=useState<ManagedWorkspace[]>([]), [grants,setGrants]=useState<ExecutionGrant[]>([]), [rows,setRows]=useState<Conversation[]>([]);
  const [selectedId,setSelectedId]=useState(saved?.kind==='send'||saved?.kind==='cancel'?saved.conversation.id:''), [view,setView]=useState<Conversation|null>(null);
  const [loading,setLoading]=useState(false), [fresh,setFresh]=useState(false), [error,setError]=useState(''), [message,setMessage]=useState(saved?.kind==='send'?saved.text:'');
  const [summary,setSummary]=useState(''), [summaryProject,setSummaryProject]=useState<string|null>(null);
  const mounted=useRef(true), sequence=useRef(0), polls=useRef(0), onBusyRef=useRef(onBusy);onBusyRef.current=onBusy;
  const notify=()=>{for(const listener of uiState.listeners)listener();};
  useEffect(()=>{mounted.current=true;const listener=()=>renderState(n=>n+1);uiState.listeners.add(listener);return()=>{mounted.current=false;++sequence.current;uiState.listeners.delete(listener);};},[uiState]);
  const pending=uiState.pending, busy=uiState.inFlight, disabled=!connected||busy||loading||!fresh||!!pending;
  const project=projects.find(p=>p.id===projectId), connection=connections.find(c=>c.id===connectionId), workspace=workspaces.find(w=>w.id===workspaceId);
  const readyWorkspaces=workspaces.filter(w=>w.project_id===projectId&&w.state==='ready'&&w.identity?.kind.kind==='git');
  const readyConnections=connections.filter(c=>c.probe_state==='ready'&&c.host_id==='local');
  const matchingGrants=grants.filter(g=>g.project_id===projectId&&!g.revoked&&g.expires_at_ms>Date.now()&&g.scope?.workspace_ids.includes(workspaceId)&&g.scope.connection_ids.includes(connectionId)&&g.scope.host_ids.includes('local'));
  const grant=matchingGrants.find(g=>g.id===grantId);
  const scopeReady=!!project&&!project.archived&&readyWorkspaces.some(w=>w.id===workspaceId)&&readyConnections.some(c=>c.id===connectionId);
  useEffect(()=>{
    const request=++sequence.current;let active=true;setFresh(false);setView(null);setError('');
    if(!connected){setLoading(false);return()=>{active=false;};}setLoading(true);
    void(async()=>{
      const [p,c,all,w,g]=await Promise.all([api.projectsList(),sessionApi.connectionsList(),conversationApi.list(),projectId?api.workspacesList(projectId):Promise.resolve([]),projectId?sessionApi.grantsList(projectId):Promise.resolve([])]);
      if(!active||request!==sequence.current)return;
      if(!Array.isArray(p)||!Array.isArray(c)||!Array.isArray(w)||!Array.isArray(g)||w.some(item=>item.project_id!==projectId)||g.some(item=>item.project_id!==projectId))throw new Error();
      for(const [id,floor]of uiState.floors){const row=all.find(item=>item.id===id);if(!row||!conversationMeetsFloor(row,floor))throw new Error();}
      let detail:Conversation|null=null;
      if(selectedId){
        const listed=all.find(row=>row.id===selectedId);if(!listed)throw new Error();
        detail=await conversationApi.get(selectedId);
        if(!conversationMeetsFloor(detail,listed))throw new Error();
      }
      if(!active||request!==sequence.current)return;
      for(const row of all)uiState.floors.set(row.id,row);
      if(detail)uiState.floors.set(detail.id,detail);
      setProjects(p);setConnections(c);setWorkspaces(w);setGrants(g);setRows(all.map(row=>row.id===detail?.id?detail:row));setView(detail);setFresh(true);
      if(uiState.pending&&!uiState.inFlight){uiState.pending={...uiState.pending,reconciled:true};notify();}
    })().catch(()=>{if(active&&request===sequence.current)setError(t.error);}).finally(()=>{if(active&&request===sequence.current)setLoading(false);});
    return()=>{active=false;};
  },[connected,projectId,selectedId,tick,t.error,uiState]);
  useEffect(()=>{if(!fresh||loading||busy||pending||!view||!['starting','running','cancel_requested'].includes(view.state)||polls.current>=20)return;const timer=setTimeout(()=>{++polls.current;setTick(n=>n+1);},1000);return()=>clearTimeout(timer);},[fresh,loading,busy,pending,view]);
  function refresh(){polls.current=0;setTick(n=>n+1);}
  function invalidateScope(){setAck(false);setGrantId('');}
  async function perform(intent:Intent,recovery=false){
    if(!connected||uiState.inFlight||(!recovery&&uiState.pending)||recovery&&(!uiState.pending?.reconciled||uiState.pending.key!==intent.key))return;
    uiState.pending={...intent,reconciled:false};uiState.inFlight=true;notify();onBusyRef.current?.(true);setError('');setFresh(false);++sequence.current;
    try{
      if(intent.kind==='grant'){
        const result=await sessionApi.grantsCreate(intent.selection,intent.key);
        if(result.project_id!==intent.selection.project_id||JSON.stringify(result.scope.workspace_ids)!==JSON.stringify([intent.selection.workspace_id])||JSON.stringify(result.scope.connection_ids)!==JSON.stringify([intent.selection.connection_id])||JSON.stringify(result.scope.host_ids)!==JSON.stringify(['local'])||result.scope.max_attempts!==intent.selection.max_attempts||result.scope.max_concurrency!==intent.selection.max_concurrency||result.scope.max_duration_ms!==intent.selection.max_duration_ms||!Number.isSafeInteger(result.revision)||result.revision<1)throw new Error();
        if(mounted.current){setGrantId(result.id);setAck(false);}
      }else{
        const result=intent.kind==='create'?await conversationApi.create(intent.selection,intent.key):intent.kind==='send'?await conversationApi.send(intent.conversation,intent.text,intent.key):await conversationApi.cancel(intent.conversation,intent.key);
        if(intent.kind==='create'){
          if((['project_id','title','workspace_id','connection_id','grant_id']as const).some(key=>result[key]!==intent.selection[key]))throw new Error();
        }else if(!sameConversation(result,intent.conversation)||(intent.kind==='send'?result.revision<=intent.conversation.revision:result.revision<intent.conversation.revision))throw new Error();
        if(intent.kind==='cancel'&&!['cancel_requested','stopped','failed','outcome_unknown'].includes(result.state))throw new Error();
        const floor=uiState.floors.get(result.id);
        // Core exact-key recovery returns the current record; do not accept regressed evidence.
        if(floor&&!conversationMeetsFloor(result,floor))throw new Error();
        if(!floor||conversationMeetsFloor(result,floor))uiState.floors.set(result.id,result);
        if(mounted.current){setSelectedId(result.id);if(intent.kind==='send')setMessage('');if(intent.kind==='create'){setTitle('');setSummary('');setSummaryProject(null);}}
      }
      uiState.pending=null;
    }catch(failure){
      if(!ambiguous(failure)&&!recovery)uiState.pending=null;
      if(mounted.current)setError(ambiguous(failure)||recovery?t.pending:t.error);
    }finally{
      uiState.inFlight=false;notify();onBusyRef.current?.(false);
      if(mounted.current){setLoading(false);setTick(n=>n+1);}
    }
  }
  function start(){if(disabled||!scopeReady||!grant||!title.trim()||byteLength(title)>512)return;void perform({kind:'create',key:crypto.randomUUID(),reconciled:false,selection:{project_id:projectId,title:title.trim(),workspace_id:workspaceId,connection_id:connectionId,grant_id:grantId}});}
  function authorize(){if(disabled||!scopeReady||!ack||!validLimit(attempts,1,100)||!validLimit(concurrency,1,attempts)||!validLimit(duration,1,3600))return;void perform({kind:'grant',key:crypto.randomUUID(),reconciled:false,selection:{project_id:projectId,workspace_id:workspaceId,connection_id:connectionId,max_attempts:attempts,max_concurrency:concurrency,max_duration_ms:duration*1000}});}
  function send(){if(disabled||!view||view.state!=='ready'||!message.trim()||byteLength(message)>65536)return;void perform({kind:'send',key:crypto.randomUUID(),reconciled:false,conversation:structuredClone(view),text:message});}
  const label=(state:string)=>Object.hasOwn(t.states,state)?t.states[state as keyof typeof t.states]:t.unknown;
  return <section className="standalone-conversations" aria-label={t.title} aria-busy={loading||busy}>
    <p>{t.help}</p><button type="button" disabled={!connected||busy||loading} onClick={refresh}>{t.refresh}</button>
    {!connected&&<p>{t.disconnected}</p>}{error&&<p role="alert">{error}</p>}{busy&&<p role="status">{t.sending}</p>}{pending&&!busy&&<div role="status"><p>{t.pending}</p><details><summary>{t.scope}</summary><pre>{JSON.stringify(pending.kind==='create'||pending.kind==='grant'?pending.selection:{conversation_id:pending.conversation.id,project_id:pending.conversation.project_id,expected_revision:pending.conversation.revision,...(pending.kind==='send'?{text:pending.text}:{})},null,2)}</pre></details><button type="button" disabled={!connected||loading||!fresh||!pending.reconciled} onClick={()=>void perform(pending,true)}>{t.recover}</button></div>}
    <details><summary>{t.entry}</summary><fieldset disabled={disabled}>
      <label>{t.project}<select value={projectId} onChange={e=>{setProjectId(e.target.value);setWorkspaceId('');invalidateScope();}}><option value="">{t.choose}</option>{projects.filter(p=>!p.archived).map(p=><option key={p.id} value={p.id}>{p.name}</option>)}</select></label>
      {!project&&<p>{t.noProject}</p>}<label>{t.workspace}<select value={workspaceId} onChange={e=>{setWorkspaceId(e.target.value);invalidateScope();}}><option value="">{t.choose}</option>{readyWorkspaces.map(w=><option key={w.id} value={w.id}>{w.root}</option>)}</select></label>{project&&!readyWorkspaces.length&&<p>{t.noWorkspace}</p>}
      <label>{t.connection}<select value={connectionId} onChange={e=>{setConnectionId(e.target.value);invalidateScope();}}><option value="">{t.choose}</option>{readyConnections.map(c=><option key={c.id} value={c.id}>{c.name} · {c.provider}</option>)}</select></label>{!readyConnections.length&&<p>{t.noConnection}</p>}
      <dl><dt>{t.recipient}</dt><dd>{connection?.name??t.choose}</dd><dt>{t.identity}</dt><dd>{connection?.identity_ref??t.choose}</dd><dt>{t.config}</dt><dd>{connection?.config_revision??t.choose}</dd><dt>{t.host}</dt><dd>{connection?.host_id??t.choose}</dd><dt>{t.workspace}</dt><dd>{workspace?.root??t.choose}</dd></dl><p>{t.boundary}</p>
      <label>{t.grant}<select value={grantId} onChange={e=>setGrantId(e.target.value)}><option value="">{t.choose}</option>{matchingGrants.map(g=><option key={g.id} value={g.id}>{g.id} · {g.scope.max_attempts}/{g.scope.max_concurrency} · {g.scope.max_duration_ms/1000}s</option>)}</select></label>
      <details><summary>{t.grantSetup}</summary><p>{t.grantHelp}</p><label>{t.attempts}<input type="number" min={1} max={100} value={attempts} onChange={e=>{setAttempts(Number(e.target.value));setAck(false);}}/></label><label>{t.concurrency}<input type="number" min={1} max={attempts} value={concurrency} onChange={e=>{setConcurrency(Number(e.target.value));setAck(false);}}/></label><label>{t.duration}<input type="number" min={1} max={3600} value={duration} onChange={e=>{setDuration(Number(e.target.value));setAck(false);}}/></label><label><input type="checkbox" checked={ack} onChange={e=>setAck(e.target.checked)}/>{t.acknowledge}</label><button type="button" disabled={!scopeReady||!ack||!validLimit(attempts,1,100)||!validLimit(concurrency,1,attempts)||!validLimit(duration,1,3600)} onClick={authorize}>{t.approveGrant}</button></details>
      <label>{t.name}<input value={title} maxLength={512} onChange={e=>setTitle(e.target.value)}/></label><button type="button" disabled={!scopeReady||!grant||!title.trim()||byteLength(title)>512} onClick={start}>{t.start}</button>
    </fieldset></details>
    {!rows.length&&fresh&&<p>{t.empty}</p>}<label>{t.selected}<select value={selectedId} disabled={busy||!!pending} onChange={e=>{setSelectedId(e.target.value);setSummary('');setSummaryProject(null);setMessage('');setView(null);polls.current=0;}}><option value="">{t.choose}</option>{rows.map(row=><option key={row.id} value={row.id}>{row.title} · {label(row.state)}</option>)}</select></label>
    {selectedId&&rows.find(row=>row.id===selectedId)&&<ConversationHistory key={selectedId} conversation={view??rows.find(row=>row.id===selectedId)!} language={language} connected={connected} disabled={busy} onSummary={(text,sourceProject)=>{setSummary(text);setSummaryProject(sourceProject);setError('');}} />}
    {view&&<><h3>{view.title}</h3><p>{label(view.state)}</p><p>{t.project}: {projects.find(p=>p.id===view.project_id)?.name??view.project_id}</p><p>{t.connection}: {connections.find(c=>c.id===view.connection_id)?.name??view.connection_id}</p>
      <form onSubmit={e=>{e.preventDefault();send();}}><label>{t.message}<textarea value={message} disabled={disabled||view.state!=='ready'} maxLength={65536} onChange={e=>setMessage(e.target.value)}/></label><button type="submit" disabled={disabled||view.state!=='ready'||!message.trim()||byteLength(message)>65536}>{t.send}</button></form>{view.state!=='ready'&&<p>{t.notReady}</p>}
      <button type="button" disabled={disabled||!['starting','ready','running'].includes(view.state)} onClick={()=>void perform({kind:'cancel',key:crypto.randomUUID(),reconciled:false,conversation:structuredClone(view)})}>{t.stop}</button>

    </>}
    {summaryProject!==null&&<><label>{t.summary}<textarea value={summary} maxLength={8192} onChange={e=>setSummary(e.target.value)}/></label><p>{t.project}: {summaryProject}</p><p>{t.conversionHelp}</p>{!fitsSummary(summary)&&<p role="alert">{t.limit}</p>}<button type="button" disabled={!connected||busy||!!pending||!summary.trim()||!fitsSummary(summary)} onClick={()=>onConvert(summary,summaryProject)}>{t.convert}</button></>}
  </section>;
}
