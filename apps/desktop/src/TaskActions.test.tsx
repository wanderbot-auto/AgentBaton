import { act,fireEvent,render,screen,waitFor } from '@testing-library/react';
import { beforeEach,expect,it,vi } from 'vitest';
import { useState } from 'react';
import { App } from './App';
import { api,DaemonError,type Task } from './api';
import { sessionApi,type ManagedSession } from './session-api';
vi.mock('./api',async original=>({...await original<typeof import('./api')>(),api:{snapshot:vi.fn(),taskGet:vi.fn(),events:vi.fn(),projectsList:vi.fn(),create:vi.fn()}}));
vi.mock('./session-api',()=>({sessionApi:{connectionsList:vi.fn(),sessionsList:vi.fn(),sessionsGet:vi.fn(),sessionsSend:vi.fn()}}));
vi.mock('./TaskPlanSummary',()=>({TaskPlanSummary:()=>null}));
vi.mock('./TaskCodeWorkspace',()=>({TaskCodeWorkspace:()=>null}));
vi.mock('./TaskIntegration',()=>({TaskIntegration:()=>null}));
vi.mock('./FactoryRuntime',()=>({FactoryRuntime:()=>null}));
vi.mock('./GoalPlanning',()=>({GoalPlanning:({task,navigation}:{task:Task;navigation?:{target:string;requestId:string}})=>{const [draft,setDraft]=useState('');return <div><p>Goal target {task.id} {navigation?.target}</p><input aria-label="Plan draft" value={draft} onChange={event=>setDraft(event.target.value)}/></div>;}}));
vi.mock('./ManagedSessions',()=>({sessionEventText:()=>null,ManagedSessions:({initialTaskId,tasks,uncertainSessions,onUncertain,sharedSendBusy,onSendBusyChange}:{initialTaskId:string;tasks:Task[];uncertainSessions:Set<string>;onUncertain:(id:string)=>void;sharedSendBusy:boolean;onSendBusyChange:(value:boolean)=>void})=><div><p>Workspace target {initialTaskId} choices {tasks.map(t=>t.id).join(',')}</p><button disabled={sharedSendBusy} onClick={()=>{onSendBusyChange(true);void sessionApi.sessionsSend(session,'Managed message','managed-key').catch(()=>onUncertain(session.id)).finally(()=>onSendBusyChange(false));}}>Managed send</button><button>Managed stop</button><p>Uncertain sessions {Array.from(uncertainSessions).join(',')}</p><button onClick={()=>onUncertain('session-one')}>Report uncertain send</button></div>}));
const task:Task={id:'one',project_id:'project',goal:'Build the recorded goal',state:'draft',revision:1,spec_revision:1,created_at:'date',updated_at:'date'};
const other={...task,id:'two',goal:'Another goal'};
const session:ManagedSession={id:'session-one',task_id:task.id,project_id:'project',workspace_id:'workspace',connection_id:'connection',grant_id:'grant',state:'ready',revision:2,native_session_id:'native',created_at:'date',updated_at:'date',last_error:null,turn_state:null};
beforeEach(()=>{vi.resetAllMocks();localStorage.clear();localStorage.setItem('agentbaton.language','en');vi.mocked(api.snapshot).mockResolvedValue({tasks:[task,other],sequence:0,api_version:1,database_schema_version:18});vi.mocked(api.events).mockResolvedValue({events:[],sequence:0,next_cursor:0,gap:false});vi.mocked(api.projectsList).mockResolvedValue([]);vi.mocked(sessionApi.connectionsList).mockResolvedValue([{id:'connection',name:'Named Agent'} as never]);vi.mocked(sessionApi.sessionsList).mockResolvedValue([session]);vi.mocked(api.taskGet).mockResolvedValue(task);vi.mocked(sessionApi.sessionsGet).mockResolvedValue(session);vi.mocked(sessionApi.sessionsSend).mockResolvedValue({...session,revision:3,state:'running'});});
async function open(){render(<App/>);fireEvent.click(await screen.findByRole('button',{name:`Open task: ${task.goal}`}));await waitFor(()=>expect(screen.getByRole('button',{name:'Send task goal'})).toBeEnabled());}
async function switchTo(value:Task){fireEvent.click(screen.getByRole('button',{name:'Back to tasks'}));fireEvent.click(await screen.findByRole('button',{name:`Open task: ${value.goal}`}));}
it('navigates to exact task controls without sending or creating, preserving unfinished plan edits',async()=>{
 await open();fireEvent.click(screen.getByRole('button',{name:'Plan and execution'}));expect(screen.getByText('Goal target one plan')).toBeVisible();
 fireEvent.change(screen.getByLabelText('Plan draft'),{target:{value:'Unfinished draft'}});fireEvent.click(screen.getByRole('tab',{name:'Conversation'}));
 fireEvent.click(screen.getByRole('button',{name:'Results and acceptance'}));expect(screen.getByText('Goal target one execution')).toBeVisible();expect(screen.getByLabelText('Plan draft')).toHaveValue('Unfinished draft');
 expect(sessionApi.sessionsSend).not.toHaveBeenCalled();expect(api.create).not.toHaveBeenCalled();
});
it('opens workspace authorization with only the originating task in its chooser',async()=>{
 await open();fireEvent.click(screen.getByRole('button',{name:'Workspace and authorization'}));expect(screen.getByText('Workspace target one choices one')).toBeVisible();expect(sessionApi.sessionsSend).not.toHaveBeenCalled();
});
it('explicitly sends the recorded goal after exact fresh reads without clearing a different composer draft',async()=>{
 await open();expect(screen.getByText('Target session: Named Agent · session-one')).toBeVisible();fireEvent.change(screen.getByLabelText('Task message'),{target:{value:'Unsent follow-up'}});expect(sessionApi.sessionsSend).not.toHaveBeenCalled();fireEvent.click(screen.getByRole('button',{name:'Send task goal'}));
 await waitFor(()=>expect(sessionApi.sessionsSend).toHaveBeenCalledWith(session,task.goal,expect.any(String)));expect(api.taskGet).toHaveBeenCalledWith(task.id);expect(sessionApi.sessionsGet).toHaveBeenCalledWith(session.id);expect(screen.getByLabelText('Task message')).toHaveValue('Unsent follow-up');
});
it('rejects a changed task spec before dispatch and does not label a read failure as an uncertain send',async()=>{
 await open();vi.mocked(api.taskGet).mockResolvedValue({...task,spec_revision:2});fireEvent.click(screen.getByRole('button',{name:'Send task goal'}));await screen.findByRole('alert');expect(sessionApi.sessionsSend).not.toHaveBeenCalled();expect(screen.getByRole('button',{name:'Send task goal'})).toBeEnabled();
});
it('does not dispatch after switching tasks while the fresh read is pending',async()=>{
 let resolve!:(value:Task)=>void;vi.mocked(api.taskGet).mockImplementation(()=>new Promise(r=>resolve=r));await open();fireEvent.click(screen.getByRole('button',{name:'Send task goal'}));await switchTo(other);await act(async()=>resolve(task));expect(sessionApi.sessionsSend).not.toHaveBeenCalled();expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});
it('keeps unknown send blocking its original task after navigating away and back',async()=>{
 let reject!:(reason:unknown)=>void;vi.mocked(sessionApi.sessionsSend).mockImplementation(()=>new Promise((_,r)=>reject=r));await open();fireEvent.click(screen.getByRole('button',{name:'Send task goal'}));await waitFor(()=>expect(sessionApi.sessionsSend).toHaveBeenCalledTimes(1));await switchTo(other);
 await act(async()=>reject(new DaemonError('OUTCOME_UNKNOWN','unknown','unknown')));expect(screen.queryByRole('alert')).not.toBeInTheDocument();await switchTo(task);expect(screen.getByRole('button',{name:'Send task goal'})).toBeDisabled();expect(screen.getByLabelText('Task message')).toBeDisabled();expect(sessionApi.sessionsSend).toHaveBeenCalledTimes(1);
});
it('rejects malformed successful responses as unknown instead of permitting a second send',async()=>{
 await open();vi.mocked(sessionApi.sessionsSend).mockResolvedValue({...session,id:'foreign',revision:3});fireEvent.click(screen.getByRole('button',{name:'Send task goal'}));await screen.findByRole('alert');expect(screen.getByRole('button',{name:'Send task goal'})).toBeDisabled();
});
it('blocks another send when the failure reports confirmed side effects',async()=>{
 await open();vi.mocked(sessionApi.sessionsSend).mockRejectedValue(new DaemonError('INTERNAL','confirmed failure','confirmed'));fireEvent.click(screen.getByRole('button',{name:'Send task goal'}));await screen.findByRole('alert');expect(screen.getByRole('button',{name:'Send task goal'})).toBeDisabled();
});
it('requires an unambiguous matching project session rather than silently choosing one',async()=>{
 vi.mocked(sessionApi.sessionsList).mockResolvedValue([session,{...session,id:'second'}]);render(<App/>);fireEvent.click(await screen.findByRole('button',{name:`Open task: ${task.goal}`}));expect(screen.getByRole('button',{name:'Send task goal'})).toBeDisabled();expect(screen.getByText(/Multiple sessions are ready/)).toBeVisible();expect(sessionApi.sessionsSend).not.toHaveBeenCalled();
});

it('shares unknown sends with execution settings in both directions across dialog close',async()=>{
 await open();fireEvent.click(screen.getByRole('button',{name:'Workspace and authorization'}));fireEvent.click(screen.getByRole('button',{name:'Report uncertain send'}));fireEvent.click(screen.getByRole('button',{name:'Close'}));
 expect(screen.getByRole('button',{name:'Send task goal'})).toBeDisabled();fireEvent.click(screen.getByRole('button',{name:'Workspace and authorization'}));expect(screen.getByText('Uncertain sessions session-one')).toBeVisible();expect(sessionApi.sessionsSend).not.toHaveBeenCalled();
});
it('passes a failed goal send to execution settings without clearing the unknown block',async()=>{
 await open();vi.mocked(sessionApi.sessionsSend).mockRejectedValue(new DaemonError('OUTCOME_UNKNOWN','unknown','unknown'));fireEvent.click(screen.getByRole('button',{name:'Send task goal'}));await screen.findByRole('alert');
 fireEvent.click(screen.getByRole('button',{name:'Workspace and authorization'}));expect(screen.getByText('Uncertain sessions session-one')).toBeVisible();
});
it('late successful goal send does not clear another task composer draft',async()=>{
 let resolve!:(value:ManagedSession)=>void;vi.mocked(sessionApi.sessionsSend).mockImplementation(()=>new Promise(r=>resolve=r));await open();fireEvent.click(screen.getByRole('button',{name:'Send task goal'}));await waitFor(()=>expect(sessionApi.sessionsSend).toHaveBeenCalledTimes(1));await switchTo(other);
 fireEvent.change(screen.getByLabelText('Task message'),{target:{value:'Other task draft'}});await act(async()=>resolve({...session,revision:3,state:'running'}));expect(screen.getByLabelText('Task message')).toHaveValue('Other task draft');
});

it('preserves plan edits when Details is first opened using keyboard navigation',async()=>{
 await open();fireEvent.keyDown(screen.getByRole('tab',{name:'Conversation'}),{key:'End'});fireEvent.change(screen.getByLabelText('Plan draft'),{target:{value:'Keyboard draft'}});
 fireEvent.keyDown(screen.getByRole('tab',{name:'Details'}),{key:'Home'});fireEvent.click(screen.getByRole('button',{name:'Plan and execution'}));expect(screen.getByLabelText('Plan draft')).toHaveValue('Keyboard draft');
});

it('requires a new click after leaving and returning to the same task during a pending read',async()=>{
 let resolve!:(value:Task)=>void;vi.mocked(api.taskGet).mockImplementation(()=>new Promise(r=>resolve=r));await open();fireEvent.click(screen.getByRole('button',{name:'Send task goal'}));await switchTo(other);await switchTo(task);
 await act(async()=>resolve(task));expect(sessionApi.sessionsSend).not.toHaveBeenCalled();expect(screen.queryByRole('alert')).not.toBeInTheDocument();expect(screen.getByRole('button',{name:'Send task goal'})).toBeEnabled();
});

it('keeps a pending managed send locked after dialog close, including Quick chat, then binds late unknown to its source',async()=>{
 let reject!:(reason:unknown)=>void;vi.mocked(sessionApi.sessionsSend).mockImplementation(()=>new Promise((_,r)=>reject=r));await open();fireEvent.click(screen.getByRole('button',{name:'Workspace and authorization'}));fireEvent.click(screen.getByRole('button',{name:'Managed send'}));fireEvent.click(screen.getByRole('button',{name:'Close'}));expect(screen.getByRole('button',{name:'Send task goal'})).toBeDisabled();expect(screen.getByRole('button',{name:'Workspace and authorization'})).toBeEnabled();
 fireEvent.click(screen.getByRole('button',{name:'Quick chat'}));fireEvent.change(screen.getByLabelText('Existing task conversation'),{target:{value:session.id}});fireEvent.change(screen.getByLabelText('Message'),{target:{value:'Would duplicate'}});await waitFor(()=>expect(sessionApi.sessionsGet).toHaveBeenCalled());expect(screen.getByRole('button',{name:'Send to existing session'})).toBeDisabled();
 await act(async()=>reject(new DaemonError('OUTCOME_UNKNOWN','unknown','unknown')));fireEvent.click(screen.getByRole('button',{name:'Tasks'}));fireEvent.click(await screen.findByRole('button',{name:`Open task: ${task.goal}`}));expect(screen.getByRole('button',{name:'Send task goal'})).toBeDisabled();expect(sessionApi.sessionsSend).toHaveBeenCalledTimes(1);
});
it('passes an App pending send lock into newly opened execution settings without blocking navigation or stop',async()=>{
 let resolve!:(value:ManagedSession)=>void;vi.mocked(sessionApi.sessionsSend).mockImplementation(()=>new Promise(r=>resolve=r));await open();fireEvent.click(screen.getByRole('button',{name:'Send task goal'}));await waitFor(()=>expect(sessionApi.sessionsSend).toHaveBeenCalledTimes(1));fireEvent.click(screen.getByRole('button',{name:'Workspace and authorization'}));expect(screen.getByRole('button',{name:'Managed send'})).toBeDisabled();expect(screen.getByRole('button',{name:'Managed stop'})).toBeEnabled();
 await act(async()=>resolve({...session,revision:3,state:'running'}));expect(screen.getByRole('button',{name:'Managed send'})).toBeEnabled();
});

it('keeps the recorded goal preview collapsed until explicitly opened',async()=>{
 await open();const preview=screen.getByText('Review the goal to send').closest('details');expect(preview).not.toHaveAttribute('open');expect(preview?.querySelector('blockquote')).toHaveTextContent(task.goal);expect(sessionApi.sessionsSend).not.toHaveBeenCalled();
});
it('treats a changed bound native session in a successful receipt as unknown',async()=>{
 await open();vi.mocked(sessionApi.sessionsSend).mockResolvedValue({...session,revision:3,native_session_id:'foreign-native'});fireEvent.click(screen.getByRole('button',{name:'Send task goal'}));await screen.findByRole('alert');expect(screen.getByRole('button',{name:'Send task goal'})).toBeDisabled();expect(sessionApi.sessionsSend).toHaveBeenCalledTimes(1);
});
