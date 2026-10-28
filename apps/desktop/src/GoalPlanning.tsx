import { useCallback, useEffect, useRef, useState, type FormEvent } from 'react';
import { DaemonError, type Task } from './api';
import type { Language } from './i18n';
import type { ExecutionGrant } from './session-api';
import { goalApi, GoalFormError, newGoalForm, parseGoalJson, parseGoalPlan, parseGoalSelection, safeInteger, type BudgetForm, type GoalBinding, type GoalForm, type GoalPlan, type GoalPreview, type GoalScope, type GoalSelection, type GoalUsage } from './goal-api';
import { goalMessages } from './goal-i18n';
import { GoalPlanEditor } from './GoalPlanEditor';
type Props = { task: Task; language: Language; connected: boolean };
type Pending = { kind: 'approve' | 'plan' | 'revoke'; key: string; form: GoalForm; planJson: string; scopeId: string; revision?: number; selection?: GoalSelection; uncertain: boolean; reconciled: boolean };
// Exact UI-process request recovery, not a durable outbox or background writer.
const pendingRequests = new Map<string, Pending>();
const snapshotForm = (form: GoalForm): GoalForm => ({ ...form, root: { ...form.root }, project: { ...form.project }, outputs: form.outputs.map(output => ({ ...output, validators: output.validators.map(ref => ({ ...ref })) })) });
const sameGoal = (a: GoalBinding,b: GoalBinding) => !!a && ['project_id','root_task_id','task_id','spec_revision','input_revision'].every(key => a[key as keyof GoalBinding] === b[key as keyof GoalBinding]);
const uncertain = (error: unknown) => !(error instanceof DaemonError) || error.code === 'OUTCOME_UNKNOWN' || !['none','not_started'].includes(error.sideEffectState);
export function GoalPlanning(props: Props) {
  const [expanded,setExpanded] = useState(false), [started,setStarted] = useState(false);
  // Core create_registered reserves "local" for personal tasks without a registry project.
  const personal = props.task.project_id === 'local';
  return <section className="goal-planning" data-testid="goal-planning" aria-labelledby={`goal-planning-${props.task.id}`}><details onToggle={event=>{const open=event.currentTarget.open;setExpanded(open);if(open)setStarted(true);}}><summary><h2 id={`goal-planning-${props.task.id}`}>{goalMessages[props.language].title}</h2></summary>{started&&(personal?<p>{goalMessages[props.language].personalProject}</p>:<GoalPanel key={props.task.id} {...props} expanded={expanded}/>)}</details></section>;
}
function GoalPanel({ task, language, connected, expanded }: Props & { expanded:boolean }) {
  const t = goalMessages[language], saved = pendingRequests.get(task.id);
  const [form,setForm] = useState<GoalForm>(() => saved?.form ?? newGoalForm()), [planJson,setPlanJson] = useState(saved?.planJson ?? ''), [scopeId,setScopeId] = useState(saved?.scopeId ?? '');
  const [grants,setGrants] = useState<ExecutionGrant[]>([]), [scopes,setScopes] = useState<GoalScope[]>([]), [plans,setPlans] = useState<GoalPlan[]>([]), [preview,setPreview] = useState<GoalPreview | null>(null), [pending,setPending] = useState<Pending | null>(saved ?? null);
  const [busy,setBusy] = useState(false), [loading,setLoading] = useState(false), [fresh,setFresh] = useState(false), [error,setError] = useState('');
  const mounted = useRef(true), sequence = useRef(0);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; ++sequence.current; }; }, []);
  const scopeInScope = useCallback((scope: GoalScope) => !!scope && scope.task_id === task.id && scope.project_id === task.project_id && scope.goal?.task_id === task.id && scope.goal.root_task_id === task.id && scope.goal.project_id === task.project_id && safeInteger(scope.goal.spec_revision,1) && safeInteger(scope.revision,1) && safeInteger(scope.root_grant_revision,1) && safeInteger(scope.authority_revision,1) && safeInteger(scope.expires_at_ms,Number.MIN_SAFE_INTEGER) && scope.planning_only === true && typeof scope.revoked === 'boolean' && scope.binding?.workspace_id === scope.workspace_id && scope.binding.input_revision === scope.input_revision && typeof scope.program_sha256 === 'string' && /^[a-f0-9]{64}$/.test(scope.program_sha256) && !!scope.root_budget && !!scope.project_budget && !!scope.inputs && !!scope.outputs && !!scope.contracts && !!scope.rules && Array.isArray(scope.required_acceptance_rules), [task.id,task.project_id]);
  const planInScope = useCallback((plan: GoalPlan, scope: GoalScope) => !!plan && plan.scope_id === scope.id && plan.dispatched === false && plan.proposal?.schema_version === 1 && safeInteger(plan.proposal.revision,1) && plan.proposal.expected_revision === plan.proposal.revision-1 && sameGoal(plan.proposal.goal,scope.goal) && plan.proposal.scope_revision === scope.authority_revision && Array.isArray(plan.proposal.nodes) && typeof plan.digest === 'string', []);
  const refresh = useCallback(async () => {
    const previous = pendingRequests.get(task.id);
    if (previous) { const blocked = { ...previous, reconciled:false }; pendingRequests.set(task.id,blocked); setPending(blocked); }
    setFresh(false); setPreview(null);
    if (!connected) { setError(t.disconnected); return; }
    if (!task.project_id) { setGrants([]); setScopes([]); setPlans([]); setError(t.noGrant); return; }
    const request = ++sequence.current; setLoading(true);
    const [g,s] = await Promise.allSettled([goalApi.grantsList(task.project_id),goalApi.scopesList(task.id)]);
    if (!mounted.current || request !== sequence.current) return;
    let problem = '';
    if (g.status === 'rejected') problem = g.reason instanceof Error ? g.reason.message : t.error;
    else if (!Array.isArray(g.value) || g.value.some(grant => !grant || grant.project_id !== task.project_id || !safeInteger(grant.revision,1) || typeof grant.revoked !== 'boolean' || !safeInteger(grant.expires_at_ms,Number.MIN_SAFE_INTEGER) || !grant.scope || !Array.isArray(grant.scope.workspace_ids) || !Array.isArray(grant.scope.connection_ids) || !Array.isArray(grant.scope.host_ids))) problem = t.scope;
    if (s.status === 'rejected') problem ||= s.reason instanceof Error ? s.reason.message : t.error;
    else if (!Array.isArray(s.value) || s.value.length > 256 || s.value.some(scope => !scopeInScope(scope)) || new Set(s.value.map(scope => scope.id)).size !== s.value.length) problem = t.scope;
    if (!problem && g.status === 'fulfilled' && s.status === 'fulfilled') {
      const choice = s.value.find(scope => scope.id === scopeId) ?? s.value[0];
      let history: GoalPlan[] = [];
      if (choice) {
        try {
          history = await goalApi.plansList(choice.id);
          if (!Array.isArray(history) || history.length > 256 || history.some(plan => !planInScope(plan,choice)) || new Set(history.map(plan => plan.proposal.revision)).size !== history.length || new Set(history.map(plan => plan.proposal.plan_id)).size > 1) problem = t.scope;
        } catch (failure) { problem = failure instanceof Error ? failure.message : t.error; }
      }
      if (!mounted.current || request !== sequence.current) return;
      if (!problem) {
        setGrants(g.value); setScopes(s.value); setPlans([...history].sort((a,b) => a.proposal.revision-b.proposal.revision)); setFresh(true);
        if ((choice?.id ?? '') !== scopeId) { setScopeId(choice?.id ?? ''); setPlanJson(''); }
        const unresolved = pendingRequests.get(task.id);
        const revoked = unresolved?.kind === 'revoke' && s.value.some(scope => scope.id === unresolved.scopeId && scope.revoked && scope.revision >= unresolved.revision!);
        if (revoked) { pendingRequests.delete(task.id); setPending(null); }
        else if (unresolved) { const next = { ...unresolved, reconciled:true }; pendingRequests.set(task.id,next); setPending(next); }
        else setPending(null);
      }
    }
    setError(problem); setLoading(false);
  }, [connected,task.id,task.project_id,scopeId,scopeInScope,planInScope,t.disconnected,t.error,t.noGrant,t.scope]);
  useEffect(() => { void refresh(); }, [refresh]);
  const scope = scopes.find(item => item.id === scopeId), latest = plans.at(-1) ?? null;
  const roots = grants.filter(grant => !grant.revoked && grant.parent_id === null && grant.root_grant_id === grant.id && grant.expires_at_ms > Date.now() && grant.scope.workspace_ids.length === 1 && grant.scope.connection_ids.length === 1 && grant.scope.host_ids.length === 1 && grant.scope.host_ids[0] === 'local' && [grant.scope.max_attempts,grant.scope.max_concurrency,grant.scope.max_duration_ms,grant.scope.max_children,grant.scope.max_depth].every(value => safeInteger(value)));
  const grant = roots.find(item => item.id === form.grantId), currentGrant = scope && grants.find(item => item.id === scope.root_grant_id);
  const stale = !scope || scope.revoked || scope.goal.spec_revision !== task.spec_revision || !currentGrant || currentGrant.revoked || currentGrant.revision !== scope.root_grant_revision || scope.expires_at_ms <= Date.now() || currentGrant.expires_at_ms <= Date.now() || scope.binding.host_id !== 'local' || (latest !== null && latest.state !== 'validated_proposal');
  const disabled = !connected || busy || loading || !fresh;
  const retry = (kind: Pending['kind']) => !!pending && pending.kind === kind && pending.uncertain && pending.reconciled;
  function clear(op: Pending) { if (pendingRequests.get(task.id)?.key === op.key) pendingRequests.delete(task.id); if (mounted.current) setPending(null); }
  const operation = (kind: Pending['kind']): Pending => ({ kind,key:crypto.randomUUID(),form:snapshotForm(form),planJson,scopeId,uncertain:false,reconciled:false });
  async function perform<T>(op: Pending, action: () => Promise<T>, apply: (value:T) => void) {
    if (!pendingRequests.has(task.id) && pendingRequests.size >= 32) { setError(t.pendingLimit); return; }
    const next = { ...op,uncertain:false,reconciled:false }; pendingRequests.set(task.id,next); setPending(next); setBusy(true); setError(''); setPreview(null);
    try { const value = await action(); apply(value); ++sequence.current; clear(op); }
    catch (failure) {
      if (uncertain(failure)) { const unknown = { ...op,uncertain:true,reconciled:false }; pendingRequests.set(task.id,unknown); if (mounted.current) { setPending(unknown); setError(t.pending); } }
      else { clear(op); if (mounted.current && failure instanceof DaemonError && ['STALE_REVISION','INPUT_CHANGED'].includes(failure.code)) setFresh(false); if (mounted.current) setError(`${failure instanceof DaemonError && failure.code === 'PERMISSION_DENIED' ? t.denied : t.error} ${failure instanceof Error ? failure.message : ''}`); }
    } finally { if (mounted.current) setBusy(false); }
  }
  async function approve(event: FormEvent) {
    event.preventDefault(); if (disabled || (!retry('approve') && (!grant || grant.expires_at_ms <= Date.now() || ['cancelled','cancel_requested','outcome_unknown'].includes(task.state))) || (pending && !retry('approve'))) return;
    let op: Pending;
    try { op = retry('approve') ? pending! : { ...operation('approve'),selection:parseGoalSelection(form,task,grant!) }; }
    catch (failure) { setError(failure instanceof GoalFormError ? t[failure.reason] : t.error); return; }
    await perform(op,() => goalApi.approve(op.selection!,op.key),value => { if (!scopeInScope(value) || value.root_grant_id !== op.selection!.grant_id || value.root_grant_revision !== op.selection!.expected_grant_revision+1) throw new Error(t.scope); if (mounted.current) { setScopes(previous => [value,...previous.filter(item => item.id!==value.id)]); setScopeId(value.id); setFresh(false); } });
  }
  // A reconciled exact-key recovery may read an already-committed historical
  // result after revocation. It cannot change bytes/CAS/key; if no result
  // exists the Core still denies stale authority before a new commit.
  async function submit() {
    if (disabled || !scope || (!retry('plan') && (stale || scope.expires_at_ms <= Date.now())) || (pending && !retry('plan'))) return;
    let op: Pending;
    try { if (retry('plan')) op = pending!; else { const proposal = parseGoalPlan(planJson,scope,latest); op = { ...operation('plan'),revision:proposal.expected_revision }; } }
    catch (failure) { setError(failure instanceof GoalFormError ? t[failure.reason] : t.error); return; }
    await perform(op,() => op.revision === 0 ? goalApi.propose(op.scopeId,op.planJson,op.revision,op.key) : goalApi.revise(op.scopeId,op.planJson,op.revision!,op.key),value => {
      if (!planInScope(value,scope) || value.proposal.expected_revision !== op.revision || value.state !== 'validated_proposal') throw new Error(t.scope);
      if (mounted.current) setPlans(previous => [...previous.filter(item => item.proposal.revision !== value.proposal.revision),value].sort((a,b) => a.proposal.revision-b.proposal.revision));
    });
  }
  async function revoke() {
    if (disabled || !scope || scope.revoked || (pending && !retry('revoke'))) return;
    const op = retry('revoke') ? pending! : { ...operation('revoke'),revision:scope.revision };
    await perform(op,() => goalApi.revoke(op.scopeId,op.revision!,op.key),value => { if (!scopeInScope(value) || value.id!==op.scopeId || !value.revoked) throw new Error(t.scope); if (mounted.current) setScopes(previous => previous.map(item => item.id===value.id ? value : item)); });
  }
  async function readPreview() {
    if (disabled || stale || !scope || scope.expires_at_ms <= Date.now() || !latest || pending) return;
    const request = ++sequence.current; setLoading(true); setError(''); setPreview(null);
    try {
      const value = await goalApi.preview(scope.id,latest.proposal.revision);
      if (!mounted.current || request !== sequence.current) return;
      const usage = (u: GoalUsage) => !!u && ['attempts','active','children'].every(key => safeInteger(u[key as keyof GoalUsage])) && ['runtime_ms','output_bytes','tokens','cost_microunits'].every(key => u[key as keyof GoalUsage] === null || safeInteger(u[key as keyof GoalUsage]));
      if (value.scope_id!==scope.id || value.plan_id!==latest.proposal.plan_id || value.plan_revision!==latest.proposal.revision || value.read_only!==true || value.reservation_made!==false || value.dispatched!==false || !safeInteger(value.journal_sequence) || !Array.isArray(value.unknown_dimensions) || value.unknown_dimensions.some(item=>typeof item!=='string') || !Array.isArray(value.blocked_reasons) || value.blocked_reasons.some(item=>typeof item!=='string') || !usage(value.budget_usage?.root) || !usage(value.budget_usage?.project) || (value.decision!==null && (value.decision.plan_digest!==latest.digest || value.decision.expected_plan_revision!==latest.proposal.revision || value.decision.requires_atomic_store_commit!==true || !safeInteger(value.decision.expected_ledger_revision) || typeof value.decision.required_checks_satisfied!=='boolean' || !Array.isArray(value.decision.admissions) || value.decision.admissions.some(item=>!item || !latest.proposal.nodes.some(node=>node.id===item.node_id)) || !value.decision.blocked || typeof value.decision.blocked!=='object' || Array.isArray(value.decision.blocked) || Object.entries(value.decision.blocked).some(([id,reason])=>typeof reason!=='string' || !latest.proposal.nodes.some(node=>node.id===id))))) throw new Error(t.scope);
      setPreview(value);
    } catch (failure) { if (mounted.current && request===sequence.current) { setError(failure instanceof Error ? failure.message : t.error); setFresh(false); } }
    finally { if (mounted.current && request===sequence.current) setLoading(false); }
  }
  function chooseGrant(id:string) { const g=roots.find(item=>item.id===id); setForm(previous=>g ? { ...previous,grantId:id,maxChildren:String(Math.max(g.scope.max_children,3)),maxDepth:String(Math.max(g.scope.max_depth,1)),root:{ ...previous.root,attempts:String(g.scope.max_attempts),parallel:String(g.scope.max_concurrency),runtime_ms:String(g.scope.max_duration_ms) },project:{ ...previous.project,attempts:String(g.scope.max_attempts),parallel:String(g.scope.max_concurrency),runtime_ms:String(g.scope.max_duration_ms) } } : { ...previous,grantId:id }); }
  const field = (key:'maxNodes'|'maxChildren'|'maxDepth'|'cap',label:string) => <label>{label}<input value={form[key]} onChange={event=>setForm(previous=>({...previous,[key]:event.target.value}))} /></label>;
  function budgetFields(key:'root'|'project',label:string) { return <fieldset><legend>{label}</legend><div className="goal-planning-fields">{(Object.keys(form[key]) as (keyof BudgetForm)[]).map(name=><label key={name}>{name==='children'?t.childrenBudget:t[name]}<input value={form[key][name]} onChange={event=>setForm(previous=>({...previous,[key]:{...previous[key],[name]:event.target.value}}))} /></label>)}</div></fieldset>; }
  function insertHeader(next:boolean) {
    if (!scope || disabled || pending || stale) return;
    try { const value=next ? parseGoalJson(planJson) as Record<string,unknown> : {schema_version:1,plan_id:crypto.randomUUID(),goal:scope.goal,scope_revision:scope.authority_revision,revision:1,expected_revision:0,acceptance_rules:scope.required_acceptance_rules,nodes:[]}; if(next) { value.expected_revision=latest?.proposal.revision ?? 0;value.revision=(latest?.proposal.revision ?? 0)+1;value.plan_id=latest?.proposal.plan_id ?? value.plan_id; } setPlanJson(JSON.stringify(value,null,2)); setPreview(null); }
    catch (failure) { setError(failure instanceof GoalFormError ? t[failure.reason] : t.error); }
  }
  const count = (value:number|null) => value===null ? t.notMeasured : safeInteger(value) ? String(value) : t.integer;
  return <div aria-busy={busy||loading}><p>{t.warning}</p><p className="hint">{t.boundaries}</p><button type="button" onClick={()=>void refresh()} disabled={!connected||busy||loading}>{t.refresh}</button>{error&&<p role={expanded?'alert':undefined} className="error">{error}</p>}{pending&&<p role={expanded?'status':undefined}>{pending.uncertain?t.pending:t.waiting}</p>}
    <form onSubmit={event=>void approve(event)}><fieldset disabled={disabled||!!pending}><legend>{t.approve}</legend><label>{t.grant}<select value={form.grantId} onChange={event=>chooseGrant(event.target.value)}><option value="">{t.chooseGrant}</option>{form.grantId&&!roots.some(g=>g.id===form.grantId)&&<option value={form.grantId} disabled>{form.grantId}</option>}{roots.map(g=><option value={g.id} key={g.id}>{g.id} · {t.before} {g.revision}</option>)}</select></label>{!roots.length&&<p>{t.noGrant}</p>}{grant&&<p>{t.before}: {grant.revision}</p>}<div className="goal-planning-fields">{field('maxNodes',t.maxNodes)}{field('maxChildren',t.children)}{field('maxDepth',t.depth)}{field('cap',t.cap)}</div>
      <details><summary>{t.rootBudget} / {t.projectBudget}</summary>{budgetFields('root',t.rootBudget)}{budgetFields('project',t.projectBudget)}<p className="hint">{t.meters}</p></details>
      <label>{t.inputs}<textarea rows={2} maxLength={8192} value={form.inputs} onChange={event=>setForm(previous=>({...previous,inputs:event.target.value}))} /></label>
      <fieldset><legend>{t.outputs}</legend>{form.outputs.map((output,index)=><div className="goal-planning-output" key={index}>{(['id','path','hash'] as const).map(key=><label key={key}>{key==='id'?t.outputId:key==='path'?t.outputPath:t.outputHash} {index+1}<input value={output[key]} maxLength={key==='hash'?64:key==='path'?4096:256} onChange={event=>setForm(previous=>({...previous,outputs:previous.outputs.map((item,n)=>n===index?{...item,[key]:event.target.value}:item)}))} /></label>)}<details><summary>{t.validators} {index+1}</summary>{output.validators.map((ref,n)=><div className="goal-planning-fields" key={n}><label>{t.validatorKind} {index+1}.{n+1}<select value={ref.kind} onChange={event=>setForm(previous=>({...previous,outputs:previous.outputs.map((item,i)=>i===index?{...item,validators:item.validators.map((r,j)=>j===n?{...r,kind:event.target.value as typeof ref.kind}:r)}:item)}))}>{['integrity','data','command'].map(kind=><option key={kind}>{kind}</option>)}</select></label>{(['policyId','revision'] as const).map(key=><label key={key}>{key==='policyId'?t.policyId:t.policyRevision} {index+1}.{n+1}<input value={ref[key]} onChange={event=>setForm(previous=>({...previous,outputs:previous.outputs.map((item,i)=>i===index?{...item,validators:item.validators.map((r,j)=>j===n?{...r,[key]:event.target.value}:r)}:item)}))} /></label>)}<button type="button" onClick={()=>setForm(previous=>({...previous,outputs:previous.outputs.map((item,i)=>i===index?{...item,validators:item.validators.filter((_,j)=>j!==n)}:item)}))}>{t.removeValidator} {index+1}.{n+1}</button></div>)}<button type="button" disabled={output.validators.length>=8} onClick={()=>setForm(previous=>({...previous,outputs:previous.outputs.map((item,i)=>i===index?{...item,validators:[...item.validators,{kind:'integrity',policyId:'',revision:'1'}]}:item)}))}>{t.addValidator} {index+1}</button></details><button type="button" disabled={form.outputs.length===1} onClick={()=>setForm(previous=>({...previous,outputs:previous.outputs.filter((_,i)=>i!==index)}))}>{t.removeOutput} {index+1}</button></div>)}<button type="button" disabled={form.outputs.length>=16} onClick={()=>setForm(previous=>({...previous,outputs:[...previous.outputs,{id:`output-${previous.outputs.length+1}`,path:'',hash:'',validators:[]}]}))}>{t.addOutput}</button><p className="hint">{t.validatorHint}</p></fieldset>
      <label>{t.writable}<textarea rows={2} value={form.writable} maxLength={8192} onChange={event=>setForm(previous=>({...previous,writable:event.target.value}))} /></label><label>{t.criteria}<textarea rows={3} value={form.criteria} maxLength={2048} onChange={event=>setForm(previous=>({...previous,criteria:event.target.value}))} /></label>
    </fieldset><button type="submit" className="primary" disabled={disabled||(!retry('approve')&&(!grant||['cancelled','cancel_requested','outcome_unknown'].includes(task.state)))||(!!pending&&!retry('approve'))}>{retry('approve')?t.recoverApproval:t.approve}</button></form>
    {!scopes.length&&<p>{t.noScopes}</p>}{!!scopes.length&&<label>{t.selectScope}<select value={scopeId} disabled={busy||loading||!!pending} onChange={event=>{setScopeId(event.target.value);setPlanJson('');setPreview(null);setFresh(false);}}>{scopes.map(s=><option key={s.id} value={s.id}>{s.id} · {s.revoked?t.revoked:t.approved}</option>)}</select></label>}
    {scope&&<><p>{scope.revoked?t.revoked:t.approved}</p><dl><dt>{t.originalGrant}</dt><dd>{scope.root_grant_revision}</dd><dt>{t.authorityRevision}</dt><dd>{scope.authority_revision}</dd><dt>{t.digest}</dt><dd><code>{scope.scope_digest}</code></dd></dl>{stale&&<p>{t.stale}</p>}<button type="button" onClick={()=>void revoke()} disabled={disabled||scope.revoked||(!!pending&&!retry('revoke'))}>{retry('revoke')?t.recoverRevoke:t.revoke}</button><p className="hint">{t.revokeHint}</p>
      <details><summary>{t.planTitle}</summary><p className="hint">{t.planHint}</p><GoalPlanEditor key={scope.id} scope={scope} latest={latest} raw={planJson} language={language} disabled={disabled||stale||!!pending} onApply={text=>{setPlanJson(text);setPreview(null);}}/><details><summary>{t.context}</summary><p>{t.contextHint}</p><button type="button" onClick={()=>insertHeader(false)} disabled={disabled||stale||!!pending}>{t.context}</button><pre>{JSON.stringify({goal:scope.goal,binding:scope.binding,program_sha256:scope.program_sha256,inputs:scope.inputs,outputs:scope.outputs,contracts:scope.contracts,rules:scope.rules},null,2)}</pre></details><label>{t.planJson}<textarea rows={12} value={planJson} onChange={event=>{setPlanJson(event.target.value);setPreview(null);}} disabled={disabled||stale||!!pending} spellCheck={false} /></label>{latest&&<button type="button" onClick={()=>insertHeader(true)} disabled={disabled||stale||!!pending}>{t.next}</button>}<button type="button" onClick={()=>void submit()} disabled={disabled||(!retry('plan')&&stale)||!planJson.trim()||(!!pending&&!retry('plan'))}>{retry('plan')?t.recoverPlan:latest?t.revise:t.propose}</button></details>
      {!plans.length&&<p>{t.noPlan}</p>}{!!plans.length&&<details><summary>{t.history}</summary>{plans.map(plan=><details key={plan.proposal.revision}><summary>{plan.state==='validated_proposal'?t.proposalOnly:`${t.unknownState}: ${plan.state}`} · {plan.proposal.revision} · <time dateTime={plan.created_at}>{plan.created_at}</time></summary><p>{t.digest}: <code>{plan.digest}</code></p><pre>{JSON.stringify(plan.proposal,null,2)}</pre></details>)}</details>}
      <button type="button" onClick={()=>void readPreview()} disabled={disabled||stale||!latest||!!pending}>{t.preview}</button>{preview&&<section aria-label={t.previewTitle}><h3>{t.previewTitle}</h3><p>{t.previewHint}</p><p>{t.sequence}: {count(preview.journal_sequence)}</p><div className="goal-planning-fields">{(['root','project'] as const).map(key=><dl key={key}><dt>{t.usage} · {t[key]}</dt>{(Object.keys(preview.budget_usage[key]) as (keyof GoalUsage)[]).map(name=><div key={name}><dt>{name==='children'?t.childrenBudget:t[name]}</dt><dd>{count(preview.budget_usage[key][name])}</dd></div>)}</dl>)}</div><p>{t.unknown}: {preview.unknown_dimensions.join(', ')||'—'}</p><p>{t.blocked}: {preview.blocked_reasons.join(', ')||'—'}</p>{preview.decision&&<><p>{t.candidates}: {preview.decision.admissions.map(item=>item.node_id).join(', ')||'—'}</p>{Object.entries(preview.decision.blocked).map(([id,reason])=><p key={id}>{id}: {reason}</p>)}</>}</section>}
    </>}
  </div>;
}
