import { useState } from 'react';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { api, DaemonError, type Task, type JournalEvent, type Project } from './api';
import { sessionApi, type ManagedSession, type NamedConnection } from './session-api';
import { QuickConversation } from './QuickConversation';
import { App } from './App';
vi.mock('./api', async original => ({...await original<typeof import('./api')>(),api:{taskGet:vi.fn(),snapshot:vi.fn(),events:vi.fn(),projectsList:vi.fn(),workspacesList:vi.fn(),create:vi.fn()}}));
vi.mock('./session-api',()=>({sessionApi:{sessionsGet:vi.fn(),sessionsSend:vi.fn(),sessionsList:vi.fn(),connectionsList:vi.fn()}}));
const task:Task={id:'task-a',project_id:'project-a',goal:'Source A goal',state:'running',revision:2,spec_revision:1,created_at:'date',updated_at:'date'};
const session:ManagedSession={id:'session-a',task_id:task.id,project_id:'project-a',workspace_id:'workspace-a',connection_id:'connection-a',grant_id:'grant-a',state:'ready',revision:3,native_session_id:'native',created_at:'date',updated_at:'date',last_error:null,turn_state:null};
const connection={id:session.connection_id,name:'Work Agent'} as NamedConnection;
const event:JournalEvent={event_id:'event-a',aggregate_id:session.id,revision:1,sequence:1,kind:'session.update',occurred_at:'date',source:'provider',payload:{update:{kind:{kind:'agent_text',text:'Chosen reply'}}}};
const convert=vi.fn(), refresh=vi.fn(async()=>{}), uncertain=vi.fn();
function Harness({sessions=[session],events=[event]}:{sessions?:ManagedSession[];events?:JournalEvent[]}) {
 const [summary,setSummary]=useState(''),[project,setProject]=useState<string|undefined>(),[busy,setBusy]=useState(false),[unknown,setUnknown]=useState(new Set<string>());
 return <QuickConversation language="en" connected busy={busy} tasks={[task]} sessions={sessions} connections={[connection]} events={events} uncertainSessions={unknown} onUncertain={id=>{uncertain(id);setUnknown(new Set([id]));}} onBusy={setBusy} refresh={refresh} summary={summary} summaryProject={project} onSummary={(s,p)=>{setSummary(s);if(p!==undefined)setProject(p);}} onConvert={convert}/>;
}
beforeEach(()=>{
 vi.resetAllMocks(); localStorage.clear();localStorage.setItem('agentbaton.language','en');
 vi.mocked(api.taskGet).mockResolvedValue(task);vi.mocked(sessionApi.sessionsGet).mockResolvedValue(session);
 vi.mocked(api.snapshot).mockResolvedValue({tasks:[task],sequence:1,api_version:1,database_schema_version:17});vi.mocked(api.events).mockResolvedValue({events:[event],sequence:1,next_cursor:1,gap:false});vi.mocked(api.projectsList).mockResolvedValue([]);vi.mocked(api.workspacesList).mockResolvedValue([]);
 vi.mocked(sessionApi.sessionsList).mockResolvedValue([session]);vi.mocked(sessionApi.connectionsList).mockResolvedValue([connection]);
});
async function choose(){fireEvent.change(screen.getByLabelText('Existing task conversation'),{target:{value:session.id}});await waitFor(()=>expect(sessionApi.sessionsGet).toHaveBeenCalledWith(session.id));}
it('requires explicit session selection and sends exact observed revision once',async()=>{
 vi.mocked(sessionApi.sessionsSend).mockResolvedValue({...session,revision:4,state:'running'});
 render(<Harness/>);expect(sessionApi.sessionsGet).not.toHaveBeenCalled();await choose();
 fireEvent.change(screen.getByLabelText('Message'),{target:{value:'Continue approved work'}});
 await waitFor(()=>expect(screen.getByRole('button',{name:'Send to existing session'})).toBeEnabled());
 fireEvent.click(screen.getByRole('button',{name:'Send to existing session'}));
 await waitFor(()=>expect(sessionApi.sessionsSend).toHaveBeenCalledWith(session,'Continue approved work',expect.any(String)));
 expect(sessionApi.sessionsSend).toHaveBeenCalledTimes(1);expect(api.create).not.toHaveBeenCalled();
});
it('blocks an unknown send and retains message through refresh',async()=>{
 vi.mocked(sessionApi.sessionsSend).mockRejectedValue(new DaemonError('OUTCOME_UNKNOWN','secret','unknown'));
 render(<Harness/>);await choose();fireEvent.change(screen.getByLabelText('Message'),{target:{value:'Keep this message'}});
 await waitFor(()=>expect(screen.getByRole('button',{name:'Send to existing session'})).toBeEnabled());fireEvent.click(screen.getByRole('button',{name:'Send to existing session'}));
 await waitFor(()=>expect(uncertain).toHaveBeenCalledWith(session.id));expect(screen.getByLabelText('Message')).toHaveValue('Keep this message');
 fireEvent.click(screen.getByRole('button',{name:'Refresh conversation'}));await act(async()=>{});
 expect(screen.getByRole('button',{name:'Send to existing session'})).toBeDisabled();expect(screen.queryByText('secret')).not.toBeInTheDocument();
});
it('rejects foreign fresh-session data',async()=>{
 vi.mocked(sessionApi.sessionsGet).mockResolvedValue({...session,connection_id:'other'});
 render(<Harness/>);await choose();await screen.findByRole('alert');fireEvent.change(screen.getByLabelText('Message'),{target:{value:'Do not send'}});
 expect(screen.getByRole('button',{name:'Send to existing session'})).toBeDisabled();
});
it('copies only selected excerpts with pinned project and leaves conversion unexecuted',async()=>{
 render(<Harness events={[event,{...event,event_id:'foreign',aggregate_id:'other',payload:{text:'Foreign secret'},kind:'session.message_queued'}]}/>);await choose();
 expect(screen.queryByText('Foreign secret')).not.toBeInTheDocument();fireEvent.click(screen.getByLabelText('Agent: Chosen reply'));fireEvent.click(screen.getByRole('button',{name:'Use selected context'}));
 expect(screen.getByLabelText('Draft context')).toHaveValue('Agent: Chosen reply');fireEvent.click(screen.getByRole('button',{name:'Turn into task'}));
 expect(convert).toHaveBeenCalledWith('Agent: Chosen reply','project-a');expect(api.create).not.toHaveBeenCalled();expect(sessionApi.sessionsSend).not.toHaveBeenCalled();
});
it('refuses oversized selected context without truncating',async()=>{
 render(<Harness events={[{...event,payload:{update:{kind:{kind:'agent_text',text:'x'.repeat(8193)}}}}]}/>);await choose();
 fireEvent.click(screen.getAllByRole('checkbox')[0]);fireEvent.click(screen.getByRole('button',{name:'Use selected context'}));
 expect(await screen.findByRole('alert')).toHaveTextContent('nothing was truncated');expect(screen.getByLabelText('Draft context')).toHaveValue('');
});
it('preserves manual draft conversion without a managed session',()=>{
 render(<Harness sessions={[]}/>);expect(screen.getByRole('button',{name:'Send message · Unavailable'})).toBeDisabled();
 fireEvent.change(screen.getByLabelText('Draft context'),{target:{value:'Manual summary'}});fireEvent.click(screen.getByRole('button',{name:'Turn into task'}));expect(convert).toHaveBeenCalledWith('Manual summary',undefined);
});
it('retains source project across App quick-view unmount and different project selection',async()=>{
 const user=userEvent.setup();const project=(id:string):Project=>({id,name:id,identity:{canonical_root:'/generated/'+id,device:1,inode:1,kind:{kind:'directory'}},revision:1,archived:false,created_at:'date',updated_at:'date'});
 vi.mocked(api.projectsList).mockResolvedValue([project('project-a'),project('project-b')]);vi.mocked(api.create).mockResolvedValue({...task,id:'new-draft',state:'draft'});
 render(<App/>);await screen.findAllByTestId('task-card');await user.click(screen.getByRole('button',{name:'Quick chat'}));await choose();
 await user.click(screen.getByLabelText('Agent: Chosen reply'));await user.click(screen.getByRole('button',{name:'Use selected context'}));
 await user.click(screen.getByRole('button',{name:'project-b'}));await user.click(screen.getByRole('button',{name:'Quick chat'}));
 expect(screen.getByLabelText('Draft context')).toHaveValue('Agent: Chosen reply');expect(screen.getByText('Draft project: project-a')).toBeInTheDocument();
 await user.click(screen.getByRole('button',{name:'Turn into task'}));expect(api.create).not.toHaveBeenCalled();
 fireEvent.change(screen.getByLabelText('Selected summary'),{target:{value:'Edited selected context'}});await user.click(screen.getByRole('button',{name:'Create task'}));
 await waitFor(()=>expect(api.create).toHaveBeenCalledWith('Edited selected context',expect.any(String),'project-a'));
});
it('ignores a late fresh-session response after selecting another session',async()=>{
 const second={...session,id:'session-b',revision:8};let resolve!:(value:ManagedSession)=>void;
 vi.mocked(sessionApi.sessionsGet).mockImplementation(id=>id===session.id?new Promise(r=>{resolve=r;}):Promise.resolve(second));
 vi.mocked(sessionApi.sessionsSend).mockResolvedValue({...second,revision:9,state:'running'});
 render(<Harness sessions={[session,second]}/>);await choose();
 fireEvent.change(screen.getByLabelText('Existing task conversation'),{target:{value:second.id}});
 fireEvent.change(screen.getByLabelText('Message'),{target:{value:'Message B'}});
 await waitFor(()=>expect(screen.getByRole('button',{name:'Send to existing session'})).toBeEnabled());
 await act(async()=>resolve(session));fireEvent.click(screen.getByRole('button',{name:'Send to existing session'}));
 await waitFor(()=>expect(sessionApi.sessionsSend).toHaveBeenCalledWith(second,'Message B',expect.any(String)));
});
it('retains unknown-send block in App after quick conversation unmount',async()=>{
 let reject!:(error:Error)=>void;vi.mocked(sessionApi.sessionsSend).mockImplementation(()=>new Promise((_,r)=>{reject=r;}));
 const user=userEvent.setup();render(<App/>);await screen.findAllByTestId('task-card');
 await user.click(screen.getByRole('button',{name:'Quick chat'}));await choose();
 fireEvent.change(screen.getByLabelText('Message'),{target:{value:'Original pending message'}});
 await waitFor(()=>expect(screen.getByRole('button',{name:'Send to existing session'})).toBeEnabled());
 await user.click(screen.getByRole('button',{name:'Send to existing session'}));
 await user.click(screen.getByRole('button',{name:'Tasks'}));
 await act(async()=>reject(new DaemonError('OUTCOME_UNKNOWN','Unknown','unknown')));
 await user.click(await screen.findByRole('button',{name:`Open task: ${task.goal}`}));
 expect(screen.getByRole('button',{name:'Send message'})).toBeDisabled();
 expect(screen.getByLabelText('Task message')).toBeDisabled();
 await user.click(screen.getByRole('button',{name:'Quick chat'}));await choose();
 expect(screen.getByRole('button',{name:'Send to existing session'})).toBeDisabled();
 expect(sessionApi.sessionsSend).toHaveBeenCalledTimes(1);
});
