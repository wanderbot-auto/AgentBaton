import {fireEvent,render,screen,waitFor,within} from '@testing-library/react';
import {expect,it,vi} from 'vitest';
import {GoalPlanning} from './GoalPlanning';
import {goalApi} from './goal-api';
import {goalMultiPlanDraftApi} from './goal-multi-plan-draft-api';
import {oneStepDraftFixture} from './goal-multi-plan-draft-test-fixtures';
import {goalPlanDraftApi} from './goal-plan-draft-api';
import {goalDraftFixture} from './goal-plan-draft-test-fixtures';
import type {ExecutionGrant} from './session-api';
vi.mock('./goal-api',async original=>({...await original<typeof import('./goal-api')>(),goalApi:{grantsList:vi.fn(),scopesList:vi.fn(),plansList:vi.fn(),propose:vi.fn(),approve:vi.fn(),preview:vi.fn()}}));
vi.mock('./goal-plan-draft-api',async original=>({...await original<typeof import('./goal-plan-draft-api')>(),goalPlanDraftApi:{generate:vi.fn()}}));
vi.mock('./goal-multi-plan-draft-api',async original=>({...await original<typeof import('./goal-multi-plan-draft-api')>(),goalMultiPlanDraftApi:{generate:vi.fn()}}));
it('applies reviewed initial draft in the real Goal panel without proposing or approving execution',async()=>{
 const f=goalDraftFixture();const grant={id:f.scope.root_grant_id,root_grant_id:f.scope.root_grant_id,parent_id:null,project_id:f.task.project_id,revision:2,expires_at_ms:f.scope.expires_at_ms,revoked:false,scope:{workspace_ids:['workspace'],connection_ids:['connection'],host_ids:['local'],max_attempts:4,max_concurrency:1,max_duration_ms:10000,max_children:3,max_depth:1}} as ExecutionGrant;
 vi.mocked(goalApi.grantsList).mockResolvedValue([grant]);vi.mocked(goalApi.scopesList).mockResolvedValue([f.scope]);vi.mocked(goalApi.plansList).mockResolvedValue([]);vi.mocked(goalPlanDraftApi.generate).mockResolvedValue(f.draft);
 render(<GoalPlanning task={f.task} language="en" connected navigation={{target:'plan',requestId:'first'}}/>);
 fireEvent.click(await screen.findByText('Compatibility: work-node-only template',{selector:'summary'}));await waitFor(()=>expect(screen.getByRole('button',{name:'Generate initial draft'})).toBeEnabled());expect(screen.getByText(/This compatibility template contains only a work node/)).toBeVisible();
 for(const [label,value] of [['Risks to review','Review overwritten files'],['Prohibited actions','Do not publish'],['Step runtime limit (ms)','1000'],['Step output limit (bytes)','4096']])fireEvent.change(screen.getByLabelText(label),{target:{value}});
 fireEvent.click(screen.getByRole('button',{name:'Generate initial draft'}));await screen.findByText(f.draft.approved_acceptance_criteria);
 for(const label of ['I reviewed the goal and approved output scope.','I reviewed the risks, prohibited actions and limits.','I reviewed the approved acceptance criteria.'])fireEvent.click(screen.getByLabelText(label));
 fireEvent.click(screen.getByRole('button',{name:'Use reviewed draft'}));
 await waitFor(()=>expect(screen.getByLabelText('Plan proposal JSON')).toHaveValue(JSON.stringify(f.draft.plan,null,2)));
 expect(goalApi.propose).not.toHaveBeenCalled();expect(goalApi.approve).not.toHaveBeenCalled();expect(goalApi.preview).not.toHaveBeenCalled();expect(within(screen.getByRole('region',{name:'Initial plan from this goal'})).getByText('An existing plan or edited draft is present. It will not be overwritten.')).toBeVisible();
});

it('prioritizes a complete one-step draft with validator and owner nodes while folding the compatibility template',async()=>{
 const f=oneStepDraftFixture(true);const grant={id:f.scope.root_grant_id,root_grant_id:f.scope.root_grant_id,parent_id:null,project_id:f.task.project_id,revision:2,expires_at_ms:f.scope.expires_at_ms,revoked:false,scope:{workspace_ids:['workspace'],connection_ids:['connection'],host_ids:['local'],max_attempts:5,max_concurrency:1,max_duration_ms:10000,max_children:3,max_depth:1}} as ExecutionGrant;
 vi.mocked(goalApi.grantsList).mockResolvedValue([grant]);vi.mocked(goalApi.scopesList).mockResolvedValue([f.scope]);vi.mocked(goalApi.plansList).mockResolvedValue([]);vi.mocked(goalMultiPlanDraftApi.generate).mockResolvedValue(f.draft);
 render(<GoalPlanning task={f.task} language="en" connected navigation={{target:'plan',requestId:'complete'}}/>);await waitFor(()=>expect(screen.getByRole('button',{name:'Generate step plan draft'})).toBeEnabled());expect(screen.getByText('Compatibility: work-node-only template',{selector:'summary'}).closest('details')).not.toHaveAttribute('open');expect(screen.getByText('Generate initial draft',{selector:'button'})).not.toBeVisible();expect(goalMultiPlanDraftApi.generate).not.toHaveBeenCalled();
 for(const [label,value] of [['Step-plan risks to review','Review overwritten files'],['Step-plan prohibited actions','Do not publish'],['Work step goal step-1','Write report'],['Runtime limit (ms) step-1','1000'],['Output limit (bytes) step-1','4096'],['Runtime per review node (ms) step-1','100'],['Output per review node (bytes) step-1','256']])fireEvent.change(screen.getByLabelText(label),{target:{value}});
 fireEvent.click(screen.getByLabelText('Produced artifacts step-1: report · outputs/report.txt'));fireEvent.click(screen.getByRole('button',{name:'Generate step plan draft'}));await screen.findByText(f.draft.approved_acceptance_criteria);expect(screen.getByText('baton-review-1-2: Required review · rule-validator')).toBeVisible();
 for(const label of ['I reviewed all step goals, dependencies and artifact mappings.','I reviewed risks, prohibited actions and the total limits.','I reviewed the approved acceptance criteria and upstream waiting.'])fireEvent.click(screen.getByLabelText(label));fireEvent.click(screen.getByRole('button',{name:'Use reviewed step plan'}));expect(screen.getByLabelText('Plan proposal JSON')).toHaveValue(JSON.stringify(f.draft.plan,null,2));expect(goalApi.propose).not.toHaveBeenCalled();expect(goalApi.approve).not.toHaveBeenCalled();expect(goalApi.preview).not.toHaveBeenCalled();
});
