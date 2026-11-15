import { request, type JournalEvent } from './api';
import type { Conversation } from './conversation-api';
export interface HistoryAnchor {sequence:number;event_id:string;revision:number;sha256:string}
export interface ConversationHistoryCursor {version:1;conversation_id:string;attempt_id:string;snapshot_sequence:number;snapshot_revision:number;before_sequence:number;expected_revision:number;anchor:HistoryAnchor|null;seal:string}
export interface ConversationHistoryPage {
 conversation_id:string;attempt_id:string;snapshot_sequence:number;snapshot_revision:number;events:JournalEvent[];
 next_cursor:ConversationHistoryCursor|null;has_more:boolean;scan_limited:boolean;byte_limited:boolean;scanned_rows:number;scanned_bytes:number;returned_bytes:number;
 coverage:'partial'|'complete'|'gap';gap:null|{expected_revision:number;observed_revision:number|null};capture:'normalized_partial';
}
export type HistoryIdentity=Pick<Conversation,'id'|'attempt_id'|'revision'>;
const MAX_BYTES=4*1024*1024;
const fail=():never=>{throw new Error('Conversation history response could not be verified');};
const integer=(v:unknown,min=0):v is number=>Number.isSafeInteger(v)&&(v as number)>=min;
const text=(v:unknown,max=256):v is string=>typeof v==='string'&&v.length>0&&new TextEncoder().encode(v).length<=max;
const hash=(v:unknown)=>typeof v==='string'&&/^[a-f0-9]{64}$/.test(v);
function object(value:unknown,keys:string[]):Record<string,unknown>{if(!value||typeof value!=='object'||Array.isArray(value))return fail();const row=value as Record<string,unknown>;if(Object.keys(row).some(key=>!keys.includes(key))||keys.some(key=>!Object.hasOwn(row,key)))return fail();return row;}
function cursorShape(value:unknown):ConversationHistoryCursor {
 const c=object(value,['version','conversation_id','attempt_id','snapshot_sequence','snapshot_revision','before_sequence','expected_revision','anchor','seal']);
 if(c.version!==1||!text(c.conversation_id,128)||!text(c.attempt_id,128)||!integer(c.snapshot_sequence)||c.snapshot_sequence>=Number.MAX_SAFE_INTEGER||!integer(c.snapshot_revision,1)||!integer(c.before_sequence,1)||c.before_sequence>c.snapshot_sequence+1||!integer(c.expected_revision,1)||c.expected_revision>c.snapshot_revision||!hash(c.seal))return fail();
 if(c.anchor!==null){const a=object(c.anchor,['sequence','event_id','revision','sha256']);if(!integer(a.sequence,1)||a.sequence>c.snapshot_sequence||!text(a.event_id,128)||!integer(a.revision,1)||a.revision>c.snapshot_revision||!hash(a.sha256)||c.before_sequence>a.sequence||c.expected_revision!==a.revision-1)return fail();}
 return c as unknown as ConversationHistoryCursor;
}
export function parseConversationHistoryPage(value:unknown,identity:HistoryIdentity,cursor:ConversationHistoryCursor|null,limit=100):ConversationHistoryPage {
 if(!integer(limit,1)||limit>100)return fail();
 const p=object(value,['conversation_id','attempt_id','snapshot_sequence','snapshot_revision','events','next_cursor','has_more','scan_limited','byte_limited','scanned_rows','scanned_bytes','returned_bytes','coverage','gap','capture']);
 if(p.conversation_id!==identity.id||p.attempt_id!==identity.attempt_id||!integer(p.snapshot_sequence)||p.snapshot_sequence>=Number.MAX_SAFE_INTEGER||!integer(p.snapshot_revision,1)||p.capture!=='normalized_partial'||!Array.isArray(p.events)||p.events.length>limit||!['partial','complete','gap'].includes(String(p.coverage))||!['has_more','scan_limited','byte_limited'].every(key=>typeof p[key]==='boolean')||!integer(p.scanned_rows)||p.scanned_rows>2048||p.scanned_rows<p.events.length||!integer(p.scanned_bytes)||p.scanned_bytes>MAX_BYTES||!integer(p.returned_bytes,2)||p.returned_bytes>MAX_BYTES||new TextEncoder().encode(JSON.stringify(p.events)).length>MAX_BYTES)return fail();
 let boundary=p.snapshot_sequence+1,expected=p.snapshot_revision;
 if(cursor){const c=cursorShape(cursor);if(c.conversation_id!==identity.id||c.attempt_id!==identity.attempt_id||p.snapshot_sequence!==c.snapshot_sequence||p.snapshot_revision!==c.snapshot_revision)return fail();boundary=c.before_sequence;expected=c.expected_revision;}
 else if(p.snapshot_revision<identity.revision)return fail();
 const ids=new Set<string>();let sequence=0;let revision=0;
 for(const value of p.events){const e=object(value,['sequence','event_id','aggregate_id','revision','kind','occurred_at','payload','source']);if(e.aggregate_id!==identity.id||!integer(e.sequence,1)||e.sequence<=sequence||e.sequence>=boundary||e.sequence>p.snapshot_sequence||!integer(e.revision,1)||e.revision>expected||(revision>0&&e.revision!==revision+1)||!text(e.event_id,128)||ids.has(e.event_id)||!text(e.kind,256)||!e.kind.startsWith('conversation.')||!text(e.occurred_at,128)||!text(e.source,256))return fail();ids.add(e.event_id);sequence=e.sequence;revision=e.revision;}
 const events=p.events as JournalEvent[],first=events[0],last=events.at(-1);
 if(last&&last.revision!==expected)return fail();
 const remaining=first?first.revision-1:expected;
 if(p.has_more!==(p.next_cursor!==null))return fail();
 if(p.coverage==='gap'){
  const gap=object(p.gap,['expected_revision','observed_revision']);if(p.has_more||!integer(gap.expected_revision,1)||gap.expected_revision!==remaining||!(gap.observed_revision===null||integer(gap.observed_revision)))return fail();
 }else if(p.gap!==null)return fail();
 if(p.coverage==='complete'&&(p.has_more||!first||first.revision!==1||first.kind!=='conversation.launch_reserved'))return fail();
 if(p.coverage==='partial'&&!p.has_more)return fail();
 if(p.next_cursor!==null){const next=cursorShape(p.next_cursor);if(p.coverage!=='partial'||next.conversation_id!==identity.id||next.attempt_id!==identity.attempt_id||next.snapshot_sequence!==p.snapshot_sequence||next.snapshot_revision!==p.snapshot_revision||next.before_sequence>=boundary||next.expected_revision!==remaining||first&&next.before_sequence>first.sequence)return fail();
  if(first){if(next.anchor?.sequence!==first.sequence||next.anchor.event_id!==first.event_id||next.anchor.revision!==first.revision)return fail();}
  else if(JSON.stringify(next.anchor)!==JSON.stringify(cursor?.anchor??null)||(!p.scan_limited&&!p.byte_limited))return fail();
 }
 return p as unknown as ConversationHistoryPage;
}
export const conversationHistoryApi={
 page:async(identity:HistoryIdentity,cursor:ConversationHistoryCursor|null,limit=100)=>parseConversationHistoryPage(await request<unknown>('conversations_history',{conversationId:identity.id,cursor,limit}),identity,cursor,limit),
};
