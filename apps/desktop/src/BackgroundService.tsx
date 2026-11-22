import {useEffect,useRef,useState} from 'react';
import {DaemonError} from './api';
import { managementMessages } from './management-i18n';
import type {Language} from './i18n';
import {backgroundInstanceId,backgroundServiceApi,type BackgroundStatus} from './background-service-api';
import {backgroundServiceMessages} from './background-service-i18n';
export const backgroundInstanceStorageKey='agentbaton.background.last_instance';
export interface BackgroundUiState {hide:'idle'|'pending'|'unknown'}
const sharedUiState:BackgroundUiState={hide:'idle'};
export function BackgroundService({language,connected,embedded=false,uiState=sharedUiState}:{language:Language;connected:boolean;embedded?:boolean;uiState?:BackgroundUiState}){
 const t=backgroundServiceMessages[language],epoch=useRef(0),connection=useRef(connected),mounted=useRef(false),reading=useRef(false),floor=useRef<BackgroundStatus|null>(null);
 if(connection.current!==connected){connection.current=connected;epoch.current++;}
 const [observation,setObservation]=useState<{status:BackgroundStatus;at:string;relation:'same'|'changed'|'first';epoch:number}|null>(null),[busy,setBusy]=useState(false),[ack,setAck]=useState(false),[failed,setFailed]=useState(false),[storageFailed,setStorageFailed]=useState(false),[hidden,setHidden]=useState(false);
 const valid=connected&&observation?.epoch===epoch.current&&!failed;
 function accept(status:BackgroundStatus,stamp:number){if(!mounted.current||stamp!==epoch.current||!connection.current)return false;const prior=floor.current;if(prior?.instance_id===status.instance_id&&(prior.started_at!==status.started_at||status.sequence<prior.sequence))throw new Error('Regressed service observation');floor.current=status;let relation:'same'|'changed'|'first'='first';try{const previous=localStorage.getItem(backgroundInstanceStorageKey);if(backgroundInstanceId(previous))relation=previous===status.instance_id?'same':'changed';localStorage.setItem(backgroundInstanceStorageKey,status.instance_id);setStorageFailed(false);}catch{setStorageFailed(true);}setObservation({status,at:new Date().toISOString(),relation,epoch:stamp});setFailed(false);return true;}
 async function refresh(){if(reading.current||!connection.current)return;const stamp=epoch.current;reading.current=true;setBusy(true);setAck(false);try{accept(await backgroundServiceApi.status(),stamp);}catch{if(mounted.current&&stamp===epoch.current){setFailed(true);setObservation(null);}}finally{reading.current=false;if(mounted.current)setBusy(false);}}
 const lastAutomaticRead=useRef<number | undefined>(undefined);
 useEffect(()=>{mounted.current=true;if(!embedded)void refresh();return()=>{mounted.current=false;epoch.current++;};},[embedded]);
 useEffect(()=>{
   if(!embedded||!connected||busy||lastAutomaticRead.current===epoch.current)return;
   let live=true;
   queueMicrotask(()=>{
     if(!live||reading.current||!connection.current)return;
     lastAutomaticRead.current=epoch.current;
     void refresh();
   });
   return()=>{live=false;};
 },[embedded,connected,busy]);
 useEffect(()=>{setAck(false);},[connected]);
 async function hide(){if(!valid||!ack||reading.current||uiState.hide!=='idle'||!observation)return;const stamp=epoch.current,instance=observation.status.instance_id;reading.current=true;setBusy(true);setAck(false);let dispatched=false;try{const live=await backgroundServiceApi.status();if(!accept(live,stamp)||live.instance_id!==instance)return;if(!mounted.current||stamp!==epoch.current||!connection.current)return;uiState.hide='pending';dispatched=true;await backgroundServiceApi.hide(live);uiState.hide='idle';if(mounted.current&&stamp===epoch.current)setHidden(true);}catch(error){if(dispatched)uiState.hide=error instanceof DaemonError&&error.code!=='OUTCOME_UNKNOWN'&&['none','not_started'].includes(error.sideEffectState)?'idle':'unknown';if(mounted.current&&stamp===epoch.current){setFailed(true);if(!dispatched)setObservation(null);}}finally{reading.current=false;if(mounted.current)setBusy(false);}}
 const facts=valid&&observation?<><dl><dt>{t.instance}</dt><dd>{observation.status.instance_id}</dd><dt>{t.started}</dt><dd>{observation.status.started_at}</dd><dt>{t.observed}</dt><dd>{observation.at}</dd><dt>{t.workers}</dt><dd>{observation.status.owned_provider_workers?t.yes:t.no}</dd></dl><p>{t[observation.relation]}</p><p>{t.recovery}</p></>:<p role="status">{t.unknown}</p>;
 const refreshControl=<button disabled={!connected||busy} onClick={()=>void refresh()}>{busy?t.busy:t.refresh}</button>;
 const confirm=<label><input type="checkbox" checked={ack} disabled={!valid||busy||uiState.hide!=='idle'} onChange={event=>setAck(event.target.checked)}/>{t.review}</label>;
 const action=<button disabled={!valid||!ack||busy||uiState.hide!=='idle'} onClick={()=>void hide()}>{t.hide}</button>;
 const notices=<>{storageFailed&&<p>{t.storage}</p>}{hidden&&<p role="status">{t.hidden}</p>}{uiState.hide==='unknown'&&<p role="alert">{t.hideUnknown}</p>}</>;
 if(embedded){
   const copy=managementMessages[language];
   return <section aria-label={t.title}>
     <details className="management-disclosure management-hide">
       <summary><strong>{copy.hideTitle}</strong></summary>
       <div className="management-detail-actions"><p>{t.limits}</p>{confirm}{action}</div>
     </details>
     <details className="management-disclosure"><summary>{copy.evidence}</summary>{facts}{refreshControl}</details>
     {notices}
   </section>;
 }
 return <section aria-label={t.title}><h3>{t.title}</h3><p>{t.limits}</p>{facts}{refreshControl}{confirm}{action}{notices}</section>;
}
