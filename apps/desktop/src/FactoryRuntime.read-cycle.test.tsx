// Actual parent React wiring with mocked read transport, not IPC/Core evidence.
import { act,render,screen,waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach,expect,it,vi } from 'vitest';
import { FactoryRuntime } from './FactoryRuntime';
import { factoryApi } from './factory-api';
import { api } from './api';
import { sessionApi } from './session-api';
import { childPlanningFixture } from './factory-child-plan-test-fixtures';
import { factoryMessages } from './factory-i18n';
import type { FactoryRunDetails } from './factory-types';
vi.mock('./FactoryChildPlans',()=>({FactoryChildPlans:(p:{parentReadGeneration?:number;parentReady:boolean})=><output data-testid='child-read-cycle' data-generation={p.parentReadGeneration} data-ready={String(p.parentReady)}/> }));
vi.mock('./factory-api',()=>({factoryApi:{list:vi.fn(),run:vi.fn(),approve:vi.fn(),start:vi.fn(),cancel:vi.fn(),revoke:vi.fn()}}));
vi.mock('./api',async original=>({...await original<typeof import('./api')>(),api:{workspacesList:vi.fn()}}));
vi.mock('./session-api',()=>({sessionApi:{grantsList:vi.fn(),connectionsList:vi.fn()}}));
beforeEach(()=>vi.resetAllMocks());
it('starts one explicit child generation with each existing parent refresh and keeps readiness withdrawn until its complete reply',async()=>{
 const f=childPlanningFixture(),u=userEvent.setup();
 // Terminal history uses the same refresh method without a live timer racing
 // this held-response wiring test. Active polling's original cadence is intact.
 f.detail.run.state='completed';f.detail.run.active_session_id=null;f.detail.tasks[0].state='result_available';f.detail.tasks[0].attempt_id='completed-root-attempt';f.detail.tasks[0].confirmed_stop=true;f.detail.tasks[0].result_available=true;
 vi.mocked(sessionApi.grantsList).mockImplementation(async()=>[structuredClone(f.grant)]);
 vi.mocked(sessionApi.connectionsList).mockImplementation(async()=>[structuredClone(f.connection)]);
 vi.mocked(api.workspacesList).mockImplementation(async()=>[structuredClone(f.workspace)]);
 vi.mocked(factoryApi.list).mockImplementation(async()=>[structuredClone(f.scope)]);
 vi.mocked(factoryApi.run).mockImplementation(async()=>structuredClone(f.detail));
 render(<FactoryRuntime task={f.task} language='en' connected/>);await u.click(screen.getByRole('heading',{name:factoryMessages.en.title}));
 await waitFor(()=>expect(screen.getByTestId('child-read-cycle')).toHaveAttribute('data-ready','true'));
 const before=Number(screen.getByTestId('child-read-cycle').getAttribute('data-generation'));expect(before).toBeGreaterThan(0);
 let release!:(value:FactoryRunDetails)=>void;vi.mocked(factoryApi.run).mockImplementationOnce(()=>new Promise(resolve=>{release=resolve;}));
 await u.click(screen.getByRole('button',{name:factoryMessages.en.refresh}));await waitFor(()=>expect(factoryApi.run).toHaveBeenCalledTimes(2));
 const next=Number(screen.getByTestId('child-read-cycle').getAttribute('data-generation'));expect(next).toBeGreaterThan(before);expect(screen.getByTestId('child-read-cycle')).toHaveAttribute('data-ready','false');
 await act(async()=>release(structuredClone(f.detail)));await waitFor(()=>expect(screen.getByTestId('child-read-cycle')).toHaveAttribute('data-ready','true'));
 expect(screen.getByTestId('child-read-cycle')).toHaveAttribute('data-generation',String(next));expect(factoryApi.start).not.toHaveBeenCalled();expect(factoryApi.approve).not.toHaveBeenCalled();
});
