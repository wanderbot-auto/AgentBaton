import { Button, Code, Disclosure, DisclosureLabel, Fieldset, Input, Textarea, Title } from './ui';
import {useEffect,useRef,useState} from 'react';
import {taskAllowsExecution,type Task} from './api';
import type {GoalScope,GoalPlan} from './goal-api';
import type {Language} from './i18n';
import {draftRequest,goalPlanDraftApi,type GoalPlanDraft as Draft} from './goal-plan-draft-api';
import {goalPlanDraftMessages} from './goal-plan-draft-i18n';
type Props={task:Task;scope:GoalScope;latest:GoalPlan|null;raw:string;language:Language;disabled:boolean;connected:boolean;onApply:(text:string)=>void};
export function GoalPlanDraft(props:Props){return <DraftPanel key={`${props.task.id}:${props.scope.id}`} {...props}/>;}
function DraftPanel({task,scope,latest,raw,language,disabled,connected,onApply}:Props){
 const t=goalPlanDraftMessages[language];
 const [risk,setRisk]=useState(''),[prohibited,setProhibited]=useState(''),[runtime,setRuntime]=useState(''),[output,setOutput]=useState('');
 const [candidate,setCandidate]=useState<{value:Draft;fingerprint:string}|null>(null),[checks,setChecks]=useState([false,false,false]),[loading,setLoading]=useState(false),[error,setError]=useState<'invalid'|'error'|null>(null);
 const fingerprint=JSON.stringify([task,scope,latest,raw,risk,prohibited,runtime,output]);
 const context=useRef({fingerprint,disabled,connected});context.current={fingerprint,disabled,connected};
 const epoch=useRef(0),mounted=useRef(false),reading=useRef(false);
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;++epoch.current;};},[]);
 useEffect(()=>{++epoch.current;reading.current=false;setLoading(false);setCandidate(null);setChecks([false,false,false]);setError(null);},[fingerprint,disabled,connected]);
 const occupied=latest!==null||raw.trim().length>0;
 const unavailable=disabled||!connected||occupied||scope.revoked||scope.expires_at_ms<=Date.now()||!taskAllowsExecution(task);
 const valid=candidate?.fingerprint===fingerprint&&!unavailable;
 async function generate(){
  if(unavailable||reading.current)return;
  let selection;try{selection=draftRequest(task,scope,risk,prohibited,runtime,output);}catch{setError('invalid');return;}
  const stamp=++epoch.current,base=fingerprint;reading.current=true;setLoading(true);setCandidate(null);setChecks([false,false,false]);setError(null);
  try{const value=await goalPlanDraftApi.generate(selection,task,scope);if(!mounted.current||stamp!==epoch.current||context.current.fingerprint!==base||context.current.disabled||!context.current.connected)return;setCandidate({value,fingerprint:base});}
  catch{if(mounted.current&&stamp===epoch.current)setError('error');}
  finally{if(stamp===epoch.current){reading.current=false;if(mounted.current)setLoading(false);}}
 }
 function apply(){if(!valid||!checks.every(Boolean)||loading||!candidate||candidate.fingerprint!==context.current.fingerprint||context.current.disabled||!context.current.connected||scope.expires_at_ms<=Date.now())return;onApply(JSON.stringify(candidate.value.plan,null,2));}
 return <section aria-label={t.title} aria-busy={loading}><Title order={3}>{t.title}</Title><p>{t.help}</p><p>{t.executionLimit}</p>{occupied?<p>{t.existing}</p>:<><Fieldset disabled={unavailable||loading}><label>{t.risk}<Textarea value={risk} maxLength={4096} onChange={event=>setRisk(event.target.value)}/></label><label>{t.prohibited}<Textarea value={prohibited} maxLength={4096} onChange={event=>setProhibited(event.target.value)}/></label><p>{t.ceiling}: {t.runtime} {Math.min(scope.root_budget.runtime_ms,scope.project_budget.runtime_ms)} · {t.output} {Math.min(scope.root_budget.output_bytes,scope.project_budget.output_bytes)}</p><p>{t.budget_ceiling_not_remaining}</p><label>{t.runtime}<Input inputMode="numeric" value={runtime} onChange={event=>setRuntime(event.target.value)}/></label><label>{t.output}<Input inputMode="numeric" value={output} onChange={event=>setOutput(event.target.value)}/></label></Fieldset><Button type="button" disabled={unavailable||loading} onClick={()=>void generate()}>{t.generate}</Button></>}
 {error&&<p role="alert">{t[error]}</p>}{valid&&candidate&&<div><Title order={4}>{t.goal}</Title><p>{candidate.value.source.task_goal}</p><Title order={4}>{t.scope}</Title><dl><dt>{t.connection}</dt><dd>{scope.binding.connection_id}</dd><dt>{t.workspace}</dt><dd>{scope.workspace_id}</dd><dt>{t.writable}</dt><dd>{candidate.value.writable_prefixes.join(', ')}</dd></dl><Title order={4}>{t.outputs}</Title><ul>{candidate.value.approved_outputs.map(value=><li key={value.artifact.artifact_id}>{value.artifact.relative_path} · {value.artifact.max_bytes}</li>)}</ul><Title order={4}>{t.acceptance}</Title><p>{candidate.value.approved_acceptance_criteria}</p><Title order={4}>{t.risk}</Title><p>{risk}</p><Title order={4}>{t.prohibited}</Title><p>{prohibited}</p><Title order={4}>{t.limits}</Title><p>{t.runtime}: {runtime} · {t.output}: {output}</p><Title order={4}>{t.facts}</Title><ul>{candidate.value.risk_facts.map(code=><li key={code}>{t[code]}</li>)}</ul><Disclosure><DisclosureLabel>{t.plan}</DisclosureLabel><Code block>{JSON.stringify(candidate.value.plan,null,2)}</Code></Disclosure>{(['reviewGoal','reviewRisk','reviewAcceptance'] as const).map((key,index)=><label key={key}><Input type="checkbox" checked={checks[index]} onChange={event=>setChecks(current=>current.map((value,i)=>i===index?event.target.checked:value))}/>{t[key]}</label>)}<Button type="button" disabled={!checks.every(Boolean)||loading} onClick={apply}>{t.apply}</Button></div>}<p>{t.boundary}</p></section>;
}
