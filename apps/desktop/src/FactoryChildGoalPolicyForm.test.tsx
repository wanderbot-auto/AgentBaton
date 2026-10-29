// Renderer form controls only; no Native approval or execution.
import { it,expect } from 'vitest';
import { render,screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { FactoryChildGoalPolicyForm } from './FactoryChildGoalPolicyForm';
import { factoryFixture } from './factory-test-fixtures';
import { parseFactorySelection,type FactoryForm } from './factory-form';
function Harness({zero=false,language='en' as 'en'|'zh'}){
 const[f]=useState(()=>{const value=factoryFixture();if(!zero){value.grant.scope.max_children=3;value.grant.scope.max_depth=3;}return value;});
 const[form,setForm]=useState<FactoryForm>({...f.form,planning:true,planningNodes:'4',planningCriteria:'Owner accepts this exact report'});
 return <><FactoryChildGoalPolicyForm form={form} grant={f.grant} language={language} onChange={setForm}/><output data-testid="selection">{JSON.stringify(parseFactorySelection(form,f.task,f.grant))}</output></>;
}
it('exposes every independent execution budget and fixed once-only/Owner policy without changing the original Grant',async()=>{
 const user=userEvent.setup();render(<Harness/>);await user.click(screen.getByText('Child Goal execution policy',{selector:'summary'}));await user.click(screen.getByLabelText('Request separate bounded child Goal execution in this Native Factory review'));expect(screen.getByLabelText('Maximum runs per task (fixed)')).toHaveValue('1');expect(screen.getByLabelText('Maximum runs per task (fixed)')).toHaveAttribute('readonly');expect(screen.getAllByRole('group',{name:/Child Goal (root|project) budget/})).toHaveLength(2);expect(screen.getByText('Exact candidate Owner acceptance remains required')).toBeVisible();const wire=JSON.parse(screen.getByTestId('selection').textContent!);expect(wire.child_goal_execution_policy).toEqual({max_nodes:2,max_runs_per_task:1,max_total_runs:1,root_budget:wire.root_budget,project_budget:wire.project_budget,require_owner_acceptance:true});expect(wire.capabilities.create_workspace).toBe(false);
});
it('keeps zero-child Grant planning-only and displays the reason bilingually',async()=>{
 const user=userEvent.setup();render(<Harness zero language="zh"/>);await user.click(screen.getByText('子 Goal 独立执行策略',{selector:'summary'}));expect(screen.getByLabelText('在此次原生 Factory 审查中单独请求有界子 Goal 执行')).toBeDisabled();expect(screen.getByText(/children 为零/)).toBeVisible();const wire=JSON.parse(screen.getByTestId('selection').textContent!);expect(wire.capabilities.child_planning).toBe(true);expect(wire.capabilities.child_goal_execution).toBe(false);expect(Object.hasOwn(wire,'child_goal_execution_policy')).toBe(false);
});
