import { useCallback, useEffect, useRef, useState, type FormEvent } from 'react';
import { DaemonError, type Task } from './api';
import type { Language } from './i18n';
import type { ManagedSession } from './session-api';
import { dataValidationApi, dataValidator, DataFormError, newColumn, newDataForm, parseDataSelection, type DataForm, type DataPolicy, type DataRecord, type DataSelection } from './data-validation-api';
import { dataValidationMessages } from './data-validation-i18n';

type Props = { task: Task; sessions: ManagedSession[]; language: Language; connected: boolean };
type Pending = { kind: 'approve' | 'run' | 'revoke'; key: string; uncertain: boolean; refreshed: boolean; form: DataForm; selection?: DataSelection; policyId?: string; revision?: number };
// UI-process reconciliation cache, never an automatic or durable outbox.
const pendingRequests = new Map<string, Pending>();
const safeRevision = (value: number) => Number.isSafeInteger(value) && value >= 1;
const uncertain = (error: unknown) => !(error instanceof DaemonError) || error.code === 'OUTCOME_UNKNOWN' || !['none', 'not_started'].includes(error.sideEffectState);
const snapshotForm = (form: DataForm) => ({ ...form, columns: form.columns.map(column => ({ ...column })) });
export function DataContractValidation(props: Props) {
  return <section className="data-validation" data-testid="data-contract-validation" aria-labelledby={`data-heading-${props.task.id}`}><details><summary><h2 id={`data-heading-${props.task.id}`}>{dataValidationMessages[props.language].title}</h2></summary><DataPanel key={props.task.id} {...props} /></details></section>;
}
function DataPanel({ task, sessions, language, connected }: Props) {
  const t = dataValidationMessages[language], saved = pendingRequests.get(task.id);
  const eligible = sessions.filter(session => session.task_id === task.id && !!task.project_id && session.project_id === task.project_id && ['stopped', 'failed'].includes(session.state));
  const [form, setForm] = useState<DataForm>(() => saved?.form ?? newDataForm(eligible[0]?.id));
  const [policies, setPolicies] = useState<DataPolicy[]>([]), [records, setRecords] = useState<DataRecord[]>([]), [pending, setPending] = useState<Pending | null>(saved ?? null);
  const [busy, setBusy] = useState(false), [loading, setLoading] = useState(false), [fresh, setFresh] = useState(false), [error, setError] = useState('');
  const sequence = useRef(0), mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; ++sequence.current; }; }, []);
  useEffect(() => { if (!form.sessionId && eligible.length && !pending) setForm(previous => ({ ...previous, sessionId: eligible[0].id })); }, [eligible, form.sessionId, pending]);
  const policyInScope = useCallback((p: DataPolicy) => !!p && p.task_id === task.id && p.project_id === task.project_id && p.scope === 'data_contract_only' && p.validator_id === dataValidator && p.file_contract?.subject?.task_id === task.id && p.file_contract.subject.attempt_id === p.attempt_id && p.file_contract.subject.version?.kind === 'data' && safeRevision(p.file_contract.subject.spec_revision) && Array.isArray(p.file_contract.inputs) && Array.isArray(p.file_contract.outputs) && p.table?.table_artifact_id === 'table' && !!p.table.analysis && !!p.table.analysis.scope && !!p.table.analysis.units && Array.isArray(p.table.columns) && safeRevision(p.revision) && typeof p.revoked === 'boolean', [task.id, task.project_id]);
  const recordInScope = useCallback((r: DataRecord) => {
    const e = r?.evidence, subject = e?.contract?.artifact_policy?.subject;
    return !!r && r.task_id === task.id && r.scope === 'data_contract_only' && r.correctness === 'approved_schema_ranges_metadata_only' && r.task_acceptance === 'not_established' && r.integration_authorized === false && e?.validator_id === dataValidator && e.contract?.validator_id === dataValidator && subject?.task_id === task.id && subject.version?.kind === 'data' && safeRevision(subject.spec_revision) && e.file_integrity?.subject?.task_id === task.id && e.file_integrity.subject.attempt_id === subject.attempt_id && e.file_integrity.subject.spec_revision === subject.spec_revision && e.file_integrity.subject.input_revision === subject.input_revision && e.file_integrity.subject.version?.kind === 'data' && Array.isArray(e.file_integrity.inputs) && Array.isArray(e.file_integrity.outputs) && (e.checked_rows === null || (Number.isSafeInteger(e.checked_rows) && e.checked_rows >= 0)) && (r.state !== 'contract_passed' || (e.passed === true && e.file_integrity.passed === true)) && (r.state !== 'contract_failed' || e.passed === false);
  }, [task.id]);
  const refresh = useCallback(async () => {
    const previous = pendingRequests.get(task.id);
    if (previous) { const blocked = { ...previous, refreshed: false }; pendingRequests.set(task.id, blocked); setPending(blocked); }
    setFresh(false);
    if (!connected) { setError(t.disconnected); return; }
    const request = ++sequence.current; setLoading(true);
    const [p, r] = await Promise.allSettled([dataValidationApi.policiesList(task.id), dataValidationApi.recordsList(task.id)]);
    if (!mounted.current || request !== sequence.current) return;
    let problem = '';
    if (p.status === 'rejected') problem = p.reason instanceof Error ? p.reason.message : t.localError;
    else if (!Array.isArray(p.value) || p.value.some(policy => !policyInScope(policy))) problem = t.scopeMismatch;
    if (r.status === 'rejected') problem ||= r.reason instanceof Error ? r.reason.message : t.localError;
    else if (!Array.isArray(r.value) || r.value.some(record => !recordInScope(record))) problem = t.scopeMismatch;
    if (!problem && p.status === 'fulfilled' && r.status === 'fulfilled') {
      setPolicies(p.value); setRecords(r.value); setFresh(true);
      const unresolved = pendingRequests.get(task.id);
      if (unresolved?.kind === 'revoke' && p.value.some(policy => policy.id === unresolved.policyId && policy.revoked)) { pendingRequests.delete(task.id); setPending(null); }
      else if (unresolved) { const next = { ...unresolved, refreshed: true }; pendingRequests.set(task.id, next); setPending(next); }
      else setPending(null);
    }
    setError(problem); setLoading(false);
  }, [connected, policyInScope, recordInScope, task.id, t.disconnected, t.localError, t.scopeMismatch]);
  useEffect(() => { void refresh(); }, [refresh]);
  const canRetry = (kind: Pending['kind'], policyId?: string) => !!pending && pending.kind === kind && pending.uncertain && pending.refreshed && (!policyId || pending.policyId === policyId);
  function clear(operation: Pending) { if (pendingRequests.get(task.id)?.key === operation.key) pendingRequests.delete(task.id); if (mounted.current) setPending(null); }
  async function perform<T>(operation: Pending, action: () => Promise<T>, apply: (value: T) => void) {
    if (!pendingRequests.has(task.id) && pendingRequests.size >= 32) { setError(t.pendingLimit); return; }
    const next = { ...operation, uncertain: false, refreshed: false }; pendingRequests.set(task.id, next); setPending(next); setBusy(true); setError('');
    try { const value = await action(); apply(value); ++sequence.current; if (mounted.current) setLoading(false); clear(operation); }
    catch (failure) {
      if (uncertain(failure)) { const unknown = { ...operation, uncertain: true, refreshed: false }; pendingRequests.set(task.id, unknown); if (mounted.current) { setPending(unknown); setError(t.unknown); } }
      else { clear(operation); if (mounted.current) setError(`${failure instanceof DaemonError && failure.code === 'PERMISSION_DENIED' ? t.denied : t.localError} ${failure instanceof Error ? failure.message : ''}`); }
    } finally { if (mounted.current) setBusy(false); }
  }
  const operation = (kind: Pending['kind']): Pending => ({ kind, key: crypto.randomUUID(), uncertain: false, refreshed: false, form: snapshotForm(form) });
  const disabled = !connected || busy || loading || !fresh;
  async function approve(event: FormEvent) {
    event.preventDefault(); if (disabled || (pending && !canRetry('approve'))) return;
    let next: Pending;
    try { if (canRetry('approve')) next = pending!; else { if (!eligible.some(session => session.id === form.sessionId)) throw new DataFormError('empty'); next = { ...operation('approve'), selection: parseDataSelection(form) }; } }
    catch (failure) { setError(failure instanceof DataFormError ? failure.reason === 'sample' ? t.sampleError : t[failure.reason] : t.localError); return; }
    await perform(next, () => dataValidationApi.approve(next.selection!, next.key), policy => {
      if (!policyInScope(policy) || policy.session_id !== next.selection!.session_id) throw new Error(t.scopeMismatch);
      if (mounted.current) setPolicies(previous => [policy, ...previous.filter(item => item.id !== policy.id)]);
    });
  }
  async function run(policy: DataPolicy) {
    if (disabled || policy.revoked || !eligible.some(session => session.id === policy.session_id) || !safeRevision(policy.revision) || policy.file_contract.subject.spec_revision !== task.spec_revision || (pending && !canRetry('run', policy.id))) return;
    const next = canRetry('run', policy.id) ? pending! : { ...operation('run'), policyId: policy.id, revision: policy.revision };
    await perform(next, () => dataValidationApi.run(policy.id, next.revision!, next.key), record => {
      if (!recordInScope(record) || record.policy_id !== policy.id) throw new Error(t.scopeMismatch);
      if (mounted.current) setRecords(previous => [record, ...previous.filter(item => item.id !== record.id)]);
    });
  }
  async function revoke(policy: DataPolicy) {
    if (disabled || policy.revoked || !safeRevision(policy.revision) || (pending && !canRetry('revoke', policy.id))) return;
    const next = canRetry('revoke', policy.id) ? pending! : { ...operation('revoke'), policyId: policy.id, revision: policy.revision };
    await perform(next, () => dataValidationApi.revoke(policy.id, next.revision!), value => {
      if (!policyInScope(value) || value.id !== policy.id || !value.revoked) throw new Error(t.scopeMismatch);
      if (mounted.current) setPolicies(previous => previous.map(item => item.id === policy.id ? value : item));
    });
  }
  const update = (field: keyof DataForm, value: string) => setForm(previous => ({ ...previous, [field]: value }));
  function field(field: keyof DataForm, label: string, type = 'text', maxLength = 4096) {
    const id = `data-${task.id}-${field}`;
    return <label htmlFor={id}>{label}<input id={id} type={type} maxLength={maxLength} value={String(form[field])} onChange={event => update(field, event.target.value)} /></label>;
  }
  return <div aria-busy={busy || loading}>
    <p>{t.warning}</p><p className="hint">{t.localOnly}</p>
    <button type="button" onClick={() => void refresh()} disabled={!connected || busy || loading}>{t.refresh}</button>
    {error && <p role="alert" className="error">{error}</p>}{pending && <p role="status">{pending.uncertain ? `${t.unknown} ${pending.refreshed ? '' : t.refreshFirst}` : t.waiting}</p>}
    <form onSubmit={event => void approve(event)}><fieldset disabled={disabled || !!pending}><legend>{t.form}</legend>
      <label>{t.session}<select value={form.sessionId} onChange={event => update('sessionId', event.target.value)}><option value="">{t.chooseSession}</option>{form.sessionId && !eligible.some(session => session.id === form.sessionId) && <option value={form.sessionId} disabled>{t.sessionUnavailable}</option>}{eligible.map(session => <option key={session.id} value={session.id}>{session.id} · {session.state === 'failed' ? t.failedSession : t.stoppedSession}</option>)}</select></label><p className="hint">{t.sessionHint}</p>{!eligible.length && <p>{t.noSession}</p>}
      <label>{t.inputs}<textarea value={form.inputs} maxLength={8192} rows={2} onChange={event => update('inputs', event.target.value)} /></label><p className="hint">{t.inputsHint}</p>
      <div className="data-validation-fields">{field('tablePath', t.tablePath, 'text', 240)}{field('scriptPath', t.scriptPath, 'text', 240)}{field('environmentPath', t.environmentPath, 'text', 240)}{field('datasetRows', t.datasetRows)}
        <label>{t.scope}<select value={form.scope} onChange={event => update('scope', event.target.value)}><option value="full">{t.full}</option><option value="sample">{t.sample}</option></select></label>{form.scope === 'sample' && <>{field('sampleRows', t.sampleRows)}{field('limitation', t.limitation)}</>}{field('timezone', t.timezone, 'text', 256)}{field('seed', t.seed)}</div>
      <fieldset className="data-validation-columns"><legend>{t.columns}</legend>{form.columns.map((column, index) => {
        const set = (patch: Partial<typeof column>) => setForm(previous => ({ ...previous, columns: previous.columns.map((value, n) => n === index ? { ...value, ...patch } : value) }));
        const text = (key: 'name' | 'min' | 'max' | 'unit' | 'maxBytes', label: string) => <label>{label} {index + 1}<input value={column[key]} maxLength={key === 'name' ? 64 : key === 'unit' ? 128 : 64} onChange={event => set({ [key]: event.target.value })} /></label>;
        return <div className="data-validation-column" key={index}>{text('name', t.name)}<label>{t.type} {index + 1}<select value={column.type} onChange={event => set({ type: event.target.value as typeof column.type })}><option value="number">{t.numberType}</option><option value="integer">{t.integerType}</option><option value="string">{t.stringType}</option><option value="boolean">{t.booleanType}</option></select></label>
          {(column.type === 'number' || column.type === 'integer') && <>{text('min', t.min)}{text('max', t.max)}{text('unit', t.unit)}</>}{column.type === 'string' && text('maxBytes', t.stringBytes)}
          <label className="data-validation-checkbox"><input type="checkbox" checked={column.required} onChange={event => set({ required: event.target.checked })} />{t.required} {index + 1}</label><label className="data-validation-checkbox"><input type="checkbox" checked={column.nullable} onChange={event => set({ nullable: event.target.checked })} />{t.nullable} {index + 1}</label><button type="button" disabled={form.columns.length === 1} onClick={() => setForm(previous => ({ ...previous, columns: previous.columns.filter((_, n) => n !== index) }))}>{t.removeColumn} {index + 1}</button></div>;
      })}<button type="button" disabled={form.columns.length >= 64} onClick={() => setForm(previous => ({ ...previous, columns: [...previous.columns, newColumn()] }))}>{t.addColumn}</button></fieldset>
      <details><summary>{t.limits}</summary><div className="data-validation-fields">{field('maxBytes', t.maxBytes)}{field('maxDepth', t.maxDepth)}{field('maxValues', t.maxValues)}{field('maxStringBytes', t.maxStringBytes)}{field('maxRows', t.maxRows)}</div></details><p className="hint">{t.bounds}</p>
    </fieldset><button type="submit" disabled={disabled || !eligible.some(session => session.id === form.sessionId) || (!!pending && !canRetry('approve'))}>{canRetry('approve') ? t.approveAgain : t.approve}</button></form>
    {!!policies.length && <section aria-label={t.policies}><h3>{t.policies}</h3>{policies.map(policy => {
      const stale = !safeRevision(task.spec_revision ?? 0) || policy.file_contract.subject.spec_revision !== task.spec_revision;
      const stoppedSession = eligible.some(session => session.id === policy.session_id);
      return <details key={policy.id}><summary>{policy.revoked ? t.revoked : t.approved} · <time dateTime={policy.created_at}>{policy.created_at}</time></summary><dl><dt>{t.policy}</dt><dd>{policy.id}</dd><dt>{t.session}</dt><dd>{policy.session_id}</dd><dt>{t.digestLabel}</dt><dd><code>{policy.contract_digest}</code></dd></dl><details><summary>{t.policyDetails}</summary><p>{t.datasetRows}: {Number.isSafeInteger(policy.table.dataset_rows) ? policy.table.dataset_rows : t.integer}</p><p>{t.scope}: {policy.table.analysis.scope.kind === 'sample' ? `${t.sample} · ${policy.table.analysis.scope.rows} · ${policy.table.analysis.scope.limitation}` : policy.table.analysis.scope.kind === 'full' ? t.full : t.unknownState}</p><p>{t.timezone}: {policy.table.analysis.timezone}</p>{policy.file_contract.outputs.map(item => <p key={item.requirement.artifact_id}><code>{item.requirement.artifact_id}: {item.requirement.relative_path}</code> · {item.requirement.max_bytes} {t.bytes}</p>)}{policy.table.columns.map(column => <p key={column.name}><code>{column.name}</code> · {column.scalar.type} · {policy.table.analysis.units[column.name] ?? ''}</p>)}</details>{stale && <p>{t.stalePolicy}</p>}{!stoppedSession && <p>{t.noSession}</p>}<div className="data-validation-actions"><button type="button" onClick={() => void run(policy)} disabled={disabled || stale || !stoppedSession || policy.revoked || (!!pending && !canRetry('run', policy.id))}>{canRetry('run', policy.id) ? t.retryRun : t.run}</button><button type="button" onClick={() => void revoke(policy)} disabled={disabled || policy.revoked || (!!pending && !canRetry('revoke', policy.id))}>{t.revoke}</button></div></details>;
    })}</section>}
    {!loading && !policies.length && <p className="hint">{t.noPolicies}</p>}{!records.length && <p className="hint">{t.noRecords}</p>}
    {!!records.length && <p className="hint">{t.historical}</p>}{records.map(record => <details key={record.id}><summary>{record.state === 'contract_passed' ? t.checkPassed : record.state === 'contract_failed' ? t.failed : `${t.unknownState}: ${record.state}`} · <time dateTime={record.created_at}>{record.created_at}</time></summary><p>{t.correctness}</p><p>{t.acceptance}</p><dl><dt>{t.validator}</dt><dd><code>{record.evidence.validator_id}</code></dd><dt>{t.digestLabel}</dt><dd><code>{record.evidence.digest}</code></dd><dt>{t.tableHash}</dt><dd><code>{record.evidence.table_sha256 ?? t.notObserved}</code></dd><dt>{t.checkedRows}</dt><dd>{record.evidence.checked_rows ?? t.notObserved}</dd><dt>{t.attempt}</dt><dd>{record.evidence.contract.artifact_policy.subject.attempt_id}</dd><dt>{t.spec}</dt><dd>{record.evidence.contract.artifact_policy.subject.spec_revision}</dd><dt>{t.inputRevision}</dt><dd><code>{record.evidence.contract.artifact_policy.subject.input_revision}</code></dd></dl>{record.evidence.failure && <dl><dt>{t.code}</dt><dd>{record.evidence.failure.code}</dd><dt>{t.failure}</dt><dd><code>{record.evidence.failure.pointer}</code></dd><dt>{t.line}</dt><dd>{record.evidence.failure.line ?? t.notObserved}</dd><dt>{t.failureColumn}</dt><dd>{record.evidence.failure.column ?? t.notObserved}</dd></dl>}<details><summary>{t.details}</summary>{[...record.evidence.file_integrity.inputs, ...record.evidence.file_integrity.outputs].map(item => <dl key={item.artifact_id}><dt>{t.pathColumn}</dt><dd><code>{item.relative_path}</code></dd><dt>{t.expected}</dt><dd><code>{item.expected_sha256 ?? t.notObserved}</code></dd><dt>{t.observed}</dt><dd><code>{item.observed_sha256 ?? t.notObserved}</code></dd><dt>{t.bytes}</dt><dd>{item.size ?? t.notObserved}</dd></dl>)}</details></details>)}
  </div>;
}
