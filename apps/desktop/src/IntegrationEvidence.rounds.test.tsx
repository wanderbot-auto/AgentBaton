import { render,screen,within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe,expect,it } from 'vitest';
import { IntegrationRunEvidence } from './IntegrationEvidence';
import { integrationMessages } from './integration-i18n';
import { integrationFixture } from './integration-test-fixtures';
import type { IntegrationAttemptView,IntegrationPolicy,IntegrationRunView,IntegrationValidationReceipt } from './integration-types';

/** Consumer-contract mocks, not observed Core history or approval/target-drift proof. */
function historyFixture():{policy:IntegrationPolicy;view:IntegrationRunView}{
 const f=integrationFixture();
 const budget={...f.policy.bindings.root_budget,attempts:12,runtime_ms:20000,reworks:2};
 const reservation={...f.reservation,runtime_ms:2000};
 const expiresAt=Date.parse(f.policy.created_at)+30000;
 // An independent three-round fixture; never mutate the shared one-round policy.
 const policy:IntegrationPolicy={...f.policy,selection:{...f.policy.selection,ttl_ms:30000,max_rounds:3,on_target_advance:'bounded_descendant_reprepare',stage_reservations:{preparation:{...reservation},validators:[{validator:f.validator,reservation:{...reservation}}],cas:{...reservation}}},expires_at_ms:expiresAt,bindings:{...f.policy.bindings,goal_scope_expires_at_ms:expiresAt,grant_lineage:f.policy.bindings.grant_lineage.map(grant=>({...grant,expires_at_ms:expiresAt,scope:{...grant.scope,max_duration_ms:30000}})),root_budget:{...budget},project_budget:{...budget}}};
 const receipt=(attemptId:string,round:number,passed:boolean,commit:string,tree:string,sequence:number):IntegrationValidationReceipt=>({
  attempt_id:attemptId,round,validator:f.validator,definition_digest:'d'.repeat(64),
  integration_commit:commit,integration_tree:tree,result_digest:String(round).repeat(64),passed,
  stop_confirmed:true,worker_joined:true,actual_runtime_ms:1025,actual_output_bytes:0,
  details_ref:`integration-event:${sequence}`,sequence,
 });
 const receipts=[receipt('first-check',1,false,'a'.repeat(40),'b'.repeat(40),11),receipt('second-check',2,false,'c'.repeat(40),'d'.repeat(40),21),receipt('final-check',3,true,'e'.repeat(40),'f'.repeat(40),31)];
 const attempt=(attemptId:string,round:number,kind:IntegrationAttemptView['kind'],runtime:number,output:number,sequence:number):IntegrationAttemptView=>({
  attempt_id:attemptId,round,kind,validator:kind==='validator'?f.validator:null,
  state:'finished',reservation:{...reservation},settled:true,
  actual_runtime_ms:runtime,actual_output_bytes:output,io_completion:'worker_joined',last_error:null,receipt_sequence:sequence,
 });
 const attempts=[
  attempt('first-preparation',1,'preparation',100,12,10),attempt('first-check',1,'validator',1025,0,12),
  attempt('second-preparation',2,'preparation',100,12,20),attempt('second-check',2,'validator',1025,0,22),
  attempt('final-preparation',3,'preparation',100,12,30),attempt('final-check',3,'validator',1025,0,32),
  attempt('local-ref-update',3,'cas',50,20,40),
 ];
 // One earlier Graph Work (200ms/64B), plus all seven settled integration stages.
 const usage={attempts:8,active:0,children:0,runtime_ms:200+100+1025+100+1025+100+1025+50,output_bytes:64+12+0+12+0+12+0+20,tokens:null,cost_microunits:null};
 const view:IntegrationRunView={...f.view,run:{...f.view.run,revision:22,round:3,state:'confirmed_local_ref',source_commit:'1'.repeat(40),source_tree:'2'.repeat(40),integration_commit:receipts[2].integration_commit,integration_tree:receipts[2].integration_tree,active_stage:null,io_completion:'worker_joined',validation_receipts:receipts},attempts,shared_usage:{root:{...usage},project:{...usage}},fence:{...f.view.fence,revision:14,state:'released'}};
 return {policy,view};
}

describe('Integration round history, mocked renderer records only',()=>{
 it.each(['en','zh'] as const)('keeps failed earlier candidates beside the confirmed round in %s',async language=>{
  const {policy,view}=historyFixture(),original=JSON.stringify(view),t=integrationMessages[language];
  expect(policy.selection.max_rounds).toBe(3);expect(view.attempts).toHaveLength(7);
  expect(policy.bindings.root_budget).toMatchObject({attempts:12,runtime_ms:20000});
  expect(policy.selection.ttl_ms).toBe(30000);expect(policy.expires_at_ms-Date.parse(policy.created_at)).toBe(30000);
  expect(view.shared_usage.root).toMatchObject({attempts:8,active:0,runtime_ms:3625,output_bytes:120,tokens:null,cost_microunits:null});
  render(<IntegrationRunEvidence view={view} language={language}/>);
  await userEvent.setup().click(screen.getByText(t.receipt,{selector:'summary'}));
  expect(screen.getByRole('heading',{name:t.accepted})).toBeVisible();
  for(const receipt of view.run.validation_receipts){
   const group=screen.getByRole('group',{name:`${t.receipt} · ${t.round} ${receipt.round} · ${receipt.attempt_id}`});
   expect(group).toBeVisible();
   expect(within(group).getByText(t.round,{selector:'dt'})).toBeVisible();
   expect(within(group).getByText(String(receipt.round),{selector:'dd',exact:true})).toBeVisible();
   expect(within(group).getByText(receipt.passed?'passed · stop_confirmed=true · worker_joined=true':'failed · stop_confirmed=true · worker_joined=true')).toBeVisible();
   const candidate=within(group).getByText(t.candidate,{selector:'dt'}).nextElementSibling;
   expect(candidate).toBeVisible();
   expect(candidate?.textContent).toBe(receipt.integration_commit+receipt.integration_tree);
   for(const other of view.run.validation_receipts.filter(value=>value.round!==receipt.round))expect(within(group).queryByText(other.integration_commit,{exact:false})).toBeNull();
   const attempt=screen.getByRole('group',{name:`${t.attempts} · ${t.round} ${receipt.round} · ${receipt.attempt_id}`});
   expect(attempt).toBeVisible();
   expect(within(attempt).getByText(String(receipt.round),{selector:'dd',exact:true})).toBeVisible();
   expect(within(attempt).getByText('finished · worker_joined · settled')).toBeVisible();
  }
  expect(JSON.stringify(view)).toBe(original);
 });

 it.each(['en','zh'] as const)('preserves an unsettled round and unknown measurements alongside measured zero in %s',async language=>{
  const {view:history}=historyFixture(),t=integrationMessages[language];
  const unknown={code:'OUTCOME_UNKNOWN',message:'Mock interrupted validator has no joined receipt',retryable:false,side_effect_state:'unknown',details_ref:null};
  const pendingAttempt:IntegrationAttemptView={...history.attempts[5],state:'outcome_unknown',settled:false,actual_runtime_ms:null,actual_output_bytes:null,io_completion:'interrupted_no_join_receipt',receipt_sequence:null,last_error:unknown};
  // Unknown does not manufacture a current validation receipt or a CAS attempt.
  const usage={attempts:7,active:1,children:0,runtime_ms:null,output_bytes:null,tokens:null,cost_microunits:null};
  const view:IntegrationRunView={...history,run:{...history.run,revision:21,state:'outcome_unknown',active_stage:'validator',io_completion:'interrupted_no_join_receipt',last_error:unknown,validation_receipts:history.run.validation_receipts.slice(0,2)},attempts:[...history.attempts.slice(0,5),pendingAttempt],shared_usage:{root:{...usage},project:{...usage}},fence:{...history.fence,revision:13,state:'unknown'}};
  const original=JSON.stringify(view);
  render(<IntegrationRunEvidence view={view} language={language}/>);
  await userEvent.setup().click(screen.getByText(t.receipt,{selector:'summary'}));
  expect(screen.getByRole('heading',{name:t.notAccepted})).toBeVisible();
  expect(screen.queryByRole('group',{name:`${t.receipt} · ${t.round} 3 · final-check`})).toBeNull();
  expect(view.attempts).toHaveLength(6);expect(view.attempts.some(value=>value.kind==='cas')).toBe(false);
  expect(view.shared_usage.root).toMatchObject({attempts:7,active:1,runtime_ms:null,output_bytes:null});
  const attempt=screen.getByRole('group',{name:`${t.attempts} · ${t.round} 3 · final-check`});
  expect(within(attempt).getByText('outcome_unknown · interrupted_no_join_receipt · unsettled')).toBeVisible();
  expect(within(attempt).getAllByText(t.unknown,{selector:'dd',exact:true})).toHaveLength(2);
  expect(within(attempt).queryByText('0',{selector:'dd',exact:true})).toBeNull();
  for(const label of [t.receipt,t.attempts]){
   const prior=screen.getByRole('group',{name:`${label} · ${t.round} 1 · first-check`});
   expect(within(prior).getByText('0',{selector:'dd',exact:true})).toBeVisible();
   expect(within(prior).getByText('1025',{selector:'dd',exact:true})).toBeVisible();
  }
  expect(JSON.stringify(view)).toBe(original);
 });
});
