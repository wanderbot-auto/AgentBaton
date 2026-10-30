import { render,screen,within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach,expect,it,vi } from 'vitest';
import { App } from './App';
import { api,type Task } from './api';
import { sessionApi,type ManagedSession } from './session-api';
vi.mock('./api',async original=>({...await original<typeof import('./api')>(),api:{snapshot:vi.fn(),create:vi.fn(),cancel:vi.fn(),events:vi.fn(),projectsList:vi.fn()}}));
vi.mock('./session-api',()=>({sessionApi:{connectionsList:vi.fn(),sessionsList:vi.fn(),sessionsSend:vi.fn()}}));
const date='2026-10-28T00:00:00Z',task:Task={id:'accepted-task',project_id:'project',goal:'A result accepted by the Core',state:'accepted',revision:7,spec_revision:1,acceptance_ref:'acceptance',acceptance_sequence:21,created_at:date,updated_at:date};
beforeEach(()=>{vi.resetAllMocks();localStorage.clear();localStorage.setItem('agentbaton.language','en');vi.mocked(api.snapshot).mockResolvedValue({tasks:[task],sequence:21,api_version:1,database_schema_version:17});vi.mocked(api.projectsList).mockResolvedValue([]);vi.mocked(api.events).mockResolvedValue({events:[],next_cursor:21,sequence:21,gap:false});vi.mocked(sessionApi.connectionsList).mockResolvedValue([]);vi.mocked(sessionApi.sessionsList).mockResolvedValue([{id:'ready',task_id:task.id,project_id:'project',workspace_id:'workspace',connection_id:'connection',grant_id:'grant',state:'ready',revision:3,native_session_id:'native',created_at:date,updated_at:date,last_error:null,turn_state:null} as ManagedSession]);});
it('keeps trusted accepted Task in finished history and disables messaging and cancellation despite a reusable ready session',async()=>{
  const user=userEvent.setup();render(<App/>);const card=(await screen.findAllByTestId('task-card'))[0];expect(within(card).getByText('Accepted')).toBeVisible();await user.click(screen.getByRole('button',{name:'Finished'}));expect(screen.getAllByTestId('task-card')).toHaveLength(1);await user.click(screen.getByRole('button',{name:'Open task: '+task.goal}));expect(screen.getByLabelText('Task message')).toBeDisabled();expect(screen.getByRole('button',{name:'Send message'})).toBeDisabled();await user.click(screen.getByRole('tab',{name:'Details'}));expect(screen.getByRole('button',{name:'Request cancellation'})).toBeDisabled();expect(sessionApi.sessionsSend).not.toHaveBeenCalled();expect(api.cancel).not.toHaveBeenCalled();
});
it('displays a future Task state as unknown and prevents a stale ready session from enabling execution',async()=>{
  const user=userEvent.setup();vi.mocked(api.snapshot).mockResolvedValue({tasks:[{...task,state:'future_state',acceptance_ref:null,acceptance_sequence:null}],sequence:21,api_version:1,database_schema_version:17});render(<App/>);await user.click(await screen.findByRole('button',{name:'Open task: '+task.goal}));expect(screen.getByText('Unknown status: future_state')).toBeVisible();expect(screen.getByLabelText('Task message')).toBeDisabled();await user.click(screen.getByRole('tab',{name:'Details'}));expect(screen.getByRole('button',{name:'Request cancellation'})).toBeDisabled();
});
