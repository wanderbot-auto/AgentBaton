import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { GoalPlanning } from './GoalPlanning';
import { goalApi } from './goal-api';
import { goalExecutionApi } from './goal-execution-api';
import { goalExecutionFixture } from './goal-execution-test-fixtures';
import type { ExecutionGrant } from './session-api';
vi.mock('./goal-api',async original=>({...await original<typeof import('./goal-api')>(),goalApi:{grantsList:vi.fn(),scopesList:vi.fn(),plansList:vi.fn(),preview:vi.fn()}}));
vi.mock('./goal-execution-api',async original=>({...await original<typeof import('./goal-execution-api')>(),goalExecutionApi:{list:vi.fn(),get:vi.fn(),approve:vi.fn(),cancel:vi.fn(),advancePlan:vi.fn()}}));
it('mounts execution as a separate folded control in the real Goal panel and leaves existing planning reads readonly',async()=>{
  const f=goalExecutionFixture(),user=userEvent.setup();
  const grant:ExecutionGrant={id:f.scope.root_grant_id,root_grant_id:f.scope.root_grant_id,parent_id:null,depth:0,project_id:'project',revision:2,approval_id:'original-native',approved_at:f.run.created_at,expires_at_ms:f.scope.expires_at_ms,revoked:false,scope:{workspace_ids:['workspace'],connection_ids:['connection'],host_ids:['local'],max_attempts:4,max_concurrency:1,max_duration_ms:10000,recipients:[],network:{mode:'inherited'},max_children:3,max_depth:1},enforcement:{baton_mediated_scope:true,os_filesystem_sandbox:false,os_network_sandbox:false,native_identity_isolation:false}};
  vi.mocked(goalApi.grantsList).mockResolvedValue([grant]);vi.mocked(goalApi.scopesList).mockResolvedValue([f.scope]);vi.mocked(goalApi.plansList).mockResolvedValue([f.plan]);vi.mocked(goalExecutionApi.list).mockResolvedValue([]);
  render(<GoalPlanning task={f.task} language="en" connected/>);await user.click(screen.getByRole('heading',{name:'Goal scope and plan'}));
  await waitFor(()=>expect(screen.getByRole('heading',{name:'Goal execution'})).toBeVisible());expect(goalExecutionApi.list).not.toHaveBeenCalled();expect(goalExecutionApi.approve).not.toHaveBeenCalled();expect(goalApi.preview).not.toHaveBeenCalled();
  await user.click(screen.getByRole('heading',{name:'Goal execution'}));await waitFor(()=>expect(goalExecutionApi.list).toHaveBeenCalledWith(f.task.id));expect(goalExecutionApi.approve).not.toHaveBeenCalled();expect(goalExecutionApi.advancePlan).not.toHaveBeenCalled();
});
