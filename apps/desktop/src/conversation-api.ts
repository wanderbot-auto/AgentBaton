import { request, type JournalEvent } from './api';
export interface Conversation {
  id:string; project_id:string; title:string; workspace_id:string; connection_id:string; grant_id:string;
  state:string; revision:number; native_session_id:string|null; created_at:string; updated_at:string;
  last_error:string|null; turn_state:string|null; runtime_snapshot:unknown;
  attempt_id:string; fencing_token:number; git_common_device:number; git_common_inode:number;
}
export interface ConversationCreate { project_id:string; title:string; workspace_id:string; connection_id:string; grant_id:string }
const fields=['id','project_id','title','workspace_id','connection_id','grant_id','state','revision','native_session_id','created_at','updated_at','last_error','turn_state','runtime_snapshot','attempt_id','fencing_token','git_common_device','git_common_inode'];
const id=(value:unknown):value is string=>typeof value==='string'&&/^[A-Za-z0-9._:-]{1,128}$/.test(value);
const text=(value:unknown,max:number):value is string=>typeof value==='string'&&new TextEncoder().encode(value).length<=max;
const bad=():never=>{throw new Error('Conversation response could not be verified');};
export function parseConversation(value:unknown,expectedId?:string):Conversation {
  if(!value||typeof value!=='object'||Array.isArray(value))return bad();
  const row=value as Record<string,unknown>;
  if(Object.keys(row).some(key=>!fields.includes(key))||fields.some(key=>!Object.hasOwn(row,key)))return bad();
  if(!['id','project_id','workspace_id','connection_id','grant_id','attempt_id'].every(key=>id(row[key]))||expectedId!==undefined&&row.id!==expectedId||!text(row.title,512)||!row.title.trim()||!text(row.state,128)||!row.state||!Number.isSafeInteger(row.revision)||(row.revision as number)<1)return bad();
  if(!['created_at','updated_at'].every(key=>text(row[key],128)&&!!row[key])||!['native_session_id','last_error','turn_state'].every(key=>row[key]===null||text(row[key],16384)))return bad();
  if(!['fencing_token','git_common_device','git_common_inode'].every(key=>Number.isSafeInteger(row[key])&&(row[key] as number)>=0)||row.fencing_token===0)return bad();
  if(row.runtime_snapshot!==null&&(typeof row.runtime_snapshot!=='object'||Array.isArray(row.runtime_snapshot))||new TextEncoder().encode(JSON.stringify(row.runtime_snapshot)).length>1_048_576)return bad();
  if(row.runtime_snapshot!==null){const binding=row.runtime_snapshot as Record<string,unknown>;const keys=['session_id','attempt_id','connection_config_revision','plan_digest','snapshot'];if(Object.keys(binding).some(key=>!keys.includes(key))||keys.some(key=>!Object.hasOwn(binding,key))||binding.session_id!==row.id||binding.attempt_id!==row.attempt_id||!text(binding.connection_config_revision,256)||!binding.connection_config_revision||typeof binding.plan_digest!=='string'||!/^[a-f0-9]{64}$/.test(binding.plan_digest))return bad();const snapshot=binding.snapshot as Record<string,unknown>;const pins=['root_device','root_inode','snapshot_device','snapshot_inode'];if(!snapshot||typeof snapshot!=='object'||Array.isArray(snapshot)||Object.keys(snapshot).some(key=>!['id','digest',...pins].includes(key))||!id(snapshot.id)||typeof snapshot.digest!=='string'||!/^[a-f0-9]{64}$/.test(snapshot.digest)||!pins.every(key=>Number.isSafeInteger(snapshot[key])&&(snapshot[key] as number)>=0))return bad();}
  return row as unknown as Conversation;
}
export function parseConversations(value:unknown):Conversation[] {
  if(!Array.isArray(value)||value.length>1000)return bad();
  const rows=value.map(row=>parseConversation(row));if(new Set(rows.map(row=>row.id)).size!==rows.length)return bad();return rows;
}
export function sameConversation(a:Conversation,b:Conversation):boolean {
  return (['id','project_id','title','workspace_id','connection_id','grant_id','created_at','attempt_id','fencing_token','git_common_device','git_common_inode'] as const).every(key=>a[key]===b[key]);
}
export function conversationMeetsFloor(next:Conversation,floor:Conversation):boolean {
  return sameConversation(next,floor)&&(floor.native_session_id===null||next.native_session_id===floor.native_session_id)&&(floor.runtime_snapshot===null||JSON.stringify(next.runtime_snapshot)===JSON.stringify(floor.runtime_snapshot))&&next.revision>=floor.revision&&(next.revision!==floor.revision||(fields as (keyof Conversation)[]).every(key=>JSON.stringify(next[key])===JSON.stringify(floor[key])));
}
export const conversationApi={
  list:async()=>parseConversations(await request<unknown>('conversations_list')),
  get:async(conversationId:string)=>parseConversation(await request<unknown>('conversations_get',{conversationId}),conversationId),
  create:async(selection:ConversationCreate,idempotencyKey:string)=>parseConversation(await request<unknown>('conversations_create',{projectId:selection.project_id,title:selection.title,workspaceId:selection.workspace_id,connectionId:selection.connection_id,grantId:selection.grant_id,idempotencyKey})),
  send:async(conversation:Conversation,text:string,idempotencyKey:string)=>parseConversation(await request<unknown>('conversations_send',{conversationId:conversation.id,expectedRevision:conversation.revision,text,idempotencyKey}),conversation.id),
  cancel:async(conversation:Conversation,idempotencyKey:string)=>parseConversation(await request<unknown>('conversations_cancel',{conversationId:conversation.id,expectedRevision:conversation.revision,idempotencyKey}),conversation.id),
};
export function conversationEventText(event:JournalEvent):string|null {
  if(!event.payload||typeof event.payload!=='object')return null;
  const payload=event.payload as Record<string,unknown>;
  if(event.kind==='conversation.message_queued'&&typeof payload.text==='string')return payload.text;
  if(event.kind!=='conversation.update'||!payload.update||typeof payload.update!=='object')return null;
  const update=payload.update as Record<string,unknown>,kind=update.kind;
  return kind&&typeof kind==='object'&&(kind as Record<string,unknown>).kind==='agent_text'&&typeof(kind as Record<string,unknown>).text==='string'?(kind as Record<string,unknown>).text as string:null;
}
