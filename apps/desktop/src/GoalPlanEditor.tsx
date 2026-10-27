import { useId, useState } from 'react';
import type { Language } from './i18n';
import { GoalFormError, parseGoalPlan, safeInteger, type GoalBudget, type GoalPlan, type GoalPlanNode, type GoalPlanProposal, type GoalScope } from './goal-api';
import { goalMessages } from './goal-i18n';

type Props = { scope: GoalScope; latest: GoalPlan | null; raw: string; language: Language; disabled: boolean; onApply: (raw: string) => void };
type Draft = { proposal: GoalPlanProposal; baseRevision: number; baseDigest: string; baseRaw: string; baseContext: string };
const clone = <T,>(value: T): T => JSON.parse(JSON.stringify(value)) as T;
const numericFields = ['attempts','parallel','runtime_ms','output_bytes','children','reworks'] as const;
const retryClasses = ['before_dispatch_transient','crashed_no_side_effect','validation_failed'];
const same = (a: unknown,b: unknown) => JSON.stringify(a) === JSON.stringify(b);
class EditorError extends Error { constructor(public reason:'editorCycle'|'editorMissingOutput') { super(reason); } }
const relative = (path: unknown): path is string => typeof path==='string' && !!path && !path.startsWith('/') && !/^[A-Za-z]:/.test(path) && !/[\\\u0000-\u001f\u007f]/.test(path) && new TextEncoder().encode(path).length<=4096 && path.split('/').every(part=>!!part&&part!=='.'&&part!=='..');
const beneath = (path: string,prefix: string) => path===prefix||path.startsWith(prefix+'/');
export function hasEditorContext(scope: GoalScope): boolean {
  const context=scope.edit_context;
  const budget=(value:GoalBudget)=>!!value&&numericFields.every(key=>safeInteger(value[key],['children','reworks'].includes(key)?0:1))&&value.parallel<=value.attempts&&value.tokens===null&&value.cost_microunits===null;
  return !!context && safeInteger(context.max_nodes,1,256) && Array.isArray(context.writable_prefixes) && context.writable_prefixes.length>0 && context.writable_prefixes.length<=32 && context.writable_prefixes.every(relative)
    && budget(scope.root_budget) && budget(scope.project_budget)
    && !!scope.inputs && Object.values(scope.inputs).every(input=>!!input&&typeof input.contract_id==='string'&&typeof input.sha256==='string'&&/^[a-f0-9]{64}$/.test(input.sha256))
    && !!scope.contracts && Object.values(scope.contracts).every(rules=>Array.isArray(rules)&&rules.every(rule=>typeof rule==='string'&&Object.hasOwn(scope.rules,rule)))
    && !!context.output_artifacts && typeof context.output_artifacts==='object' && !Array.isArray(context.output_artifacts) && same(Object.keys(context.output_artifacts).sort(),Object.keys(scope.outputs).sort())
    && Object.entries(context.output_artifacts).every(([id,artifact])=>!!artifact&&artifact.artifact_id===id&&relative(artifact.relative_path)&&context.writable_prefixes.some(prefix=>beneath(artifact.relative_path,prefix))&&safeInteger(artifact.max_bytes,1)&& (artifact.expected_sha256===null||typeof artifact.expected_sha256==='string'&&/^[a-f0-9]{64}$/.test(artifact.expected_sha256)))
    && !!context.rule_executors && typeof context.rule_executors==='object' && !Array.isArray(context.rule_executors) && same(Object.keys(context.rule_executors).sort(),Object.keys(scope.rules).sort()) && Object.values(context.rule_executors).every(kind=>kind==='owner'||kind==='validator');
}

/** UI validation is not a mintable scope. Core still resolves paths, rule kinds,
 * real ceilings, shared consumption, complete frame and current authority. */
export function validateEditorPlan(proposal: GoalPlanProposal, scope: GoalScope, latest: GoalPlan | null): GoalPlanProposal {
  const value = parseGoalPlan(JSON.stringify(proposal),scope,latest);
  if (!hasEditorContext(scope) || value.nodes.length>scope.edit_context!.max_nodes) throw new GoalFormError('scope');
  const nodes = new Map(value.nodes.map(node=>[node.id,node]));
  const outputs = new Map<string,{ node: GoalPlanNode; contract: string }>();
  const edges = new Map<string,Set<string>>();
  const fail = (reason: 'scope'|'limit'|'invalid'|'path'|'duplicate' = 'invalid'): never => { throw new GoalFormError(reason); };
  const narrower = (a: GoalBudget,b: GoalBudget) => numericFields.every(key=>a[key]<=b[key]) && a.tokens===null && a.cost_microunits===null;
  for (const node of value.nodes) {
    if (node.recipients.length || node.external_action_id!==null || !narrower(node.budget,scope.root_budget) || node.budget.parallel>node.budget.attempts || node.reservation.runtime_ms>node.budget.runtime_ms || node.reservation.output_bytes>node.budget.output_bytes || node.retry.max_attempts>node.budget.attempts || node.retry.max_reworks>node.budget.reworks) fail('limit');
    if (node.parent_id && (!nodes.has(node.parent_id) || !narrower(node.budget,nodes.get(node.parent_id)!.budget))) fail('scope');
    if (node.side_effect==='advisory_read_only' && node.write_set.length || node.side_effect==='workspace_write' && !node.write_set.length) fail('path');
    if (node.executor.kind==='connection') {
      if (!['work','summary'].includes(node.kind) || !node.outputs.length) fail();
    } else if (node.kind!==(node.executor.kind==='owner'?'owner_approval':'validate') || scope.edit_context!.rule_executors[node.executor.rule_id]!==node.executor.kind || node.side_effect!=='advisory_read_only' || node.outputs.length || !node.required_inputs.length || !same(node.required_validators,[node.executor.rule_id])) fail();
    if (node.write_set.some(write=>write.kind!=='workspace'||!relative(write.path)||!scope.edit_context!.writable_prefixes.some(prefix=>beneath(write.path,prefix)))) fail('path');
    const needs = new Set([...node.dependencies,...(node.parent_id?[node.parent_id]:[])]);
    if ([...needs].some(id=>!nodes.has(id))) fail('scope');
    edges.set(node.id,needs);
    for (const output of node.outputs) {
      if (scope.outputs[output.artifact_id]!==output.contract_id || outputs.has(output.artifact_id)) fail('duplicate');
      if (scope.contracts[output.contract_id].some(rule=>!node.required_validators.includes(rule))) fail('scope');
      outputs.set(output.artifact_id,{node,contract:output.contract_id});
    }
  }
  for (const node of value.nodes) {
    const seen = new Set<string>();
    for (const input of node.required_inputs) {
      if (seen.has(input.artifact_id)) fail('duplicate');
      seen.add(input.artifact_id);
      if (input.source.kind==='approved_external') {
        const approved = scope.inputs[input.artifact_id];
        if (!approved || approved.contract_id!==input.contract_id || approved.sha256!==input.source.sha256) fail('scope');
      } else {
        const source = outputs.get(input.artifact_id);
        if (!source || source.node.id!==input.source.node_id || source.contract!==input.contract_id) fail('scope');
        if (input.source.candidate_for_validation && (node.executor.kind==='connection' || !scope.contracts[input.contract_id].includes(node.executor.rule_id))) fail('scope');
        edges.get(node.id)!.add(input.source.node_id);
      }
    }
  }
  if (Object.entries(scope.outputs).some(([id,contract])=>outputs.get(id)?.contract!==contract)) throw new EditorError('editorMissingOutput');
  const remaining = new Map([...edges].map(([id,deps])=>[id,new Set(deps)]));
  while (remaining.size) {
    const ready = [...remaining].filter(([,deps])=>!deps.size).map(([id])=>id);
    if (!ready.length) throw new EditorError('editorCycle');
    for (const id of ready) { remaining.delete(id); for (const deps of remaining.values()) deps.delete(id); }
  }
  return value;
}

function newNode(scope: GoalScope,id: string,outputs: boolean): GoalPlanNode {
  const budget: GoalBudget = {attempts:1,parallel:1,runtime_ms:Math.min(scope.root_budget.runtime_ms,10000),output_bytes:Math.min(scope.root_budget.output_bytes,1024),children:0,reworks:0,tokens:null,cost_microunits:null};
  const selected = outputs ? Object.entries(scope.outputs).map(([artifact_id,contract_id])=>({artifact_id,contract_id})) : [];
  return { id,parent_id:null,kind:'work',goal:'',dependencies:[],workspace_id:scope.workspace_id,executor:{kind:'connection',binding:clone(scope.binding),program_sha256:scope.program_sha256},recipients:[],
    required_inputs:Object.entries(scope.inputs).map(([artifact_id,input])=>({goal_task_id:scope.task_id,artifact_id,contract_id:input.contract_id,source:{kind:'approved_external',sha256:input.sha256}})),
    outputs:selected,required_validators:[...new Set(selected.flatMap(output=>scope.contracts[output.contract_id]))],side_effect:'workspace_write',write_set:selected.map(output=>({kind:'workspace',path:scope.edit_context!.output_artifacts[output.artifact_id].relative_path})),budget,
    reservation:{runtime_ms:Math.min(budget.runtime_ms,1000),output_bytes:Math.min(budget.output_bytes,1024),tokens:null,cost_microunits:null},retry:{max_attempts:1,max_reworks:0,retry_classes:[]},idempotency_key:crypto.randomUUID(),external_action_id:null };
}

export function GoalPlanEditor({scope,latest,raw,language,disabled,onApply}: Props) {
  const t=goalMessages[language], uid=useId();
  const [draft,setDraft]=useState<Draft|null>(null),[error,setError]=useState(''),[applied,setApplied]=useState(false),[edited,setEdited]=useState(false),[numbers,setNumbers]=useState<Record<string,string>>({});
  const revision=latest?.proposal.revision??0, digest=latest?.digest??'';
  const context=JSON.stringify([scope.scope_digest,scope.edit_context]), available=hasEditorContext(scope);
  const changed=!!draft&&(draft.baseRevision!==revision||draft.baseDigest!==digest||draft.baseRaw!==raw||draft.baseContext!==context);
  const blocked=disabled||changed||!available;
  const invalidNumbers=Object.values(numbers).some(text=>!/^(0|[1-9]\d*)$/.test(text)||!safeInteger(Number(text)));
  function begin(proposal: GoalPlanProposal,edited=false) { setDraft({proposal:clone(proposal),baseRevision:revision,baseDigest:digest,baseRaw:raw,baseContext:context});setNumbers({});setError('');setApplied(false);setEdited(edited); }
  function prepare() {
    if (disabled||!available) return;
    if (latest) begin({...clone(latest.proposal),revision:revision+1,expected_revision:revision},true);
    else begin({schema_version:1,plan_id:crypto.randomUUID(),goal:clone(scope.goal),scope_revision:scope.authority_revision,revision:1,expected_revision:0,acceptance_rules:[...scope.required_acceptance_rules],nodes:[newNode(scope,'step-1',true)]},true);
  }
  function load() {
    if(disabled)return;
    try { begin(parseGoalPlan(raw,scope,latest)); } catch(failure) { setError(failure instanceof GoalFormError?t[failure.reason]:t.invalid); }
  }
  function update(id:string,change:(node:GoalPlanNode)=>GoalPlanNode) {
    if(blocked)return;
    setDraft(previous=>previous?{...previous,proposal:{...previous.proposal,nodes:previous.proposal.nodes.map(node=>node.id===id?change(clone(node)):node)}}:previous);setApplied(false);setEdited(true);setError('');
  }
  function numeric(id:string,group:'budget'|'reservation'|'retry',key:string,text:string) {
    if(blocked)return;
    setNumbers(previous=>({...previous,[`${id}.${group}.${key}`]:text}));
    if(/^(0|[1-9]\d*)$/.test(text)&&safeInteger(Number(text))) update(id,node=>({...node,[group]:{...node[group],[key]:Number(text)}}));
    else {setError(t.integer);setApplied(false);}
  }
  function add() {
    if(blocked||!draft||draft.proposal.nodes.length>=scope.edit_context!.max_nodes)return;
    let index=draft.proposal.nodes.length+1;while(draft.proposal.nodes.some(node=>node.id===`step-${index}`))++index;
    setDraft({...draft,proposal:{...draft.proposal,nodes:[...draft.proposal.nodes,newNode(scope,`step-${index}`,false)]}});setApplied(false);setEdited(true);
  }
  function remove(id:string) {
    if(blocked||!draft)return;
    if(draft.proposal.nodes.some(node=>node.id!==id&&(node.parent_id===id||node.dependencies.includes(id)||node.required_inputs.some(input=>input.source.kind==='node'&&input.source.node_id===id)))){setError(t.editorReferenced);return;}
    setDraft({...draft,proposal:{...draft.proposal,nodes:draft.proposal.nodes.filter(node=>node.id!==id)}});setNumbers(previous=>Object.fromEntries(Object.entries(previous).filter(([key])=>!key.startsWith(id+'.'))));setApplied(false);setEdited(true);
  }
  function apply() {
    if(blocked||!draft||invalidNumbers||!edited)return;
    try {const proposal=validateEditorPlan(draft.proposal,scope,latest),text=JSON.stringify(proposal,null,2);onApply(text);setDraft({...draft,baseRaw:text});setApplied(true);setEdited(false);setError('');}
    catch(failure){setError(failure instanceof GoalFormError||failure instanceof EditorError?t[failure.reason]:t.invalid);}
  }
  const check=(label:string,checked:boolean,onChange:()=>void,off=false)=><label className="goal-plan-editor-check"><input type="checkbox" checked={checked} disabled={blocked||off} onChange={onChange}/>{label}</label>;
  const numericInput=(node:GoalPlanNode,group:'budget'|'reservation'|'retry',key:string,label:string)=><label key={`${group}.${key}`}>{label}<input inputMode="numeric" value={numbers[`${node.id}.${group}.${key}`]??String((node[group] as unknown as Record<string,unknown>)[key])} onChange={event=>numeric(node.id,group,key,event.target.value)}/></label>;
  return <details className="goal-plan-editor" data-testid="goal-plan-editor"><summary>{t.editorTitle}</summary><p>{t.editorHint}</p><div className="goal-plan-editor-actions"><button type="button" disabled={disabled||!available} onClick={prepare}>{latest?t.editorFromLatest:t.editorFromScope}</button><button type="button" disabled={disabled||!raw.trim()} onClick={load}>{t.editorLoad}</button></div>{!available&&<p>{t.editorUnavailable}</p>}{changed&&<p role="alert">{t.editorChanged}</p>}{error&&<p role="alert" className="error">{error}</p>}{applied&&<p role="status">{t.editorApplied}</p>}
    {draft&&<><p>{t.editorVersion}: {draft.proposal.revision} · CAS {draft.proposal.expected_revision}</p><p>{t.editorAcceptance}: {draft.proposal.acceptance_rules.join(', ')}</p>
      <section aria-label={t.editorGraph}><h3>{t.editorGraph}</h3><ol className="goal-plan-editor-graph">{draft.proposal.nodes.map(node=>{const dependencies=[...new Set([...node.dependencies,...(node.parent_id?[node.parent_id]:[]),...node.required_inputs.flatMap(input=>input.source.kind==='node'?[input.source.node_id]:[])])];return <li key={node.id}><strong>{node.id}</strong><span>{node.goal||t.editorUntitled}</span><span>{dependencies.length?`${dependencies.join(', ')} → ${node.id}`:t.editorRoot}</span></li>;})}</ol></section>
      <fieldset disabled={blocked}><legend>{t.editorNodes}</legend>{draft.proposal.nodes.map((node,index)=><details key={node.id} className="goal-plan-editor-node" open={draft.proposal.nodes.length===1?true:undefined}><summary>{index+1}. {node.goal||t.editorUntitled} · {node.id}</summary><label>{t.editorGoal} {node.id}<textarea rows={2} value={node.goal} onChange={event=>update(node.id,next=>({...next,goal:event.target.value}))}/></label><label>{t.editorKind} {node.id}<select value={node.kind} onChange={event=>update(node.id,next=>{const kind=event.target.value as GoalPlanNode['kind'];if(kind==='work'||kind==='summary')return {...next,kind,executor:{kind:'connection',binding:clone(scope.binding),program_sha256:scope.program_sha256},side_effect:'workspace_write'};const rule=Object.keys(scope.rules).find(rule=>scope.edit_context!.rule_executors[rule]===(kind==='validate'?'validator':'owner'))??'';return {...next,kind,executor:{kind:kind==='validate'?'validator':'owner',rule_id:rule},required_validators:[rule],outputs:[],side_effect:'advisory_read_only',write_set:[]};})}>{(['work','summary','validate','owner_approval'] as const).map(kind=><option key={kind} value={kind} disabled={(kind==='validate'||kind==='owner_approval')&&!Object.values(scope.edit_context?.rule_executors??{}).includes(kind==='validate'?'validator':'owner')}>{t[kind]}</option>)}</select></label>
        <p className="hint">{t.editorModeHint}</p>{node.executor.kind==='connection'?<p>{t.editorExecutor}: <code>{scope.binding.connection_id}</code> · <code>{scope.binding.host_id}</code> · <code>{scope.workspace_id}</code></p>:<label>{t.editorRule} {node.id}<select value={node.executor.rule_id} onChange={event=>update(node.id,next=>({...next,executor:{kind:next.kind==='validate'?'validator':'owner',rule_id:event.target.value},required_validators:[event.target.value]}))}>{Object.keys(scope.rules).filter(rule=>scope.edit_context?.rule_executors?.[rule]===node.executor.kind).map(rule=><option key={rule}>{rule}</option>)}</select></label>}
        <details><summary>{t.editorDependencies}</summary><label>{t.editorParent} {node.id}<select value={node.parent_id??''} onChange={event=>update(node.id,next=>({...next,parent_id:event.target.value||null}))}><option value="">{t.editorNone}</option>{draft.proposal.nodes.filter(other=>other.id!==node.id).map(other=><option key={other.id}>{other.id}</option>)}</select></label>{draft.proposal.nodes.filter(other=>other.id!==node.id).map(other=><div key={other.id}>{check(`${t.editorDepends} ${node.id} ← ${other.id}`,node.dependencies.includes(other.id),()=>update(node.id,next=>({...next,dependencies:next.dependencies.includes(other.id)?next.dependencies.filter(id=>id!==other.id):[...next.dependencies,other.id]})))}</div>)}</details>
        <details><summary>{t.editorArtifacts}</summary><fieldset><legend>{t.editorFixedInputs}</legend>{Object.entries(scope.inputs).map(([artifact,input])=><div key={artifact}>{check(`${t.editorInput} ${node.id}: ${artifact}`,node.required_inputs.some(item=>item.artifact_id===artifact),()=>update(node.id,next=>({...next,required_inputs:next.required_inputs.some(item=>item.artifact_id===artifact)?next.required_inputs.filter(item=>item.artifact_id!==artifact):[...next.required_inputs,{goal_task_id:scope.task_id,artifact_id:artifact,contract_id:input.contract_id,source:{kind:'approved_external',sha256:input.sha256}}]})))}<code>{input.sha256}</code></div>)}</fieldset>
          <fieldset><legend>{t.editorNodeInputs}</legend>{draft.proposal.nodes.filter(other=>other.id!==node.id).flatMap(other=>other.outputs.map(output=><div key={`${other.id}:${output.artifact_id}`}>{check(`${t.editorInput} ${node.id}: ${other.id}/${output.artifact_id}`,node.required_inputs.some(input=>input.source.kind==='node'&&input.source.node_id===other.id&&input.artifact_id===output.artifact_id),()=>update(node.id,next=>({...next,required_inputs:next.required_inputs.some(input=>input.source.kind==='node'&&input.source.node_id===other.id&&input.artifact_id===output.artifact_id)?next.required_inputs.filter(input=>!(input.source.kind==='node'&&input.source.node_id===other.id&&input.artifact_id===output.artifact_id)):[...next.required_inputs,{goal_task_id:scope.task_id,artifact_id:output.artifact_id,contract_id:output.contract_id,source:{kind:'node',node_id:other.id,candidate_for_validation:next.executor.kind!=='connection'}}]})))}</div>))}</fieldset>
          {node.executor.kind==='connection'&&<fieldset><legend>{t.editorOutputs}</legend>{Object.entries(scope.outputs).map(([artifact,contract])=>{const owner=draft.proposal.nodes.find(other=>other.id!==node.id&&other.outputs.some(output=>output.artifact_id===artifact));return <div key={artifact}>{check(`${t.editorOutput} ${node.id}: ${artifact}`,node.outputs.some(output=>output.artifact_id===artifact),()=>update(node.id,next=>{const outputs=next.outputs.some(output=>output.artifact_id===artifact)?next.outputs.filter(output=>output.artifact_id!==artifact):[...next.outputs,{artifact_id:artifact,contract_id:contract}];return {...next,outputs,required_validators:[...new Set(outputs.flatMap(output=>scope.contracts[output.contract_id]))]};}),!!owner)}{owner&&<p className="hint">{t.editorProducer}: {owner.id}</p>}</div>;})}</fieldset>}</details>
        {node.executor.kind==='connection'&&<><label>{t.editorEffect} {node.id}<select value={node.side_effect} onChange={event=>update(node.id,next=>({...next,side_effect:event.target.value as GoalPlanNode['side_effect'],write_set:event.target.value==='advisory_read_only'?[]:next.write_set}))}><option value="workspace_write">{t.editorWrite}</option><option value="advisory_read_only">{t.editorRead}</option></select></label><label>{t.editorWritePaths} {node.id}<textarea rows={2} value={node.write_set.flatMap(write=>write.kind==='workspace'?[write.path]:[]).join('\n')} disabled={node.side_effect!=='workspace_write'} onChange={event=>update(node.id,next=>({...next,write_set:event.target.value.split('\n').filter(Boolean).map(path=>({kind:'workspace' as const,path}))}))}/></label><p className="hint">{t.editorPathHint}</p></>}
        <details><summary>{t.editorBudget}</summary><div className="goal-planning-fields">{numericFields.map(key=>numericInput(node,'budget',key,t[key==='children'?'childrenBudget':key]))}{numericInput(node,'reservation','runtime_ms',t.editorReservedRuntime)}{numericInput(node,'reservation','output_bytes',t.editorReservedOutput)}{numericInput(node,'retry','max_attempts',t.editorRetries)}{numericInput(node,'retry','max_reworks',t.editorReworks)}</div><fieldset><legend>{t.editorRetryClasses}</legend>{retryClasses.map(kind=><div key={kind}>{check(kind,node.retry.retry_classes.includes(kind),()=>update(node.id,next=>({...next,retry:{...next.retry,retry_classes:next.retry.retry_classes.includes(kind)?next.retry.retry_classes.filter(item=>item!==kind):[...next.retry.retry_classes,kind]}})))}</div>)}</fieldset></details><button type="button" onClick={()=>remove(node.id)}>{t.editorRemove} {node.id}</button>
      </details>)}<button type="button" disabled={!available||draft.proposal.nodes.length>=(scope.edit_context?.max_nodes??0)} onClick={add}>{t.editorAdd}</button></fieldset><button type="button" className="primary" disabled={blocked||invalidNumbers||!edited} onClick={apply} aria-describedby={`${uid}-apply-hint`}>{t.editorApply}</button><p id={`${uid}-apply-hint`} className="hint">{t.editorApplyHint}</p></>}
  </details>;
}
