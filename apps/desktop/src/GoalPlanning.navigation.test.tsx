import { fireEvent,render,screen,waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach,expect,it,vi } from 'vitest';
import { GoalPlanning } from './GoalPlanning';
import { goalApi } from './goal-api';
import { goalExecutionApi } from './goal-execution-api';
import { goalGraphApi } from './goal-graph-api';
import { goalExecutionFixture } from './goal-execution-test-fixtures';
vi.mock('./goal-api',async original=>({...await original<typeof import('./goal-api')>(),goalApi:{grantsList:vi.fn(),scopesList:vi.fn(),plansList:vi.fn(),approve:vi.fn(),revoke:vi.fn(),propose:vi.fn(),revise:vi.fn(),preview:vi.fn()}}));
vi.mock('./goal-execution-api',async original=>({...await original<typeof import('./goal-execution-api')>(),goalExecutionApi:{list:vi.fn(),get:vi.fn(),approve:vi.fn(),cancel:vi.fn(),advancePlan:vi.fn()}}));
vi.mock('./goal-graph-api',async original=>({...await original<typeof import('./goal-graph-api')>(),goalGraphApi:{list:vi.fn(),get:vi.fn(),approve:vi.fn(),cancel:vi.fn(),advancePlan:vi.fn(),approveNode:vi.fn(),rejectNode:vi.fn()}}));
beforeEach(()=>{vi.resetAllMocks();vi.mocked(goalApi.grantsList).mockResolvedValue([]);vi.mocked(goalExecutionApi.list).mockResolvedValue([]);vi.mocked(goalGraphApi.list).mockResolvedValue([]);});
function ready(){const f=goalExecutionFixture();vi.mocked(goalApi.scopesList).mockResolvedValue([f.scope]);vi.mocked(goalApi.plansList).mockResolvedValue([f.plan]);return f;}
function noWrites(){for(const method of [goalApi.approve,goalApi.revoke,goalApi.propose,goalApi.revise,goalExecutionApi.approve,goalExecutionApi.cancel,goalExecutionApi.advancePlan,goalGraphApi.approve,goalGraphApi.cancel,goalGraphApi.advancePlan,goalGraphApi.approveNode,goalGraphApi.rejectNode])expect(method).not.toHaveBeenCalled();expect(goalApi.preview).not.toHaveBeenCalled();}
it('reopens plan without losing unsaved text or triggering execution',async()=>{
 const f=ready(),user=userEvent.setup();const view=render(<GoalPlanning task={f.task} language="en" connected navigation={{target:'plan',requestId:'first'}}/>);
 const input=await screen.findByLabelText('Plan proposal JSON');await waitFor(()=>expect(input).toBeVisible());expect(screen.getByTestId('goal-plan-proposal')).toHaveAttribute('open');expect(screen.getByTestId('goal-execution').querySelector('details')).not.toHaveAttribute('open');
 fireEvent.change(input,{target:{value:'unsubmitted exact draft'}});const criteria=screen.getByLabelText('Owner acceptance criteria');fireEvent.change(criteria,{target:{value:'unsaved acceptance text'}});
 await user.click(screen.getByText('Versioned plan proposal',{selector:'summary'}));await user.click(screen.getByRole('heading',{name:'Goal scope and plan'}));
 view.rerender(<GoalPlanning task={f.task} language="en" connected navigation={{target:'plan',requestId:'second'}}/>);
 await waitFor(()=>expect(input).toBeVisible());expect(input).toHaveValue('unsubmitted exact draft');expect(criteria).toHaveValue('unsaved acceptance text');expect(goalExecutionApi.list).not.toHaveBeenCalled();noWrites();
});
it('reopens real execution history without approving or starting a run',async()=>{
 const f=ready(),user=userEvent.setup();const view=render(<GoalPlanning task={f.task} language="en" connected navigation={{target:'execution',requestId:'first'}}/>);
 await waitFor(()=>expect(goalExecutionApi.list).toHaveBeenCalledWith(f.task.id));const execution=screen.getByTestId('goal-execution');expect(execution).toHaveAttribute('data-execution-mode','legacy');expect(execution.querySelector('details')).toHaveAttribute('open');expect(screen.getByTestId('goal-plan-proposal')).not.toHaveAttribute('open');noWrites();
 await user.click(screen.getByRole('heading',{name:'Goal execution'}));expect(execution.querySelector('details')).not.toHaveAttribute('open');view.rerender(<GoalPlanning task={f.task} language="en" connected navigation={{target:'execution',requestId:'second'}}/>);await waitFor(()=>expect(execution.querySelector('details')).toHaveAttribute('open'));noWrites();
});
it('preserves lazy collapsed defaults',()=>{const f=ready();render(<GoalPlanning task={f.task} language="en" connected/>);expect(screen.getByTestId('goal-planning').querySelector('details')).not.toHaveAttribute('open');expect(goalApi.grantsList).not.toHaveBeenCalled();expect(goalApi.scopesList).not.toHaveBeenCalled();noWrites();});
it('personal navigation explains prerequisites without authority reads',()=>{const f=ready();render(<GoalPlanning task={{...f.task,project_id:'local'}} language="en" connected navigation={{target:'execution',requestId:'personal'}}/>);expect(screen.getByText(/this personal task has no project execution scope/)).toBeVisible();expect(goalApi.grantsList).not.toHaveBeenCalled();expect(goalApi.scopesList).not.toHaveBeenCalled();noWrites();});
it('disconnected navigation does not read or mutate',async()=>{const f=ready();render(<GoalPlanning task={f.task} language="en" connected={false} navigation={{target:'plan',requestId:'offline'}}/>);await waitFor(()=>expect(screen.getByRole('alert')).toBeVisible());expect(goalApi.grantsList).not.toHaveBeenCalled();expect(goalApi.scopesList).not.toHaveBeenCalled();noWrites();});
