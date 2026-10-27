import { useCallback, useEffect, useRef, useState, type FormEvent } from 'react';
import { DaemonError, type Project } from './api';
import type { Language } from './i18n';
import { dataWorkspaceApi as api, dataCreationShape, dataPreviewShape, dataSelectionEqual, dataSnapshotEqual, DataFormError, newDataForm, parseDataSelection, type DataCreation, type DataPreview, type DataSelection, type DatasetSnapshot, type DataWorkspaceForm } from './data-workspace-api';
import { dataWorkspaceMessages } from './data-workspace-i18n';
type Props={project:Project;connected:boolean;language:Language;onReady?:()=>void};
type Floor={revision:number;projectRevision:number};
type Floors=[string,Floor][];
type Receipt={key:string;floor:Floor};
type Pending={floors:Floors;receipt?:Receipt;projectRevision:number;kind:'start'|'approve'|'cancel_preview'|'cancel_copy';key:string;revision:number;id:string;selection?:DataSelection;form:DataWorkspaceForm;uncertain:boolean;reconciled:boolean};
const mergeFloors=(...groups:Floors[]):Floors=>{const merged=new Map<string,Floor>();for(const group of groups)for(const [key,floor]of group){const previous=merged.get(key);if(!previous||floor.revision>previous.revision)merged.set(key,{...floor});}return [...merged];};
const matchesFloor=(row:{revision:number;project_revision:number},floor:Floor)=>row.revision>=floor.revision&&row.project_revision===floor.projectRevision;
const intents=new Map<string,Pending>(); // bounded UI-process recovery, not a durable background outbox
const previewStates=['checking','verified','failed','cancel_requested','cancelled','outcome_unknown'];
const copyStates=['creating','ready','failed','cancel_requested','outcome_unknown'];
const previewActive=(record:DataPreview)=>['checking','cancel_requested'].includes(record.state);
const copyActive=(record:DataCreation)=>['creating','cancel_requested'].includes(record.state);
const unknown=(failure:unknown)=>!(failure instanceof DaemonError)||failure.code==='OUTCOME_UNKNOWN'||!['none','not_started'].includes(failure.sideEffectState);
export function DataWorkspaces(props:Props){const [open,setOpen]=useState(false),[started,setStarted]=useState(false);return <section className="data-workspaces" data-testid="data-workspaces"><details onToggle={event=>{setOpen(event.currentTarget.open);if(event.currentTarget.open)setStarted(true);}}><summary><h3>{dataWorkspaceMessages[props.language].title}</h3></summary>{started&&<DataWorkspacePanel key={`${props.project.id}:${props.project.revision}`} {...props} expanded={open}/>}</details></section>;}
function DataWorkspacePanel({project,connected,language,onReady,expanded}:Props&{expanded:boolean}){
  const t=dataWorkspaceMessages[language],saved=intents.get(project.id);
  const [form,setForm]=useState<DataWorkspaceForm>(()=>saved?.form??newDataForm()),[previews,setPreviews]=useState<DataPreview[]>([]),[copies,setCopies]=useState<DataCreation[]>([]),[pending,setPending]=useState<Pending|null>(saved??null),[busy,setBusy]=useState(false),[loading,setLoading]=useState(false),[fresh,setFresh]=useState(false),[error,setError]=useState(''),[paused,setPaused]=useState(false),[readVersion,setReadVersion]=useState(0);
  const mounted=useRef(true),sequence=useRef(0),writing=useRef(false),reads=useRef(0),observed=useRef(new Map<string,Floor>(saved?.floors??[])),notified=useRef(new Set<string>()),readyCallback=useRef(onReady);readyCallback.current=onReady;
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;++sequence.current;};},[]);
  const refresh=useCallback(async(manual=true)=>{
    if(manual){reads.current=0;setPaused(false);}setFresh(false);const cached=intents.get(project.id);if(cached){const blocked={...cached,reconciled:false};intents.set(project.id,blocked);setPending(blocked);}
    if(!connected){setError(t.disconnected);return;}
    const current=++sequence.current;setLoading(true);setError('');
    try{
      const [p,c]=await Promise.all([api.previewsList(project.id),api.creationsList(project.id)]);
      if(!mounted.current||current!==sequence.current)return;
      if(!Array.isArray(p)||!Array.isArray(c)||p.length>100||c.length>100||p.some(row=>!dataPreviewShape(row,project.id))||c.some(row=>!dataCreationShape(row,project.id))||new Set(p.map(row=>row.id)).size!==p.length||new Set(c.map(row=>row.id)).size!==c.length)throw new Error(t.scope);
      for(const copy of c){const preview=p.find(row=>row.id===copy.preview_id);if(preview&&(copy.project_revision!==preview.project_revision||!preview.snapshot||!dataSnapshotEqual(copy.snapshot,preview.snapshot)))throw new Error(t.scope);}
      const op=intents.get(project.id);
      observed.current=new Map(mergeFloors([...observed.current],op?.floors??[]));
      for(const [kind,rows]of [['p',p],['c',c]] as const){const returned=new Set(rows.map(row=>`${kind}:${row.id}`));if([...observed.current.keys()].some(key=>key.startsWith(kind+':')&&!returned.has(key)))throw new Error(t.scope);}
      for(const [kind,rows]of [['p',p],['c',c]] as const)for(const row of rows){const floor=observed.current.get(`${kind}:${row.id}`);if(floor&&!matchesFloor(row,floor))throw new Error(t.scope);}
      if(op&&op.kind!=='start'){
        const target=(op.kind==='cancel_copy'?c:p).find(row=>row.id===op.id);
        if(!target||!matchesFloor(target,{revision:op.revision,projectRevision:op.projectRevision}))throw new Error(t.scope);
        if(op.kind==='approve'&&c.some(row=>row.preview_id===op.id&&row.project_revision!==op.projectRevision))throw new Error(t.scope);
      }
      for(const [kind,rows]of [['p',p],['c',c]] as const)for(const row of rows)observed.current.set(`${kind}:${row.id}`,{revision:row.revision,projectRevision:row.project_revision});
      setPreviews(p);setCopies(c);setFresh(true);for(const row of c)if(row.state==='ready'&&!notified.current.has(row.id)){notified.current.add(row.id);readyCallback.current?.();}
      if(op){
        const receipt=op.receipt;const receiptRow=receipt?[...p.map(row=>({key:`p:${row.id}`,row})),...c.map(row=>({key:`c:${row.id}`,row}))].find(item=>item.key===receipt.key)?.row:undefined;
        const resolved=receipt?!!receiptRow&&matchesFloor(receiptRow,receipt.floor):op.kind==='approve'?c.some(row=>row.preview_id===op.id&&row.project_revision===op.projectRevision):op.kind==='cancel_preview'?p.some(row=>row.id===op.id&&matchesFloor(row,{revision:op.revision,projectRevision:op.projectRevision})&&['cancel_requested','cancelled','failed','verified'].includes(row.state)):op.kind==='cancel_copy'?c.some(row=>row.id===op.id&&matchesFloor(row,{revision:op.revision,projectRevision:op.projectRevision})&&['cancel_requested','ready','failed'].includes(row.state)):false;
        if(resolved){intents.delete(project.id);setPending(null);}else{const ready={...op,floors:[...observed.current],reconciled:true};intents.set(project.id,ready);setPending(ready);}
      }else setPending(null);
    }catch(failure){if(mounted.current&&current===sequence.current){setFresh(false);setError(failure instanceof Error?failure.message:t.error);}}
    finally{if(mounted.current&&current===sequence.current){setLoading(false);setReadVersion(value=>value+1);}}
  },[connected,project.id,project.revision,t.disconnected,t.scope,t.error]);
  useEffect(()=>{void refresh();},[refresh]);
  const active=previews.some(previewActive)||copies.some(copyActive);
  useEffect(()=>{if(!expanded||!connected||!fresh||busy||loading||pending||!active)return;if(reads.current>=20){setPaused(true);return;}const timer=setTimeout(()=>{++reads.current;void refresh(false);},500);return()=>clearTimeout(timer);},[expanded,connected,fresh,busy,loading,pending,active,previews,copies,refresh,readVersion]);
  const disabled=!connected||busy||loading||!fresh;
  const live=!project.archived&&project.identity.kind.kind==='directory';
  const unresolved=previews.some(row=>previewActive(row)||!previewStates.includes(row.state)||row.state==='outcome_unknown')||copies.some(row=>copyActive(row)||!copyStates.includes(row.state)||['outcome_unknown','failed'].includes(row.state));
  const canRecover=!!pending&&pending.uncertain&&pending.reconciled&&!disabled;
  function operation(kind:Pending['kind'],id:string,revision:number,selection?:DataSelection,projectRevision=project.revision):Pending{return {floors:[...observed.current],projectRevision,kind,id,revision,selection,key:crypto.randomUUID(),form:{...form},uncertain:false,reconciled:false};}
  async function perform(op:Pending){
    if(disabled||writing.current||intents.has(project.id)&&(!op.uncertain||!op.reconciled))return;
    if(!intents.has(project.id)&&intents.size>=32){setError(t.pendingLimit);return;}
    writing.current=true;const stored={...op,reconciled:false};intents.set(project.id,stored);setPending(stored);setBusy(true);setError('');++sequence.current;
    try{
      let value:DataPreview|DataCreation;let kind:'p'|'c';
      if(op.kind==='start'){value=await api.previewStart(op.selection!,op.revision,op.key);kind='p';if(!dataPreviewShape(value,project.id)||value.project_revision!==op.projectRevision||!dataSelectionEqual(value.selection,op.selection!))throw new Error(t.scope);}
      else if(op.kind==='approve'){value=await api.approve(op.id,op.revision,op.key);kind='c';if(!dataCreationShape(value,project.id)||value.preview_id!==op.id||value.project_revision!==op.projectRevision)throw new Error(t.scope);}
      else if(op.kind==='cancel_preview'){value=await api.previewCancel(op.id,op.revision,op.key);kind='p';if(!dataPreviewShape(value,project.id)||value.id!==op.id||!matchesFloor(value,{revision:op.revision,projectRevision:op.projectRevision}))throw new Error(t.scope);}
      else{value=await api.creationCancel(op.id,op.revision,op.key);kind='c';if(!dataCreationShape(value,project.id)||value.id!==op.id||!matchesFloor(value,{revision:op.revision,projectRevision:op.projectRevision}))throw new Error(t.scope);}
      const current=intents.get(project.id);if(current?.key!==op.key)return;
      const receipt={key:`${kind}:${value.id}`,floor:{revision:value.revision,projectRevision:value.project_revision}};
      const floors=mergeFloors(op.floors,current.floors,[...observed.current],[[receipt.key,receipt.floor]]);observed.current=new Map(floors);
      const acknowledged={...current,floors,receipt,reconciled:false};intents.set(project.id,acknowledged);
      if(mounted.current){setPending(acknowledged);await refresh();}
    }catch(failure){
      const current=intents.get(project.id);if(current?.key!==op.key)return;
      const uncertain=op.uncertain||unknown(failure);
      if(uncertain){const held={...op,floors:mergeFloors(op.floors,current.floors,[...observed.current]),uncertain:true,reconciled:false};intents.set(project.id,held);if(mounted.current){setPending(held);setFresh(false);setError(t.pending);}}
      else{intents.delete(project.id);if(mounted.current){setPending(null);setError(`${failure instanceof DaemonError&&failure.code==='PERMISSION_DENIED'?t.denied:t.error} ${failure instanceof Error?failure.message:''}`);if(failure instanceof DaemonError&&['INPUT_CHANGED','STALE_REVISION'].includes(failure.code))setFresh(false);}}
    }
    finally{writing.current=false;if(mounted.current)setBusy(false);}
  }
  function start(event:FormEvent){event.preventDefault();if(disabled||pending||!live||unresolved)return;try{void perform(operation('start','',project.revision,parseDataSelection(project,form)));}catch(failure){setError(failure instanceof DataFormError?t[failure.reason]:t.error);}}
  const state=(value:string,kind:'preview'|'copy')=>(kind==='preview'?previewStates:copyStates).includes(value)?t[value as 'checking'|'ready']:`${t.unknownState}: ${value}`;
  const snapshot=(value:DatasetSnapshot)=><><dl><dt>{t.count}</dt><dd>{value.files.length}</dd><dt>{t.bytes}</dt><dd>{value.total_bytes}</dd><dt>{t.dataset}</dt><dd><code>{value.dataset_revision}</code></dd></dl><ul>{value.files.map(file=><li key={file.requirement.artifact_id}><span>{file.requirement.relative_path}</span> · {file.bytes} bytes<br/><code>{file.sha256}</code></li>)}</ul></>;
  return <div aria-busy={busy||loading}><p>{t.hint}</p><details><summary>{t.boundaryTitle}</summary><p>{t.boundaries}</p><p>{t.cancelHint}</p></details>{!live&&<p>{t.unavailable}</p>}<button type="button" disabled={!connected||busy||loading} onClick={()=>void refresh()}>{t.refresh}</button>{error&&<p className="error" role={expanded?'alert':undefined}>{error}</p>}{pending&&<p role={expanded?'status':undefined}>{pending.uncertain?t.pending:pending.receipt?t.receiptPending:t.waiting}</p>}{pending?.uncertain&&<button type="button" disabled={!canRecover} onClick={()=>pending&&void perform(pending)}>{t.recover}</button>}{paused&&<p>{t.pollPaused}</p>}
    <form onSubmit={start}><fieldset disabled={disabled||!!pending||!live||unresolved}><label>{t.files}<textarea rows={3} value={form.files} maxLength={12288} onChange={event=>setForm(previous=>({...previous,files:event.target.value}))}/></label><details><summary>{t.maxTotalBytes}</summary>{(['maxFileBytes','maxTotalBytes','maxFiles']as const).map(key=><label key={key}>{t[key]}<input inputMode="numeric" value={form[key]} onChange={event=>setForm(previous=>({...previous,[key]:event.target.value}))}/></label>)}</details></fieldset><button type="submit" className="primary" disabled={disabled||!!pending||!live||unresolved||!form.files.trim()}>{t.start}</button></form>
    {!previews.length&&!copies.length&&<p>{t.empty}</p>}<h4>{t.previews}</h4>{previews.map(record=><details key={record.id}><summary>{state(record.state,'preview')} · {record.id}</summary><p>{t.revision}: {record.revision}</p>{record.snapshot&&snapshot(record.snapshot)}{record.error&&<p className="error">{record.error.code}: {record.error.message}</p>}{record.state==='verified'&&<p>{t.previewOnly}</p>}<button type="button" disabled={disabled||!!pending||!live||unresolved||record.state!=='verified'||record.project_revision!==project.revision||record.snapshot?.contract.source.canonical_root!==project.identity.canonical_root||copies.some(copy=>copy.preview_id===record.id)} onClick={()=>void perform(operation('approve',record.id,record.revision,undefined,record.project_revision))}>{t.approve}</button>{previewActive(record)&&<button type="button" disabled={disabled||!!pending} onClick={()=>void perform(operation('cancel_preview',record.id,record.revision,undefined,record.project_revision))}>{t.cancelPreview}</button>}{record.state==='outcome_unknown'&&<p>{t.retained}</p>}</details>)}
    <h4>{t.copies}</h4>{copies.map(record=><details key={record.id}><summary>{state(record.state,'copy')} · {record.id}</summary><dl><dt>{t.workspace}</dt><dd>{record.workspace_id}</dd><dt>{t.destination}</dt><dd><code>{record.destination}</code></dd><dt>{t.dataset}</dt><dd><code>{record.dataset_revision}</code></dd><dt>{t.revision}</dt><dd>{record.revision}</dd>{record.contract&&<><dt>{t.digest}</dt><dd><code>{record.contract.contract_digest}</code></dd></>}</dl>{record.cancel_requested&&<p>{record.state==='ready'?t.readyAfterCancel:t.cancelHint}</p>}{['failed','outcome_unknown'].includes(record.state)&&<p>{t.retained}</p>}{record.observation&&<p>{t.copied}: {record.observation.copied_files}</p>}{record.error&&<p className="error">{record.error.code}: {record.error.message}</p>}{copyActive(record)&&<button type="button" disabled={disabled||!!pending} onClick={()=>void perform(operation('cancel_copy',record.id,record.revision,undefined,record.project_revision))}>{t.cancelCopy}</button>}</details>)}
  </div>;
}
