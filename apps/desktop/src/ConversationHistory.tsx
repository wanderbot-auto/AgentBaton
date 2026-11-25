import { useEffect,useRef,useState } from 'react';
import type { JournalEvent } from './api';
import type { Conversation } from './conversation-api';
import { conversationEventText } from './conversation-api';
import { conversationHistoryApi,type ConversationHistoryCursor,type ConversationHistoryPage } from './conversation-history-api';
import { earlierEventWindow,type WindowReplacement } from './event-window';
import { conversationHistoryMessages } from './conversation-history-i18n';
import type { Language } from './i18n';
import { groupTranscript } from './task-thread-history';

type Props={conversation:Pick<Conversation,'id'|'attempt_id'|'project_id'|'revision'>;language:Language;connected:boolean;disabled:boolean;onSummary:(text:string,projectId:string)=>void};
type SelectedExcerpt={id:string;sequence:number;text:string;raw:string;kind:string};
const utf8=(value:string)=>new TextEncoder().encode(value).length;
export function mergeConversationHistoryPage(page:JournalEvent[],current:JournalEvent[]){
 const ids=new Map(current.map(e=>[e.event_id,e])),sequences=new Map(current.map(e=>[e.sequence,e.event_id]));
 for(const event of page){const old=ids.get(event.event_id);if(old&&JSON.stringify(old)!==JSON.stringify(event)||sequences.has(event.sequence)&&sequences.get(event.sequence)!==event.event_id)throw new Error('Conflicting conversation history');}
 const incoming=new Set(page.map(e=>e.event_id));const merged=earlierEventWindow(page,current.filter(e=>!incoming.has(e.event_id)));return {...merged,events:[...merged.events].sort((a,b)=>a.sequence-b.sequence)};
}
export function ConversationHistory(props:Props){return <ConversationHistoryPanel key={`${props.conversation.id}:${props.conversation.attempt_id}:${props.conversation.project_id}`} {...props}/>;}
function ConversationHistoryPanel({conversation,language,connected,disabled,onSummary}:Props){
 const t=conversationHistoryMessages[language];
 const zh = language === 'zh';
 const [selecting,setSelecting] = useState(false);
 const time = new Intl.DateTimeFormat(language, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' });
 const [page,setPage]=useState<ConversationHistoryPage|null>(null),[events,setEvents]=useState<JournalEvent[]>([]),[loading,setLoading]=useState(false),[error,setError]=useState(false),[replacement,setReplacement]=useState<WindowReplacement>(null);
 const [selected,setSelected]=useState(new Map<string,SelectedExcerpt>()),[selectionError,setSelectionError]=useState(false);
 const selectedRef=useRef(selected);selectedRef.current=selected;
 const epoch=useRef(0),reading=useRef(false),mounted=useRef(false),context=useRef(conversation),windowRef=useRef<JournalEvent[]>([]),failedCursor=useRef<ConversationHistoryCursor|null>(null),floor=useRef({sequence:0,revision:0});context.current=conversation;
 async function read(cursor:ConversationHistoryCursor|null){
  if(!connected||reading.current)return;reading.current=true;const stamp=++epoch.current,identity={...context.current};failedCursor.current=cursor;setLoading(true);setError(false);
  try{
   const result=await conversationHistoryApi.page(identity,cursor);
   if(!mounted.current||stamp!==epoch.current||context.current.id!==identity.id||context.current.attempt_id!==identity.attempt_id)return;
   if(result.conversation_id!==identity.id||result.attempt_id!==identity.attempt_id||(!cursor&&(result.snapshot_sequence<floor.current.sequence||result.snapshot_revision<floor.current.revision)))throw new Error();
   const next=mergeConversationHistoryPage(result.events,cursor?windowRef.current:[]);
   // Selected text is an immutable reviewed copy, not a live reference to a page.
   for(const e of result.events){const chosen=selectedRef.current.get(e.event_id);if(chosen){const value=conversationEventText(e);if(chosen.sequence!==e.sequence||value===null||chosen.raw!==value||chosen.kind!==e.kind)throw new Error();}}
   windowRef.current=next.events;setEvents(next.events);setReplacement(next.reason);setPage(result);floor.current={sequence:Math.max(floor.current.sequence,result.snapshot_sequence),revision:Math.max(floor.current.revision,result.snapshot_revision)};
  }catch{if(mounted.current&&stamp===epoch.current)setError(true);}
  finally{if(stamp===epoch.current){reading.current=false;if(mounted.current)setLoading(false);}}
 }
 useEffect(()=>{mounted.current=true;reading.current=false;if(connected)void read(null);else setLoading(false);return()=>{mounted.current=false;++epoch.current;reading.current=false;};},[conversation.id,conversation.attempt_id,connected]);
 const excerpts=events.filter(e=>conversationEventText(e)!==null),selection=[...selected.values()].sort((a,b)=>a.sequence-b.sequence),selectionText=selection.map(e=>e.text).join('\n\n');
 function choose(event:JournalEvent,checked:boolean){
  const next=new Map(selected);if(checked)next.set(event.event_id,{id:event.event_id,sequence:event.sequence,raw:conversationEventText(event)!,kind:event.kind,text:`${event.kind==='conversation.message_queued'?t.user:t.agent}: ${conversationEventText(event)}`});else next.delete(event.event_id);
  const joined=[...next.values()].sort((a,b)=>a.sequence-b.sequence).map(e=>e.text).join('\n\n');if(next.size>500||joined.length>8192||utf8(joined)>8192){setSelectionError(true);return;}setSelected(next);setSelectionError(false);
 }
 const groups = groupTranscript(excerpts, conversationEventText, 'conversation.message_queued');
 return <section aria-label={t.title} aria-busy={loading}>
  <div className="conversation-tools">
    {page?.next_cursor && <button type="button" disabled={!connected||loading} onClick={() => void read(page.next_cursor)}>{page.scan_limited||page.byte_limited?t.next:t.older}</button>}
    {(error || !!page && conversation.revision > page.snapshot_revision) && <button type="button" disabled={!connected||loading} onClick={() => void read(null)}>{t.latest}</button>}
    {!!excerpts.length && <button type="button" aria-pressed={selecting} disabled={disabled} onClick={() => setSelecting(value => !value)}>{zh ? '选摘消息' : 'Select excerpts'}</button>}
  </div>
  {!connected && <p>{t.disconnected}</p>}{loading && <p role="status">{t.loading}</p>}
  {error && <div role="alert"><p>{t.error}</p><button type="button" disabled={!connected||loading} onClick={() => void read(failedCursor.current)}>{t.retry}</button></div>}
  {page && conversation.revision > page.snapshot_revision && <p role="status">{t.updated}</p>}
  {replacement && <p role="status">{t.replaced}</p>}{page?.coverage==='gap' && <p role="status">{t.gap}</p>}
  {page && !excerpts.length && <p>{t.empty}</p>}
  <ol className="transcript">{groups.map(({first, parts, text, user}) => {
    const date = new Date(first.occurred_at);
    const role = user ? t.user : t.agent;
    return <li key={first.event_id}>
      <header><strong>{role}</strong>{!Number.isNaN(+date) && <time dateTime={first.occurred_at}>{time.format(date)}</time>}</header>
      {selecting ? parts.map(event => <label key={event.event_id}><input type="checkbox" checked={selected.has(event.event_id)} disabled={disabled} onChange={e => choose(event,e.target.checked)}/>{role}: {conversationEventText(event)}</label>) : <p>{text}</p>}
      <details><summary>{zh ? '消息详情' : 'Message details'}</summary>
        {parts.map(event => <div key={event.event_id}><small>{event.occurred_at} · <code>{event.event_id}</code></small><pre>{conversationEventText(event)}</pre></div>)}
      </details>
    </li>;
  })}</ol>
  {selecting && <div className="conversation-tools">
    {selection.length > 0 && <details open><summary>{t.selected} ({selection.length})</summary><ol>{selection.map(excerpt =>
      <li key={excerpt.id}><p>{excerpt.text}</p><button type="button" disabled={disabled} aria-label={`${t.remove} #${excerpt.sequence}`} onClick={() => {
        setSelected(old => {const next = new Map(old);next.delete(excerpt.id);return next;});setSelectionError(false);
      }}>{t.remove}</button></li>)}</ol></details>}
    {selectionError && <p role="alert">{t.selectionLimit}</p>}
    <button type="button" disabled={disabled||!selection.length} onClick={() => onSummary(selectionText,conversation.project_id)}>{t.use}</button>
  </div>}
  <details><summary>{zh ? '历史信息' : 'History details'}</summary>
    <p>{t.partial}</p><p>{t.source} · <code>{conversation.id}</code></p>
    {page?.coverage === 'complete' && <p>{t.end}</p>}
    {(page?.scan_limited||page?.byte_limited) && <p>{t.limited}</p>}
    {!error && (!page || conversation.revision <= page.snapshot_revision) && <button type="button" disabled={!connected||loading} onClick={() => void read(null)}>{t.latest}</button>}
  </details>
 </section>;
}
