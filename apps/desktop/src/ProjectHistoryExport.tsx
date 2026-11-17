import { useEffect, useRef, useState } from 'react';
import { DaemonError } from './api';
import type { Language } from './i18n';
import { historyExportApi } from './history-export-api';
import { historyExportMessages, historyExportErrors } from './history-export-i18n';
import { reference, referenceShape, intentShape, sameSelection, selectionShape, sequenceInput, type HistoryPreview, type HistoryReference, type HistoryReceipt, type HistorySelection, type PreviewIntent } from './history-export-types';

export function ProjectHistoryExport({projectId,connected,language}:{projectId:string;connected:boolean;language:Language}) {
 const t=historyExportMessages[language];
 const [from,setFrom]=useState('0'),[to,setTo]=useState('');
 const [tasks,setTasks]=useState(true),[messages,setMessages]=useState(false),[events,setEvents]=useState(false);
 const [preview,setPreview]=useState<HistoryPreview|null>(null),[result,setResult]=useState<HistoryReceipt|null>(null);
 const [missing,setMissing]=useState(false);
 const [pending,setPending]=useState<HistoryReference|PreviewIntent|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState(''),[notice,setNotice]=useState('');
 const epoch=useRef(0),working=useRef(false);
 const pendingRecovery=useRef<HistoryReference|PreviewIntent|null>(null);
 function clearPending(){pendingRecovery.current=null;setPending(null);}
 const storageKey='agentbaton.history-export.v1.'+projectId;
 useEffect(()=>{++epoch.current;working.current=false;setBusy(false);setPreview(null);setResult(null);clearPending();setMissing(false);setFrom('0');setTo('');setTasks(true);setMessages(false);setEvents(false);setError('');setNotice('');
   try {const saved=localStorage.getItem(storageKey);if(saved&&saved.length<=1024){const ref:unknown=JSON.parse(saved);if(referenceShape(ref)){pendingRecovery.current=reference(ref);setPending(reference(ref));}else if(intentShape(ref)&&ref.selection.project_id===projectId){pendingRecovery.current=ref;setPending(ref);const s=ref.selection;setFrom(String(s.from_sequence));setTo(s.to_sequence===null?'':String(s.to_sequence));setTasks(s.tasks);setMessages(s.messages);setEvents(s.events);}else localStorage.removeItem(storageKey);}}catch{setNotice(t.storage);}
   return ()=>{++epoch.current;};
 // Language changes do not replace an in-flight operation or selection.
 // eslint-disable-next-line react-hooks/exhaustive-deps
 },[projectId,storageKey]);
 useEffect(()=>{++epoch.current;working.current=false;setBusy(false);if(!connected)setPreview(null);},[connected]);
 function selection():HistorySelection {try{const value:HistorySelection={version:1,project_id:projectId,from_sequence:sequenceInput(from),to_sequence:to===''?null:sequenceInput(to),tasks,messages,events};if(!selectionShape(value))throw new Error(t.range);return value;}catch{throw new Error(t.range);}}
 function changed(){++epoch.current;setPreview(null);setResult(null);setError('');setNotice(t.invalidated);}
 function retain(ref:HistoryReference|PreviewIntent){try{localStorage.setItem(storageKey,JSON.stringify(ref));}catch{throw new Error(t.storage);}pendingRecovery.current=ref;setPending(ref);}
 async function perform(operation:(current:()=>boolean)=>Promise<void>){if(working.current||!connected)return;working.current=true;const generation=++epoch.current;setBusy(true);setError('');setMissing(false);const current=()=>epoch.current===generation;
   try{await operation(current);}catch(e){if(current()){setError(e instanceof DaemonError?(historyExportErrors[language][e.code]??`${t.error} (${e.code})`):e instanceof Error?e.message:String(e));if(e instanceof DaemonError&&(e.code==='NOT_FOUND'||e.code==='PREVIEW_INCOMPLETE'||intentShape(pendingRecovery.current)&&e.sideEffectState==='not_started'&&['RESOURCE_LIMIT','STALE_REVISION','EXPORT_CAPACITY'].includes(e.code)))setMissing(true);}}finally{if(current()){working.current=false;setBusy(false);}}}
 function restore(r:HistoryReceipt){if(r.preview.selection.project_id!==projectId)throw new Error('INVALID_EXPORT_PROJECT');setPreview(r.preview);setResult(r.state==='ready'?r:null);const s=r.preview.selection;setFrom(String(s.from_sequence));setTo(s.to_sequence===null?'':String(s.to_sequence));setTasks(s.tasks);setMessages(s.messages);setEvents(s.events);clearPending();setNotice(t.recovered);}
 const count=preview?Object.values(preview.counts).reduce((a,b)=>a+b,0):0;
 return <section aria-label={t.title}>
  <h3>{t.title}</h3><p className="hint">{t.help}</p>
  <fieldset disabled={busy||!!pending||!connected}><legend>{t.title}</legend>
   <label>{t.from}<input inputMode="numeric" value={from} onChange={e=>{changed();setFrom(e.target.value);}} maxLength={16}/></label>
   <label>{t.to}<input inputMode="numeric" value={to} onChange={e=>{changed();setTo(e.target.value);}} maxLength={16}/></label>
   <label><input type="checkbox" checked={tasks} onChange={e=>{changed();setTasks(e.target.checked);}}/>{t.tasks}</label>
   <label><input type="checkbox" checked={messages} onChange={e=>{changed();setMessages(e.target.checked);}}/>{t.messages}</label>
   <label><input type="checkbox" checked={events} onChange={e=>{changed();setEvents(e.target.checked);}}/>{t.events}</label>
  </fieldset>
  <p className="hint">{t.metadata}</p><p>{t.sensitive}</p><p className="hint">{t.excluded}</p>
  {!connected&&<p role="status">{t.offline}</p>}{notice&&<p role="status">{notice}</p>}
  {error&&<p role="alert">{t.error}: {error}</p>}
  {pending?<><p role="status">{intentShape(pending)?t.previewUnknown:t.unknown}</p><button disabled={busy||!connected} onClick={()=>void perform(async current=>{let r:HistoryReceipt;if(intentShape(pending)){const p=await historyExportApi.preview(pending.selection,pending.key);if(!current())return;retain(reference(p));r=await historyExportApi.get(reference(p));}else{r=await historyExportApi.get(pending);}if(current())restore(r);})}>{busy?t.busy:t.reconcile}</button>{missing&&<button disabled={busy||!connected} onClick={()=>{try{localStorage.removeItem(storageKey);}catch{setNotice(t.storage);}clearPending();setPreview(null);setResult(null);setMissing(false);setNotice(t.invalidated);}}>{t.fresh}</button>}</>:<button disabled={busy||!connected} onClick={()=>void perform(async current=>{const s=selection();const intent:PreviewIntent={kind:'preview_intent',key:crypto.randomUUID(),selection:s};retain(intent);const p=await historyExportApi.preview(s,intent.key);if(current()){retain(reference(p));clearPending();setPreview(p);setResult(null);setNotice('');}})}>{busy?t.busy:t.preview}</button>}
  {preview&&<div><dl><dt>{t.cutoff}</dt><dd>{preview.snapshot_sequence}</dd><dt>{t.counts}</dt><dd>{count} ({Object.entries(preview.counts).map(([k,v])=>k+': '+v).join(', ')})</dd><dt>{t.redacted}</dt><dd>{preview.redacted_fields}</dd><dt>{t.omitted}</dt><dd>{preview.excluded_events}</dd><dt>{t.missing}</dt><dd>{preview.missing_references}</dd><dt>{t.bytes}</dt><dd>{preview.bytes}</dd><dt>{t.hash}</dt><dd>{preview.package_sha256}</dd></dl>
   {!count&&<p>{t.empty}</p>}
   {!result&&!pending&&<button disabled={busy||!connected} onClick={()=>void perform(async current=>{if(!sameSelection(selection(),preview.selection))throw new Error(t.range);const ref=reference(preview);retain(ref);const reconciled=await historyExportApi.get(ref);if(!current())return;const r=reconciled.state==='ready'?reconciled:await historyExportApi.publish(ref);if(current()){setResult(r);clearPending();}})}>{t.generate}</button>}
  </div>}
  {result&&<div role="status"><strong>{t.ready}</strong><p>{result.filename}</p><button disabled={busy||!connected} onClick={()=>void perform(async()=>{await historyExportApi.reveal(reference(result.preview));})}>{t.reveal}</button></div>}
 </section>;
}
