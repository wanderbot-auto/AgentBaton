import { render,screen,waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach,expect,it,vi } from 'vitest';
import { GoalExecution } from './GoalExecution';
import { goalExecutionApi as api } from './goal-execution-api';
import { goalExecutionFixture,executionView } from './goal-execution-test-fixtures';
import { goalApi } from './goal-api';
vi.mock('./goal-execution-api',async original=>({...await original<typeof import('./goal-execution-api')>(),goalExecutionApi:{list:vi.fn(),get:vi.fn(),approve:vi.fn(),cancel:vi.fn(),advancePlan:vi.fn()}}));
vi.mock('./goal-api',async original=>({...await original<typeof import('./goal-api')>(),goalApi:{preview:vi.fn()}}));
beforeEach(()=>vi.resetAllMocks());
it('lets an old unaccepted candidate request cancellation with exact run CAS without treating request as stop or Task acceptance',async()=>{
  const user=userEvent.setup(),f=goalExecutionFixture(),candidate={...f.run,state:'awaiting_acceptance',revision:3},requested={...candidate,state:'cancel_requested',cancel_requested:true,revision:4};
  vi.mocked(api.list).mockResolvedValue([candidate]);vi.mocked(api.get).mockResolvedValue(executionView(f,candidate));vi.mocked(api.cancel).mockImplementation(async()=>{vi.mocked(api.list).mockResolvedValue([requested]);vi.mocked(api.get).mockResolvedValue({...executionView(f,requested),nodes:[{...executionView(f,requested).nodes[0],state:'outcome_unknown',stop_confirmed:false}]});return requested;});
  render(<GoalExecution task={f.task} scope={f.scope} latest={f.plan} language="en" connected approvalAllowed/>);await user.click(screen.getByRole('heading',{name:'Goal execution'}));const cancel=await screen.findByRole('button',{name:'Request Goal run cancellation'});await waitFor(()=>expect(cancel).toBeEnabled());await user.click(cancel);await waitFor(()=>expect(api.cancel).toHaveBeenCalledOnce());expect(api.cancel).toHaveBeenCalledWith(candidate.id,3,expect.any(String));expect(await screen.findByText('Cancellation requested; stop not yet confirmed · '+candidate.id,{selector:'summary'})).toBeVisible();expect(screen.getByRole('button',{name:'Review Goal execution in native dialog'})).toBeDisabled();expect(screen.queryByText('Task accepted')).toBeNull();expect(goalApi.preview).not.toHaveBeenCalled();expect(api.approve).not.toHaveBeenCalled();
});
