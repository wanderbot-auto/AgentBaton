import {act,fireEvent,render,screen} from '@testing-library/react';
import {beforeEach,expect,it,vi} from 'vitest';
import {GoalMultiPlanDraft} from './GoalMultiPlanDraft';
import type {GoalAdviceStep} from './goal-plan-advice-api';
import {adviceFixture} from './goal-plan-advice-test-fixtures';
type Captured={formKey:string;formVersion:number;onImport:(steps:GoalAdviceStep[],key:string,version:number)=>boolean};
const capture=vi.hoisted(()=>({value:null as Captured|null}));
vi.mock('./GoalPlanAdvice',()=>({GoalPlanAdvice:(props:Captured)=>{capture.value=props;return null;}}));
beforeEach(()=>{capture.value=null;});
function setup(){const f=adviceFixture();render(<GoalMultiPlanDraft task={f.task} scope={f.scope} latest={null} raw="" language="en" disabled={false} connected onApply={vi.fn()} initialOpen/>);return f;}
it.each(['Work step goal step-1','Step-plan risks to review','Runtime per review node (ms) step-1'])('rejects a same-batch stale import after editing %s',label=>{const f=setup(),old=capture.value!;let result=true;act(()=>{fireEvent.change(screen.getByLabelText(label),{target:{value:'User edit'}});result=old.onImport(f.imported.steps,old.formKey,old.formVersion);});expect(result).toBe(false);expect(screen.getByLabelText(label)).toHaveValue('User edit');expect(screen.queryByLabelText('Work step goal step-2')).not.toBeInTheDocument();});
it('allows only one same-batch import into the pristine form',()=>{const f=setup(),old=capture.value!;let first=false,second=true;act(()=>{first=old.onImport(f.imported.steps,old.formKey,old.formVersion);second=old.onImport([{...f.imported.steps[0],goal:'Overwrite'}],old.formKey,old.formVersion);});expect(first).toBe(true);expect(second).toBe(false);expect(screen.getByLabelText('Work step goal step-1')).toHaveValue('Write report');});
it('rejects ABA edits even when the form text returns to empty in the same batch',()=>{const f=setup(),old=capture.value!;let result=true;act(()=>{const field=screen.getByLabelText('Work step goal step-1');fireEvent.change(field,{target:{value:'Changed'}});fireEvent.change(field,{target:{value:''}});result=old.onImport(f.imported.steps,old.formKey,old.formVersion);});expect(result).toBe(false);expect(screen.queryByLabelText('Work step goal step-2')).not.toBeInTheDocument();});
