import {fireEvent,render,screen,waitFor} from '@testing-library/react';
import {expect,it,vi} from 'vitest';
import {GoalPlanning} from './GoalPlanning';
import {goalApi} from './goal-api';
import {multiDraftFixture} from './goal-multi-plan-draft-test-fixtures';
import type {ExecutionGrant} from './session-api';
const callbacks=vi.hoisted(()=>({single:(_text:string)=>{},multi:(_text:string)=>{}}));
vi.mock('./GoalPlanDraft',()=>({GoalPlanDraft:({onApply}:{onApply:(text:string)=>void})=>{callbacks.single=onApply;return null;}}));
vi.mock('./GoalMultiPlanDraft',()=>({GoalMultiPlanDraft:({onApply}:{onApply:(text:string)=>void})=>{callbacks.multi=onApply;return <button onClick={()=>{callbacks.single('first reviewed draft');callbacks.multi('competing reviewed draft');}}>Apply competing candidates</button>;}}));
vi.mock('./goal-api',async original=>({...await original<typeof import('./goal-api')>(),goalApi:{grantsList:vi.fn(),scopesList:vi.fn(),plansList:vi.fn(),propose:vi.fn(),approve:vi.fn()}}));
it('atomically fills only empty raw when single and multi candidate callbacks compete',async()=>{
 const f=multiDraftFixture();const grant={id:f.scope.root_grant_id,root_grant_id:f.scope.root_grant_id,parent_id:null,project_id:f.task.project_id,revision:2,expires_at_ms:f.scope.expires_at_ms,revoked:false,scope:{workspace_ids:['workspace'],connection_ids:['connection'],host_ids:['local'],max_attempts:4,max_concurrency:1,max_duration_ms:10000,max_children:3,max_depth:1}} as ExecutionGrant;
 vi.mocked(goalApi.grantsList).mockResolvedValue([grant]);vi.mocked(goalApi.scopesList).mockResolvedValue([f.scope]);vi.mocked(goalApi.plansList).mockResolvedValue([]);
 render(<GoalPlanning task={f.task} language="en" connected navigation={{target:'plan',requestId:'first'}}/>);await screen.findByRole('button',{name:'Apply competing candidates'});
 fireEvent.click(screen.getByRole('button',{name:'Apply competing candidates'}));await waitFor(()=>expect(screen.getByLabelText('Plan proposal JSON')).toHaveValue('first reviewed draft'));
 fireEvent.change(screen.getByLabelText('Plan proposal JSON'),{target:{value:'User edited draft'}});fireEvent.click(screen.getByRole('button',{name:'Apply competing candidates'}));expect(screen.getByLabelText('Plan proposal JSON')).toHaveValue('User edited draft');expect(goalApi.propose).not.toHaveBeenCalled();expect(goalApi.approve).not.toHaveBeenCalled();
});
