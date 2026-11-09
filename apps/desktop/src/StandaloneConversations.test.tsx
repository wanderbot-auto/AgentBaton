import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { api, DaemonError, type Project, type ManagedWorkspace, type JournalEvent } from './api';
import { sessionApi, type NamedConnection, type ExecutionGrant } from './session-api';
import { conversationApi, type Conversation } from './conversation-api';
import { App } from './App';
import { StandaloneConversations, createConversationUiState } from './StandaloneConversations';
vi.mock('./api',async original=>({...await original<typeof import('./api')>(),api:{projectsList:vi.fn(),workspacesList:vi.fn(),snapshot:vi.fn(),events:vi.fn(),create:vi.fn()}}));
vi.mock('./session-api',()=>({sessionApi:{connectionsList:vi.fn(),grantsList:vi.fn(),grantsCreate:vi.fn(),sessionsCreate:vi.fn(),sessionsSend:vi.fn(),sessionsList:vi.fn()}}));
vi.mock('./conversation-api',async original=>({...await original<typeof import('./conversation-api')>(),conversationApi:{list:vi.fn(),get:vi.fn(),create:vi.fn(),send:vi.fn(),cancel:vi.fn()}}));
const project:Project={id:'project',name:'Test project',identity:{canonical_root:'/generated/source',device:1,inode:2,kind:{kind:'git',common_dir:'/generated/git',head:'a'.repeat(40),dirty:false}},revision:1,archived:false,created_at:'date',updated_at:'date'};
const workspace:ManagedWorkspace={id:'workspace',project_id:project.id,identity:project.identity,root:'/generated/workspace',revision:1,base_revision:'a'.repeat(40),created_at:'date',state:'ready'};
const connection={id:'connection',name:'Approved Agent',provider:'custom_acp',probe_state:'ready',host_id:'local',identity_ref:'identity',config_revision:'config'} as NamedConnection;
const grant:ExecutionGrant={id:'grant',project_id:project.id,root_grant_id:'grant',parent_id:null,depth:0,revision:1,approval_id:'approval',approved_at:'date',expires_at_ms:Date.now()+600000,revoked:false,scope:{workspace_ids:[workspace.id],connection_ids:[connection.id],host_ids:['local'],max_attempts:1,max_concurrency:1,max_duration_ms:60000,recipients:[],network:{mode:'none'},max_depth:0,max_children:0},enforcement:{baton_mediated_scope:true,os_filesystem_sandbox:false,os_network_sandbox:false,native_identity_isolation:false}};
const conversation:Conversation={id:'conversation',project_id:project.id,title:'Discuss design',workspace_id:workspace.id,connection_id:connection.id,grant_id:grant.id,state:'ready',revision:2,native_session_id:'native',created_at:'date',updated_at:'date',last_error:null,turn_state:null,runtime_snapshot:null,attempt_id:'attempt',fencing_token:1,git_common_device:1,git_common_inode:2};
const event:JournalEvent={event_id:'event',aggregate_id:conversation.id,revision:1,sequence:1,kind:'conversation.update',occurred_at:'date',source:'provider',payload:{update:{kind:{kind:'agent_text',text:'Discussed option'}}}};
const convert=vi.fn();let state=createConversationUiState();
beforeEach(()=>{vi.resetAllMocks();localStorage.clear();localStorage.setItem('agentbaton.language','en');vi.mocked(sessionApi.sessionsList).mockResolvedValue([]);state=createConversationUiState();vi.mocked(api.projectsList).mockResolvedValue([project]);vi.mocked(api.workspacesList).mockResolvedValue([workspace]);vi.mocked(sessionApi.connectionsList).mockResolvedValue([connection]);vi.mocked(sessionApi.grantsList).mockResolvedValue([grant]);vi.mocked(conversationApi.list).mockResolvedValue([]);vi.mocked(conversationApi.get).mockResolvedValue(conversation);vi.mocked(api.snapshot).mockResolvedValue({tasks:[],sequence:1,api_version:1,database_schema_version:18});vi.mocked(api.events).mockResolvedValue({events:[event],sequence:1,next_cursor:1,gap:false});});
function mount(){return render(<StandaloneConversations language="en" connected onConvert={convert} uiState={state}/>);}
async function setup(){
 fireEvent.click(screen.getByText('New independent conversation',{selector:'summary'}));
 await waitFor(()=>expect(screen.getByLabelText('Conversation project')).toBeEnabled());fireEvent.change(screen.getByLabelText('Conversation project'),{target:{value:project.id}});
 await waitFor(()=>expect(screen.getByRole('option',{name:workspace.root})).toBeInTheDocument());
 fireEvent.change(screen.getByLabelText('Managed workspace'),{target:{value:workspace.id}});fireEvent.change(screen.getByLabelText('Agent connection'),{target:{value:connection.id}});
 fireEvent.change(screen.getByLabelText('Execution grant'),{target:{value:grant.id}});fireEvent.change(screen.getByLabelText('Conversation name'),{target:{value:conversation.title}});
}
async function select(){vi.mocked(conversationApi.list).mockResolvedValue([conversation]);mount();await screen.findByRole('option',{name:'Discuss design · Ready'});fireEvent.change(screen.getByLabelText('Selected conversation'),{target:{value:conversation.id}});await screen.findByRole('heading',{name:conversation.title});}
it('starts a taskless conversation only after explicit scope and start action',async()=>{
 vi.mocked(conversationApi.create).mockImplementation(async()=>{const row={...conversation,state:'starting',revision:1};vi.mocked(conversationApi.list).mockResolvedValue([row]);vi.mocked(conversationApi.get).mockResolvedValue(row);return row;});
 mount();await setup();expect(conversationApi.create).not.toHaveBeenCalled();fireEvent.click(screen.getByRole('button',{name:'Start conversation'}));
 await waitFor(()=>expect(conversationApi.create).toHaveBeenCalledWith({project_id:project.id,title:conversation.title,workspace_id:workspace.id,connection_id:connection.id,grant_id:grant.id},expect.any(String)));
 expect(api.create).not.toHaveBeenCalled();expect(sessionApi.sessionsCreate).not.toHaveBeenCalled();expect(conversationApi.send).not.toHaveBeenCalled();expect(sessionApi.grantsCreate).not.toHaveBeenCalled();
});
it('retains unknown creation across unmount and only explicitly replays the exact request',async()=>{
 vi.mocked(conversationApi.create).mockRejectedValueOnce(new DaemonError('OUTCOME_UNKNOWN','unknown','unknown')).mockImplementation(async()=>{vi.mocked(conversationApi.list).mockResolvedValue([conversation]);return conversation;});
 const first=mount();await setup();fireEvent.click(screen.getByRole('button',{name:'Start conversation'}));
 await waitFor(()=>expect(state.pending?.reconciled).toBe(true));const call=vi.mocked(conversationApi.create).mock.calls[0];first.unmount();mount();
 await waitFor(()=>expect(screen.getByRole('button',{name:'Reconcile original request'})).toBeEnabled());expect(conversationApi.create).toHaveBeenCalledTimes(1);
 fireEvent.click(screen.getByRole('button',{name:'Reconcile original request'}));await waitFor(()=>expect(conversationApi.create).toHaveBeenCalledTimes(2));expect(vi.mocked(conversationApi.create).mock.calls[1]).toEqual(call);
});
it('disables expired grants and non-Git workspaces',async()=>{
 vi.mocked(sessionApi.grantsList).mockResolvedValue([{...grant,expires_at_ms:0}]);vi.mocked(api.workspacesList).mockResolvedValue([{...workspace,identity:{...project.identity,kind:{kind:'directory'}}}]);
 mount();fireEvent.click(screen.getByText('New independent conversation',{selector:'summary'}));await waitFor(()=>expect(screen.getByLabelText('Conversation project')).toBeEnabled());fireEvent.change(screen.getByLabelText('Conversation project'),{target:{value:project.id}});
 await screen.findByText('No ready managed Git workspace is available for this project. Prepare one in project settings.');expect(screen.getByRole('button',{name:'Start conversation'})).toBeDisabled();
});
it('uses native grant authorization without launching a conversation',async()=>{
 vi.mocked(sessionApi.grantsCreate).mockResolvedValue(grant);mount();await setup();fireEvent.click(screen.getByText('Authorize this scope'));fireEvent.click(screen.getByLabelText('I have reviewed the connection, workspace and execution limits.'));fireEvent.click(screen.getByRole('button',{name:'Approve execution grant'}));
 await waitFor(()=>expect(sessionApi.grantsCreate).toHaveBeenCalledWith({project_id:project.id,workspace_id:workspace.id,connection_id:connection.id,max_attempts:1,max_concurrency:1,max_duration_ms:60000},expect.any(String)));expect(conversationApi.create).not.toHaveBeenCalled();
});
it('sends through the standalone route with exact observed revision',async()=>{
 vi.mocked(conversationApi.send).mockImplementation(async()=>{const result={...conversation,revision:3,state:'running'};vi.mocked(conversationApi.list).mockResolvedValue([result]);vi.mocked(conversationApi.get).mockResolvedValue(result);return result;});
 await select();fireEvent.change(screen.getByLabelText('Conversation message'),{target:{value:'A question'}});fireEvent.click(screen.getByRole('button',{name:'Send message'}));
 await waitFor(()=>expect(conversationApi.send).toHaveBeenCalledWith(conversation,'A question',expect.any(String)));expect(sessionApi.sessionsSend).not.toHaveBeenCalled();expect(api.create).not.toHaveBeenCalled();
});
it('does not auto-retry unknown sends and retains their exact text',async()=>{
 vi.mocked(conversationApi.send).mockRejectedValue(new DaemonError('OUTCOME_UNKNOWN','unknown','unknown'));await select();fireEvent.change(screen.getByLabelText('Conversation message'),{target:{value:'Retained question'}});fireEvent.click(screen.getByRole('button',{name:'Send message'}));
 await waitFor(()=>expect(state.pending?.kind).toBe('send'));await waitFor(()=>expect(state.pending?.reconciled).toBe(true));expect(conversationApi.send).toHaveBeenCalledTimes(1);expect(screen.getByRole('button',{name:'Send message'})).toBeDisabled();expect(screen.getByLabelText('Conversation message')).toHaveValue('Retained question');
});
it('projects cancellation as a request rather than confirmed stop',async()=>{
 vi.mocked(conversationApi.cancel).mockImplementation(async()=>{const result={...conversation,revision:3,state:'cancel_requested'};vi.mocked(conversationApi.list).mockResolvedValue([result]);vi.mocked(conversationApi.get).mockResolvedValue(result);return result;});await select();fireEvent.click(screen.getByRole('button',{name:'Request stop'}));
 await waitFor(()=>expect(conversationApi.cancel).toHaveBeenCalledWith(conversation,expect.any(String)));await screen.findByText('Stop requested');expect(screen.queryByText('Stopped')).not.toBeInTheDocument();
});
it('converts only chosen excerpts with fixed source project and no hidden Task creation',async()=>{
 await select();fireEvent.click(screen.getByLabelText('Agent: Discussed option'));fireEvent.click(screen.getByRole('button',{name:'Use selected excerpts'}));fireEvent.change(screen.getByLabelText('Selected conversation summary'),{target:{value:'Reviewed selected option'}});fireEvent.click(screen.getByRole('button',{name:'Create task from summary'}));expect(convert).toHaveBeenCalledWith('Reviewed selected option',project.id);expect(api.create).not.toHaveBeenCalled();
});
it('rejects details that regress below the observed conversation revision',async()=>{
 vi.mocked(conversationApi.list).mockResolvedValue([conversation]);vi.mocked(conversationApi.get).mockResolvedValue({...conversation,revision:1});mount();await screen.findByRole('option',{name:'Discuss design · Ready'});fireEvent.change(screen.getByLabelText('Selected conversation'),{target:{value:conversation.id}});await screen.findByRole('alert');expect(screen.queryByRole('button',{name:'Send message'})).not.toBeInTheDocument();
});
it('never sends automatically after a cancelled native grant confirmation',async()=>{
 vi.mocked(sessionApi.grantsCreate).mockRejectedValue(new DaemonError('PERMISSION_DENIED','cancelled','not_started'));mount();await setup();fireEvent.click(screen.getByText('Authorize this scope'));fireEvent.click(screen.getByLabelText('I have reviewed the connection, workspace and execution limits.'));fireEvent.click(screen.getByRole('button',{name:'Approve execution grant'}));await waitFor(()=>expect(sessionApi.grantsCreate).toHaveBeenCalledTimes(1));await waitFor(()=>expect(state.inFlight).toBe(false));expect(state.pending).toBeNull();expect(conversationApi.create).not.toHaveBeenCalled();
});

it('opens the actual App dialog and starts no task or prompt implicitly',async()=>{
 vi.mocked(conversationApi.create).mockImplementation(async()=>{vi.mocked(conversationApi.list).mockResolvedValue([conversation]);return conversation;});
 render(<App/>);await screen.findByText('Local daemon connected');fireEvent.click(screen.getByRole('button',{name:'Quick chat'}));fireEvent.click(screen.getByRole('button',{name:'New independent conversation'}));
 expect(screen.getByRole('dialog',{name:'Independent Agent conversations'})).toBeInTheDocument();
 await setup();expect(conversationApi.create).not.toHaveBeenCalled();fireEvent.click(screen.getByRole('button',{name:'Start conversation'}));
 await waitFor(()=>expect(conversationApi.create).toHaveBeenCalledTimes(1));expect(api.create).not.toHaveBeenCalled();expect(conversationApi.send).not.toHaveBeenCalled();expect(sessionApi.sessionsCreate).not.toHaveBeenCalled();
});
