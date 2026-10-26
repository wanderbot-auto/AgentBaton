import { useCallback, useEffect, useRef, useState, type FormEvent } from 'react';
import { DaemonError, type Task } from './api';
import type { Language } from './i18n';
import type { ManagedSession } from './session-api';
import { ArtifactSelectionError, maxArtifactMiB, parseValidationSelection, validationApi, type ArtifactEvidence, type ValidationPolicy, type ValidationRecord, type ValidationSelection } from './validation-api';
import { validationMessages } from './validation-i18n';

type FormState = { sessionId: string; inputs: string; outputs: string; cap: string };
type Pending = { kind: 'approve' | 'run' | 'revoke'; key: string; uncertain: boolean; refreshed: boolean; form: FormState; selection?: ValidationSelection; policyId?: string; revision?: number };
// Unresolved intent survives tab/task navigation in this UI process. This is not
// a durable outbox or a claim that unknown outcomes survive application restart.
const pendingRequests = new Map<string, Pending>();
const artifactStates = new Set(['passed', 'missing', 'unsafe_or_unreadable', 'too_large', 'digest_mismatch']);
function uncertain(error: unknown) { return !(error instanceof DaemonError) || error.code === 'OUTCOME_UNKNOWN' || !['none', 'not_started'].includes(error.sideEffectState); }
function usableRevision(policy: ValidationPolicy) { return Number.isSafeInteger(policy.revision) && policy.revision >= 1; }
export interface ArtifactValidationProps { task: Task; sessions: ManagedSession[]; language: Language; connected: boolean }

export function ArtifactValidation({ task, sessions, language, connected }: ArtifactValidationProps) {
  const t = validationMessages[language], saved = pendingRequests.get(task.id);
  const eligible = sessions.filter(session => session.task_id === task.id && !!task.project_id && session.project_id === task.project_id && ['stopped', 'failed'].includes(session.state));
  const [form, setForm] = useState<FormState>(() => saved?.form ?? { sessionId: eligible[0]?.id ?? '', inputs: '', outputs: '', cap: '8' });
  const [policies, setPolicies] = useState<ValidationPolicy[]>([]), [records, setRecords] = useState<ValidationRecord[]>([]);
  const [pending, setPending] = useState<Pending | null>(saved ?? null);
  const [busy, setBusy] = useState(false), [loading, setLoading] = useState(false), [error, setError] = useState('');
  const sequence = useRef(0), mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; ++sequence.current; }; }, []);
  useEffect(() => { if (!form.sessionId && eligible.length && !pending) setForm(previous => ({ ...previous, sessionId: eligible[0].id })); }, [eligible, form.sessionId, pending]);
  const policyInScope = useCallback((policy: ValidationPolicy) => !!policy && policy.task_id === task.id && policy.project_id === task.project_id && policy.contract?.subject?.task_id === task.id && policy.scope === 'artifact_integrity_only' && Array.isArray(policy.contract.inputs) && Array.isArray(policy.contract.outputs), [task.id, task.project_id]);
  const recordInScope = useCallback((record: ValidationRecord) => !!record && record.task_id === task.id && record.evidence?.subject?.task_id === task.id && record.scope === 'artifact_integrity_only' && record.correctness === 'not_established' && record.task_acceptance === 'not_established' && record.integration_authorized === false && Array.isArray(record.evidence.inputs) && Array.isArray(record.evidence.outputs) && (record.state !== 'integrity_passed' || record.evidence.passed === true), [task.id]);
  const refresh = useCallback(async () => {
    // A retry requires the latest reconciliation to succeed. An earlier good
    // read must not authorize replay after a later failed or foreign-scope read.
    const previous = pendingRequests.get(task.id);
    if (previous) { const blocked = { ...previous, refreshed: false }; pendingRequests.set(task.id, blocked); setPending(blocked); }
    if (!connected) { setError(t.disconnected); return; }
    const request = ++sequence.current; setLoading(true);
    const [p, r] = await Promise.allSettled([validationApi.policiesList(task.id), validationApi.recordsList(task.id)]);
    if (!mounted.current || request !== sequence.current) return;
    let problem = '';
    if (p.status === 'fulfilled') { if (!Array.isArray(p.value) || p.value.some(policy => !policyInScope(policy))) problem = t.scopeMismatch; else setPolicies(p.value); }
    else problem = p.reason instanceof Error ? p.reason.message : t.localError;
    if (r.status === 'fulfilled') { if (!Array.isArray(r.value) || r.value.some(record => !recordInScope(record))) problem = t.scopeMismatch; else setRecords(r.value); }
    else problem ||= r.reason instanceof Error ? r.reason.message : t.localError;
    const unresolved = pendingRequests.get(task.id);
    if (!problem && unresolved) {
      const revoked = unresolved.kind === 'revoke' && p.status === 'fulfilled' && p.value.some(policy => policy.id === unresolved.policyId && policy.revoked);
      if (revoked) { pendingRequests.delete(task.id); setPending(null); }
      else { const next = { ...unresolved, refreshed: true }; pendingRequests.set(task.id, next); setPending(next); }
    } else if (!unresolved) setPending(null);
    setError(problem); setLoading(false);
  }, [connected, policyInScope, recordInScope, task.id, t.disconnected, t.localError, t.scopeMismatch]);
  useEffect(() => { void refresh(); }, [refresh]);
  const canRetry = (kind: Pending['kind'], policyId?: string) => !!pending && pending.kind === kind && pending.uncertain && pending.refreshed && (!policyId || pending.policyId === policyId);
  function clear(operation: Pending) { if (pendingRequests.get(task.id)?.key === operation.key) pendingRequests.delete(task.id); if (mounted.current) setPending(null); }
  async function perform<T>(operation: Pending, action: () => Promise<T>, apply: (value: T) => void) {
    if (!pendingRequests.has(task.id) && pendingRequests.size >= 32) { setError(t.pendingLimit); return; }
    const next = { ...operation, uncertain: false, refreshed: false }; pendingRequests.set(task.id, next); setPending(next); setBusy(true); setError('');
    try { const result = await action(); apply(result); ++sequence.current; if (mounted.current) setLoading(false); clear(operation); }
    catch (failure) {
      if (uncertain(failure)) { const unknown = { ...operation, uncertain: true, refreshed: false }; pendingRequests.set(task.id, unknown); if (mounted.current) { setPending(unknown); setError(t.unknown); } }
      else { clear(operation); if (mounted.current) setError(`${failure instanceof DaemonError && failure.code === 'PERMISSION_DENIED' ? t.denied : t.localError} ${failure instanceof Error ? failure.message : ''}`); }
    } finally { if (mounted.current) setBusy(false); }
  }
  function operation(kind: Pending['kind']): Pending { return { kind, key: crypto.randomUUID(), uncertain: false, refreshed: false, form: { ...form } }; }
  async function approve(event: FormEvent) {
    event.preventDefault(); if (!connected || busy || loading || (pending && !canRetry('approve'))) return;
    let next: Pending;
    try {
      if (canRetry('approve')) next = pending!;
      else { if (!eligible.some(session => session.id === form.sessionId)) throw new ArtifactSelectionError('empty'); next = { ...operation('approve'), selection: parseValidationSelection(form.sessionId, form.inputs, form.outputs, Number(form.cap)) }; }
    } catch (failure) { setError(failure instanceof ArtifactSelectionError ? t[failure.reason] : t.localError); return; }
    if (!next.selection) return;
    await perform(next, () => validationApi.policiesApprove(next.selection!, next.key), policy => {
      if (!policyInScope(policy) || policy.session_id !== next.selection!.session_id) throw new Error(t.scopeMismatch);
      if (mounted.current) setPolicies(previous => [policy, ...previous.filter(record => record.id !== policy.id)]);
    });
  }
  async function run(policy: ValidationPolicy) {
    if (!connected || busy || loading || policy.revoked || !usableRevision(policy) || !eligible.some(session => session.id === policy.session_id) || (pending && !canRetry('run', policy.id))) return;
    const next = canRetry('run', policy.id) ? pending! : { ...operation('run'), policyId: policy.id, revision: policy.revision };
    await perform(next, () => validationApi.run(policy.id, next.revision!, next.key), record => {
      if (!recordInScope(record) || record.policy_id !== policy.id) throw new Error(t.scopeMismatch);
      if (mounted.current) setRecords(previous => [record, ...previous.filter(existing => existing.id !== record.id)]);
    });
  }
  async function revoke(policy: ValidationPolicy) {
    if (!connected || busy || loading || policy.revoked || !usableRevision(policy) || (pending && !canRetry('revoke', policy.id))) return;
    const next = canRetry('revoke', policy.id) ? pending! : { ...operation('revoke'), policyId: policy.id, revision: policy.revision };
    await perform(next, () => validationApi.revoke(policy.id, next.revision!), updated => {
      if (!policyInScope(updated) || updated.id !== policy.id || !updated.revoked) throw new Error(t.scopeMismatch);
      if (mounted.current) setPolicies(previous => previous.map(record => record.id === policy.id ? updated : record));
    });
  }
  const disabled = !connected || busy || loading;
  return <section className="artifact-validation" aria-busy={busy || loading} aria-labelledby={`integrity-heading-${task.id}`}>
    <div className="section-heading"><h2 id={`integrity-heading-${task.id}`}>{t.title}</h2><button type="button" onClick={() => void refresh()} disabled={!connected || busy || loading}>{t.refresh}</button></div>
    <p className="integrity-warning">{t.warning}</p><p className="hint">{t.localOnly}</p>
    {error && <p role="alert" className="error">{error}</p>}{pending && !pending.uncertain && <p role="status">{t.waiting}</p>}{pending?.uncertain && <p role="status">{t.unknown}{!pending.refreshed ? ` ${t.refreshFirst}` : ''}</p>}
    <details className="integrity-policy-form"><summary>{t.form}</summary>{!eligible.length && <p>{t.noSession}</p>}
      <form onSubmit={event => void approve(event)}><fieldset disabled={disabled || !!pending}><legend className="sr-only">{t.form}</legend>
        <label htmlFor={`integrity-session-${task.id}`}>{t.session}</label><select id={`integrity-session-${task.id}`} value={form.sessionId} onChange={event => setForm(previous => ({ ...previous, sessionId: event.target.value }))}><option value="">{t.chooseSession}</option>{form.sessionId && !eligible.some(session => session.id === form.sessionId) && <option value={form.sessionId} disabled>{t.sessionUnavailable}</option>}{eligible.map(session => <option key={session.id} value={session.id}>{session.id} · {session.state === 'failed' ? t.failedSession : t.stoppedSession}</option>)}</select><p className="hint">{t.sessionHint}</p>
        <label htmlFor={`integrity-outputs-${task.id}`}>{t.outputs}</label><textarea id={`integrity-outputs-${task.id}`} value={form.outputs} maxLength={8192} rows={3} onChange={event => setForm(previous => ({ ...previous, outputs: event.target.value }))} aria-describedby={`integrity-output-hint-${task.id}`} /><p id={`integrity-output-hint-${task.id}`} className="hint">{t.outputsHint}</p>
        <details><summary>{t.inputs}</summary><label className="sr-only" htmlFor={`integrity-inputs-${task.id}`}>{t.inputs}</label><textarea id={`integrity-inputs-${task.id}`} value={form.inputs} maxLength={8192} rows={2} onChange={event => setForm(previous => ({ ...previous, inputs: event.target.value }))} /><p className="hint">{t.inputsHint}</p></details>
        <label htmlFor={`integrity-cap-${task.id}`}>{t.cap}</label><input id={`integrity-cap-${task.id}`} type="number" min={1} max={maxArtifactMiB} step={1} value={form.cap} onChange={event => setForm(previous => ({ ...previous, cap: event.target.value }))} /><p className="hint">{t.bounds}</p>
      </fieldset><button type="submit" disabled={disabled || !eligible.some(session => session.id === form.sessionId) || !form.outputs.trim() || (!!pending && !canRetry('approve'))}>{canRetry('approve') ? t.approveAgain : t.approve}</button></form>
    </details>
    {!!policies.length && <section aria-label={t.policies} className="integrity-policies"><h3>{t.policies}</h3>{policies.map(policy => {
      const stale = task.spec_revision !== undefined && policy.contract.subject.spec_revision !== task.spec_revision;
      const stoppedSession = eligible.some(session => session.id === policy.session_id);
      return <details key={policy.id} className="integrity-policy"><summary>{policy.revoked ? t.revoked : t.approved} · <time dateTime={policy.created_at}>{policy.created_at}</time></summary><p>{t.policy}: <code>{policy.id}</code></p><p>{t.session}: <code>{policy.session_id}</code></p><p>{t.workspace}: <code>{policy.workspace_id}</code></p><details><summary>{t.policyDetails}</summary>{policy.contract.outputs.map(item => <p key={item.requirement.artifact_id}><code>{item.canonical_root}/{item.requirement.relative_path}</code> · {item.requirement.max_bytes} {t.bytes}</p>)}</details>{stale && <p>{t.stalePolicy}</p>}{!stoppedSession && !policy.revoked && <p>{t.noSession}</p>}<div className="integrity-actions"><button type="button" onClick={() => void run(policy)} disabled={disabled || policy.revoked || stale || !stoppedSession || !usableRevision(policy) || (!!pending && !canRetry('run', policy.id))}>{canRetry('run', policy.id) ? t.retryRun : t.run}</button><button type="button" onClick={() => void revoke(policy)} disabled={disabled || policy.revoked || !usableRevision(policy) || (!!pending && !canRetry('revoke', policy.id))}>{t.revoke}</button></div></details>;
    })}</section>}
    {!loading && !policies.length && <p className="hint">{t.noPolicies}</p>}{!records.length && <p className="hint">{t.noRecords}</p>}
    {!!records.length && <p className="hint">{t.historical}</p>}<div className="integrity-records">{records.map(record => <details key={record.id} className="integrity-record"><summary>{record.state === 'integrity_passed' ? t.checkPassed : record.state === 'failed' ? t.failed : `${t.unknownState}: ${record.state}`} · <time dateTime={record.created_at}>{record.created_at}</time></summary><p>{t.correctness}</p><p>{t.acceptance}</p><dl><dt>{t.digestLabel}</dt><dd><code>{record.evidence.digest}</code></dd><dt>{t.validator}</dt><dd><code>{record.evidence.validator_id}</code></dd><dt>{t.attempt}</dt><dd><code>{record.evidence.subject.attempt_id}</code></dd><dt>{t.spec}</dt><dd>{record.evidence.subject.spec_revision}</dd><dt>{t.inputRevision}</dt><dd><code>{record.evidence.subject.input_revision}</code></dd><dt>{t.policy}</dt><dd><code>{record.policy_id}</code></dd></dl><EvidenceTable title={t.inputsLabel} records={record.evidence.inputs} language={language} /><EvidenceTable title={t.outputsLabel} records={record.evidence.outputs} language={language} /></details>)}</div>
  </section>;
}
function EvidenceTable({ title, records, language }: { title: string; records: ArtifactEvidence[]; language: Language }) {
  const t = validationMessages[language]; if (!records.length) return null;
  return <div className="integrity-table-scroll"><table><caption>{title}</caption><thead><tr><th scope="col">{t.pathColumn}</th><th scope="col">{t.observed}</th><th scope="col">{t.bytes}</th><th scope="col">{t.state}</th></tr></thead><tbody>{records.map(record => <tr key={record.artifact_id}><th scope="row"><code>{record.relative_path}</code>{record.expected_sha256 && <details><summary>{t.expected}</summary><code>{record.expected_sha256}</code></details>}</th><td><code>{record.observed_sha256 ?? t.notObserved}</code></td><td>{record.size ?? t.notObserved}</td><td>{artifactStates.has(record.state) ? t[record.state as keyof typeof t] : `${t.unknownState}: ${record.state}`}</td></tr>)}</tbody></table></div>;
}
