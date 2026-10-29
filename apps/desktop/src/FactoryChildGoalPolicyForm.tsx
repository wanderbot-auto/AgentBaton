import type { Dispatch, SetStateAction } from 'react';
import type { ExecutionGrant } from './session-api';
import type { Language } from './i18n';
import type { FactoryForm, FactoryBudgetForm } from './factory-form';
import { childGoalMessages } from './factory-child-goal-i18n';
import { factoryMessages } from './factory-i18n';
import { safeInteger } from './goal-api';
export function FactoryChildGoalPolicyForm({form,grant,language,onChange}:{form:FactoryForm;grant:ExecutionGrant|undefined;language:Language;onChange:Dispatch<SetStateAction<FactoryForm>>}){
 const t=childGoalMessages[language],limits=factoryMessages[language].limits;
 const ceiling=grant&&safeInteger(grant.scope.max_children,0,4294967294)?grant.scope.max_children:null;
 function budget(which:'executionRoot'|'executionProject'){
  const fallback=which==='executionRoot'?'root':'project',values=form[which]??form[fallback];
  return <fieldset><legend>{which==='executionRoot'?t.root:t.project}</legend><div className="factory-fields">{(Object.keys(values) as (keyof FactoryBudgetForm)[]).map(field=><label key={field}>{limits[field]}<input inputMode="numeric" value={values[field]} onChange={e=>onChange(f=>({...f,[which]:{...(f[which]??f[fallback]),[field]:e.target.value}}))}/></label>)}</div></fieldset>;
 }
 return <details><summary>{t.policy}</summary><p>{t.separate}</p><p>{t.grant}: {grant?.id??'—'} · r{grant?.revision??'—'} · children {ceiling??'—'} · max nodes {ceiling===null?'—':Math.min(256,ceiling+1)}</p>{ceiling===0&&<p>{t.planningOnly}</p>}
 <label><input type="checkbox" checked={form.execution??false} disabled={!form.planning||ceiling===null||ceiling===0} onChange={e=>onChange(f=>({...f,execution:e.target.checked}))}/>{t.enable}</label>
 {form.execution&&<><div className="factory-fields"><label>{t.nodes}<input inputMode="numeric" value={form.executionNodes??'2'} onChange={e=>onChange(f=>({...f,executionNodes:e.target.value}))}/></label><label>{t.runsPerTask}<input value="1" readOnly/></label><label>{t.totalRuns}<input inputMode="numeric" value={form.executionTotalRuns??'1'} onChange={e=>onChange(f=>({...f,executionTotalRuns:e.target.value}))}/></label></div>{budget('executionRoot')}{budget('executionProject')}<p>{t.acceptance}</p><p>{t.reworks}</p><p>{t.remaining}</p></>}
 </details>;
}
