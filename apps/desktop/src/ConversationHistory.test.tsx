import {act,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {beforeEach,expect,it,vi} from 'vitest';
import {ConversationHistory,mergeConversationHistoryPage} from './ConversationHistory';
import {conversationHistoryApi} from './conversation-history-api';
import {historyIdentity as identity,historyEvent as event,historyCursor as cursor,historyPage as page} from './conversation-history-test-fixtures';
import type {ConversationHistoryPage} from './conversation-history-api';
vi.mock('./conversation-history-api',async original=>({...await original<typeof import('./conversation-history-api')>(),conversationHistoryApi:{page:vi.fn()}}));
const summary=vi.fn();
beforeEach(()=>{vi.resetAllMocks();vi.mocked(conversationHistoryApi.page).mockResolvedValue(page());});
const props={conversation:identity,language:'en' as const,connected:true,disabled:false,onSummary:summary};
it('keeps older browsing and selected excerpts when the actor revision updates',async()=>{
 const {rerender}=render(<ConversationHistory {...props}/>);await screen.findByLabelText('Agent: Message 3');fireEvent.click(screen.getByLabelText('Agent: Message 3'));
 vi.mocked(conversationHistoryApi.page).mockResolvedValue(page([event(1),event(2)],null));fireEvent.click(screen.getByRole('button',{name:'Load earlier history'}));await screen.findByLabelText('Agent: Message 2');fireEvent.click(screen.getByLabelText('Agent: Message 2'));
 rerender(<ConversationHistory {...props} conversation={{...identity,revision:4}}/>);expect(screen.getByText(/The conversation changed/)).toBeInTheDocument();expect(conversationHistoryApi.page).toHaveBeenCalledTimes(2);
 fireEvent.click(screen.getByRole('button',{name:'Use selected excerpts'}));expect(summary).toHaveBeenCalledWith('Agent: Message 2\n\nAgent: Message 3','project');
 expect(screen.getByRole('button',{name:'Remove excerpt #30'})).toBeInTheDocument();fireEvent.click(screen.getByRole('button',{name:'Remove excerpt #30'}));expect(screen.getByLabelText('Agent: Message 3')).not.toBeChecked();
});
it('continues empty scan pages without declaring history empty or complete',async()=>{
 vi.mocked(conversationHistoryApi.page).mockResolvedValue({...page([],{...cursor(25,3),anchor:null}),scan_limited:true,scanned_rows:2048});render(<ConversationHistory {...props}/>);await screen.findByText('No message text in this page.');expect(screen.getByRole('button',{name:'Continue history page'})).toBeEnabled();expect(screen.queryByText('Reached the beginning of available history.')).not.toBeInTheDocument();
 fireEvent.click(screen.getByRole('button',{name:'Continue history page'}));await waitFor(()=>expect(conversationHistoryApi.page).toHaveBeenCalledTimes(2));expect(vi.mocked(conversationHistoryApi.page).mock.calls[1][1]?.before_sequence).toBe(25);
});
it('keeps the current view and selection after a failed read',async()=>{
 render(<ConversationHistory {...props}/>);await screen.findByLabelText('Agent: Message 3');fireEvent.click(screen.getByLabelText('Agent: Message 3'));vi.mocked(conversationHistoryApi.page).mockRejectedValue(new Error('private failure'));fireEvent.click(screen.getByRole('button',{name:'Load earlier history'}));await screen.findByRole('alert');expect(screen.getByLabelText('Agent: Message 3')).toBeChecked();expect(screen.queryByText('private failure')).not.toBeInTheDocument();
});
it('does not let a late page from another conversation replace the current view',async()=>{
 let resolve!:(value:ConversationHistoryPage)=>void;vi.mocked(conversationHistoryApi.page).mockImplementationOnce(()=>new Promise(r=>{resolve=r;}));const {rerender}=render(<ConversationHistory {...props}/>);
 const other={...identity,id:'other',attempt_id:'other-attempt'};vi.mocked(conversationHistoryApi.page).mockResolvedValue({...page(),conversation_id:other.id,attempt_id:other.attempt_id,events:[]});rerender(<ConversationHistory {...props} conversation={other}/>);await screen.findByText('No message text in this page.');await act(async()=>resolve(page()));expect(screen.queryByLabelText('Agent: Message 3')).not.toBeInTheDocument();
});
it('shows explicit gaps and keeps valid earlier chosen context reviewable',async()=>{
 render(<ConversationHistory {...props}/>);await screen.findByLabelText('Agent: Message 3');fireEvent.click(screen.getByLabelText('Agent: Message 3'));vi.mocked(conversationHistoryApi.page).mockResolvedValue({...page([],null),coverage:'gap',gap:{expected_revision:2,observed_revision:null}});fireEvent.click(screen.getByRole('button',{name:'Load earlier history'}));await screen.findByText(/Some history is unavailable/);expect(screen.getByRole('button',{name:'Use selected excerpts'})).toBeEnabled();expect(screen.queryByText('Reached the beginning of available history.')).not.toBeInTheDocument();
});
it('refuses an oversized selection without truncation',async()=>{
 vi.mocked(conversationHistoryApi.page).mockResolvedValue(page([event(3,'x'.repeat(8193))]));render(<ConversationHistory {...props}/>);await screen.findByRole('checkbox');fireEvent.click(screen.getByRole('checkbox'));await screen.findByRole('alert');expect(screen.getByRole('checkbox')).not.toBeChecked();expect(screen.getByRole('button',{name:'Use selected excerpts'})).toBeDisabled();
});
it('deduplicates identical overlapping records and rejects conflicting IDs or sequences',()=>{
 expect(mergeConversationHistoryPage([event(2)],[event(2),event(3)]).events.map(e=>e.revision)).toEqual([2,3]);expect(()=>mergeConversationHistoryPage([event(2,'changed')],[event(2)])).toThrow();expect(()=>mergeConversationHistoryPage([{...event(2),event_id:'other'}],[event(2)])).toThrow();
});
it('replaces a full count or byte window with the requested older page',()=>{
 const current=Array.from({length:500},(_,i)=>event(i+2));expect(mergeConversationHistoryPage([event(1)],current).reason).toBe('count');const text='界'.repeat(400000);expect(mergeConversationHistoryPage([event(2,text),event(3,text)],Array.from({length:6},(_,i)=>event(i+4,text))).reason).toBe('bytes');
});

it('preserves captured excerpts across language changes and latest refresh',async()=>{
 const {rerender}=render(<ConversationHistory {...props}/>);fireEvent.click(await screen.findByLabelText('Agent: Message 3'));
 rerender(<ConversationHistory {...props} language="zh"/>);
 fireEvent.click(screen.getByRole('button',{name:'读取最新历史'}));await waitFor(()=>expect(conversationHistoryApi.page).toHaveBeenCalledTimes(2));
 expect(screen.queryByRole('alert')).not.toBeInTheDocument();expect(screen.getByText('Agent: Message 3',{selector:'p'})).toBeInTheDocument();
});

it('keeps selected context reviewable after the bounded window is replaced',async()=>{
 let revision=601;
 vi.mocked(conversationHistoryApi.page).mockImplementation(async()=>{const events=Array.from({length:100},(_,i)=>event(revision-99+i));revision-=100;return {...page(events),snapshot_revision:601,snapshot_sequence:6010};});
 await act(async()=>{render(<ConversationHistory {...props} conversation={{...identity,revision:601}}/>);});
 fireEvent.click(screen.getByLabelText('Agent: Message 601'));
 const loadEarlier=screen.getByRole('button',{name:'Load earlier history'});
 for(let index=0;index<5;index++){
  await act(async()=>{fireEvent.click(loadEarlier);});
  expect(screen.getByText('Agent: Message '+(501-index*100),{selector:'label'})).toBeInTheDocument();
 }
 expect(screen.queryByText('Agent: Message 601',{selector:'label'})).not.toBeInTheDocument();
 expect(screen.getByText('Agent: Message 601',{selector:'p'})).toBeInTheDocument();
 fireEvent.click(screen.getByRole('button',{name:'Use selected excerpts'}));expect(summary).toHaveBeenCalledWith('Agent: Message 601','project');
 fireEvent.click(screen.getByRole('button',{name:'Remove excerpt #6010'}));expect(screen.getByRole('button',{name:'Use selected excerpts'})).toBeDisabled();
});
it('rejects a latest watermark regression without replacing loaded history',async()=>{
 render(<ConversationHistory {...props}/>);await screen.findByLabelText('Agent: Message 3');
 vi.mocked(conversationHistoryApi.page).mockResolvedValue({...page([event(2)]),snapshot_revision:2,snapshot_sequence:20});
 fireEvent.click(screen.getByRole('button',{name:'Load latest history'}));await screen.findByRole('alert');
 expect(screen.getByLabelText('Agent: Message 3')).toBeInTheDocument();expect(screen.queryByLabelText('Agent: Message 2')).not.toBeInTheDocument();
});
