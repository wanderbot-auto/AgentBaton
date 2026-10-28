import { render,screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect,it } from 'vitest';
import { GoalGraphExecutionHistory } from './GoalGraphExecutionHistory';
import { goalGraphFixture,graphCandidateView,graphAcceptedFixture } from './goal-graph-test-fixtures';
it('presents exact candidate artifacts as evidence while leaving Task acceptance pending',async()=>{
  const user=userEvent.setup(),f=goalGraphFixture(),view=graphCandidateView(f);render(<GoalGraphExecutionHistory view={view} language="en"/>);
  expect(screen.getByRole('status')).toHaveTextContent('Task acceptance pending');expect(screen.queryByText('Task accepted')).toBeNull();
  await user.click(screen.getByText('Candidate result · 1',{selector:'summary'}));expect(screen.getByText('a'.repeat(64),{selector:'code'})).toBeVisible();expect(screen.getByText('12',{selector:'dd'})).toBeVisible();expect(screen.getByText('/generated/workspace',{selector:'dd'})).toBeVisible();expect(screen.getByText('outputs/report.txt',{selector:'dd'})).toBeVisible();expect(screen.getByText('File integrity, declared schemas and successful commands do not establish semantic acceptance.')).toBeVisible();expect(screen.queryByRole('button')).toBeNull();expect(screen.queryByRole('link')).toBeNull();
});
it('requires a trusted matching accepted Task projection as well as versioned graph acceptance',()=>{
  const f=goalGraphFixture(),accepted=graphAcceptedFixture(f),view=accepted.view;const mounted=render(<GoalGraphExecutionHistory view={view} language="en"/>);expect(screen.getByRole('status')).toHaveTextContent('Task acceptance pending');mounted.rerender(<GoalGraphExecutionHistory view={view} language="en" task={accepted.task} scope={f.scope}/>);expect(screen.getByRole('status')).toHaveTextContent(/^Task accepted$/);
  mounted.rerender(<GoalGraphExecutionHistory view={{...view,task_accepted:false}} language="en" task={accepted.task} scope={f.scope}/>);expect(screen.getByRole('status')).toHaveTextContent('Task acceptance pending');mounted.rerender(<GoalGraphExecutionHistory view={{...view,graph_contract_version:2}} language="en" task={accepted.task} scope={f.scope}/>);expect(screen.getByRole('status')).toHaveTextContent('Task acceptance pending');mounted.rerender(<GoalGraphExecutionHistory view={view} language="en" task={{...accepted.task,acceptance_sequence:21}} scope={f.scope}/>);expect(screen.getByRole('status')).toHaveTextContent('Task acceptance pending');

});
it('keeps unknown states and observed usage distinct from confirmation of effects',async()=>{
  const user=userEvent.setup(),f=goalGraphFixture(),view=graphCandidateView(f);view.nodes[0].state='future_unknown_effect';view.nodes[0].actual_output_bytes=null;render(<GoalGraphExecutionHistory view={view} language="en"/>);await user.click(screen.getByText(/Unknown \/ unsupported: future_unknown_effect/,{selector:'summary'}));expect(screen.getByText('Observed selected output file bytes')).toBeVisible();expect(screen.getAllByText('Not measured / unknown').length).toBeGreaterThan(0);expect(screen.getByText('Cancellation requested is distinct from confirmed stop; stopped processes can still have unknown effects.')).toBeVisible();expect(screen.queryByRole('button')).toBeNull();
});
it('renders untrusted goals and artifact metadata as text and never opens their paths',async()=>{
  const user=userEvent.setup(),f=goalGraphFixture(),view=graphCandidateView(f);view.nodes[0].node.goal='<img src=x onerror=alert(1)>';view.candidates[0].artifacts[0].relative_path='javascript:alert(1)';const {container}=render(<GoalGraphExecutionHistory view={view} language="en"/>);await user.click(screen.getByText(/^work · Candidate result/,{selector:'summary'}));expect(screen.getByText('<img src=x onerror=alert(1)>')).toBeVisible();expect(container.querySelector('img')).toBeNull();expect(container.querySelector('a')).toBeNull();
});
