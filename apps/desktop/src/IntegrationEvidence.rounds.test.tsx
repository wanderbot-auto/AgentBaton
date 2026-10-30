import { render,screen,within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe,expect,it } from 'vitest';
import { IntegrationRunEvidence } from './IntegrationEvidence';
import { integrationMessages } from './integration-i18n';
import { integrationFixture } from './integration-test-fixtures';
import type { IntegrationAttemptView,IntegrationRunView,IntegrationValidationReceipt } from './integration-types';

/** Renderer fixtures only: these records do not prove Core/IPC or Git execution. */
function historyFixture():IntegrationRunView{
 const f=integrationFixture();
 const receipt=(attemptId:string,round:number,passed:boolean,commit:string,tree:string):IntegrationValidationReceipt=>({
  attempt_id:attemptId,round,validator:f.validator,definition_digest:'d'.repeat(64),
  integration_commit:commit,integration_tree:tree,result_digest:'e'.repeat(64),passed,
  stop_confirmed:true,worker_joined:true,actual_runtime_ms:1025,actual_output_bytes:0,
  details_ref:`validation-${round}`,sequence:10+round,
 });
 const receipts=[receipt('first-check',1,false,'a'.repeat(40),'b'.repeat(40)),receipt('second-check',2,false,'c'.repeat(40),'d'.repeat(40)),receipt('final-check',3,true,'e'.repeat(40),'f'.repeat(40))];
 const attempt=(attemptId:string,round:number):IntegrationAttemptView=>({
  attempt_id:attemptId,round,kind:'validator',validator:f.validator,
  state:round===3?'passed':'failed',reservation:{...f.reservation},settled:true,
  actual_runtime_ms:1025,actual_output_bytes:0,io_completion:'worker_joined',last_error:null,receipt_sequence:10+round,
 });
 return {...f.view,run:{...f.view.run,round:3,state:'confirmed_local_ref',integration_commit:receipts[2].integration_commit,integration_tree:receipts[2].integration_tree,validation_receipts:receipts},attempts:[attempt('first-check',1),attempt('second-check',2),attempt('final-check',3)]};
}

describe('Integration round history, mocked renderer records only',()=>{
 it.each(['en','zh'] as const)('keeps failed earlier candidates beside the confirmed round in %s',async language=>{
  const view=historyFixture(),original=JSON.stringify(view),t=integrationMessages[language];
  render(<IntegrationRunEvidence view={view} language={language}/>);
  await userEvent.setup().click(screen.getByText(t.receipt,{selector:'summary'}));
  expect(screen.getByRole('heading',{name:t.accepted})).toBeVisible();
  for(const receipt of view.run.validation_receipts){
   const group=screen.getByRole('group',{name:`${t.receipt} · ${t.round} ${receipt.round} · ${receipt.attempt_id}`});
   expect(group).toBeVisible();
   expect(within(group).getByText(t.round,{selector:'dt'})).toBeVisible();
   expect(within(group).getByText(String(receipt.round),{selector:'dd',exact:true})).toBeVisible();
   expect(within(group).getByText(receipt.passed?'passed · stop_confirmed=true · worker_joined=true':'failed · stop_confirmed=true · worker_joined=true')).toBeVisible();
   expect(within(group).getByText(receipt.integration_commit,{exact:false})).toBeVisible();
   expect(within(group).getByText(receipt.integration_tree,{exact:false})).toBeVisible();
   for(const other of view.run.validation_receipts.filter(value=>value.round!==receipt.round))expect(within(group).queryByText(other.integration_commit,{exact:false})).toBeNull();
   const attempt=screen.getByRole('group',{name:`${t.attempts} · ${t.round} ${receipt.round} · ${receipt.attempt_id}`});
   expect(attempt).toBeVisible();
   expect(within(attempt).getByText(String(receipt.round),{selector:'dd',exact:true})).toBeVisible();
   expect(within(attempt).getByText(`${receipt.passed?'passed':'failed'} · worker_joined · settled`)).toBeVisible();
  }
  expect(JSON.stringify(view)).toBe(original);
 });

 it.each(['en','zh'] as const)('preserves an unsettled round and unknown measurements alongside measured zero in %s',async language=>{
  const history=historyFixture(),t=integrationMessages[language];
  const pendingReceipt={...history.run.validation_receipts[2],passed:false,stop_confirmed:false,worker_joined:false,actual_runtime_ms:null,actual_output_bytes:null};
  const pendingAttempt:IntegrationAttemptView={...history.attempts[2],state:'outcome_unknown',settled:false,actual_runtime_ms:null,actual_output_bytes:null,io_completion:null,receipt_sequence:null};
  const view:IntegrationRunView={...history,run:{...history.run,state:'outcome_unknown',validation_receipts:[history.run.validation_receipts[0],pendingReceipt]},attempts:[history.attempts[0],pendingAttempt]};
  const original=JSON.stringify(view);
  render(<IntegrationRunEvidence view={view} language={language}/>);
  await userEvent.setup().click(screen.getByText(t.receipt,{selector:'summary'}));
  expect(screen.getByRole('heading',{name:t.notAccepted})).toBeVisible();
  const receipt=screen.getByRole('group',{name:`${t.receipt} · ${t.round} 3 · final-check`});
  expect(within(receipt).getByText('3',{selector:'dd',exact:true})).toBeVisible();
  expect(within(receipt).getAllByText(t.unknown,{selector:'dd'})).toHaveLength(2);
  expect(within(receipt).queryByText('0',{selector:'dd',exact:true})).toBeNull();
  expect(within(receipt).getByText('failed · stop_confirmed=false · worker_joined=false')).toBeVisible();
  const attempt=screen.getByRole('group',{name:`${t.attempts} · ${t.round} 3 · final-check`});
  expect(within(attempt).getByText(`outcome_unknown · ${t.unknown} · unsettled`)).toBeVisible();
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
