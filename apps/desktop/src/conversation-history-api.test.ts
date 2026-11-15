import {expect,it,vi} from 'vitest';
import {request} from './api';
import {parseConversationHistoryPage as parse,conversationHistoryApi} from './conversation-history-api';
import {historyIdentity as identity,historyEvent as event,historyCursor as cursor,historyPage as page} from './conversation-history-test-fixtures';
vi.mock('./api',()=>({request:vi.fn()}));
it('parses latest and older pages bound to one conversation snapshot',()=>{expect(parse(page(),identity,null).next_cursor).toEqual(cursor());const older=page([event(1),event(2)],null);expect(parse(older,identity,cursor()).coverage).toBe('complete');});
it('accepts an empty bounded scan page with a progressing opaque cursor',()=>{const value={...page([],{...cursor(),before_sequence:25}),scan_limited:true,scanned_rows:2048};expect(parse(value,identity,cursor()).events).toEqual([]);});
it('preserves an explicit missing-revision gap without claiming completion',()=>{const value={...page([],null),coverage:'gap',gap:{expected_revision:2,observed_revision:null}};expect(parse(value,identity,cursor()).coverage).toBe('gap');});
it.each(['conversation_id','attempt_id'])('rejects foreign %s',key=>{expect(()=>parse({...page(),[key]:'other'},identity,null)).toThrow();});
it('rejects foreign events, duplicate IDs, out-of-order rows and discontinuous local revisions',()=>{
 for(const events of [[{...event(3),aggregate_id:'other'}],[event(3),event(3)],[event(3),event(2)],[event(1),event(3)]])expect(()=>parse({...page(events),scanned_rows:events.length},identity,null)).toThrow();
});
it('rejects cursor non-progress, snapshot mixing and backward latest observations',()=>{
 expect(()=>parse(page([],{...cursor(),before_sequence:30}),identity,cursor())).toThrow();
 expect(()=>parse({...page([event(2)]),snapshot_sequence:40},identity,cursor())).toThrow();
 expect(()=>parse(page(),{...identity,revision:4},null)).toThrow();
});
it('rejects forged completion and has-more contradictions',()=>{expect(()=>parse(page([event(3)],null),identity,null)).toThrow();expect(()=>parse({...page(),has_more:false},identity,null)).toThrow();});
it('enforces page bounds and does not require adjacent global sequence numbers',()=>{
 expect(parse(page(),identity,null).events[0].sequence).toBe(30);
 expect(()=>parse({...page(),scanned_rows:2049},identity,null)).toThrow();expect(()=>parse({...page(),returned_bytes:4194305},identity,null)).toThrow();expect(()=>parse(page(),identity,null,101)).toThrow();
});
it('passes the exact sealed cursor through the read-only wire',async()=>{const next=page([event(1),event(2)],null);vi.mocked(request).mockResolvedValue(next);await conversationHistoryApi.page(identity,cursor());expect(request).toHaveBeenCalledWith('conversations_history',{conversationId:identity.id,cursor:cursor(),limit:100});});

it('reports an entirely missing journal as a gap at sequence zero',()=>{
 const value={...page([],null),snapshot_sequence:0,coverage:'gap',gap:{expected_revision:3,observed_revision:null}};
 expect(parse(value,identity,null).coverage).toBe('gap');
 expect(()=>parse({...value,snapshot_sequence:Number.MAX_SAFE_INTEGER},identity,null)).toThrow();
});
