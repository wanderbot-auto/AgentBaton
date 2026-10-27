import { useCallback, useEffect, useRef, useState, type FormEvent } from 'react';
import { DaemonError, type Task } from './api';
import type { Language } from './i18n';
import type { ManagedSession } from './session-api';
import { commandScope, commandValidator, commandValidationApi, CommandFormError, enforcementKeys, newCommandForm, parseCommandSelection, type CommandForm, type CommandPolicy, type CommandRecord, type CommandSelection } from './command-validation-api';
import { commandMessages } from './command-validation-i18n';
type Props = { task: Task; sessions: ManagedSession[]; language: Language; connected: boolean };
type Pending = { kind: 'approve' | 'run' | 'cancel' | 'revoke'; key: string; revision?: number; policyId?: string; runId?: string; selection?: CommandSelection; form: CommandForm; uncertain: boolean; reconciled: boolean };
const pendingRequests = new Map<string, Pending>();
const active = new Set(['prepared', 'running', 'cancel_requested']);
const knownStates = new Set([...active, 'command_passed', 'command_failed', 'outcome_unknown']);
const revision = (value: number) => Number.isSafeInteger(value) && value > 0;
const uncertain = (error: unknown) => !(error instanceof DaemonError) || error.code === 'OUTCOME_UNKNOWN' || !['none', 'not_started'].includes(error.sideEffectState);
export const commandPollLimit = 70;
export function CommandValidation(props: Props) {
  return <section className="command-validation" data-testid="command-validation" aria-labelledby={`command-heading-${props.task.id}`}><details><summary><h2 id={`command-heading-${props.task.id}`}>{commandMessages[props.language].title}</h2></summary><CommandPanel key={props.task.id} {...props} /></details></section>;
}
function CommandPanel({ task, sessions, language, connected }: Props) {
  const t = commandMessages[language], saved = pendingRequests.get(task.id);
  const eligible = sessions.filter(s => s.task_id === task.id && !!task.project_id && s.project_id === task.project_id && ['stopped', 'failed'].includes(s.state));
  const [form, setForm] = useState<CommandForm>(() => saved?.form ?? newCommandForm(eligible[0]?.id));
  const [policies, setPolicies] = useState<CommandPolicy[]>([]), [records, setRecords] = useState<CommandRecord[]>([]), [pending, setPending] = useState<Pending | null>(saved ?? null);
  const [busy, setBusy] = useState(false), [loading, setLoading] = useState(false), [fresh, setFresh] = useState(false), [error, setError] = useState(''), [observation, setObservation] = useState(0);
  const mounted = useRef(true), sequence = useRef(0), polls = useRef(0);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; ++sequence.current; }; }, []);
  useEffect(() => { if (!form.sessionId && eligible.length && !pending) setForm(previous => ({ ...previous, sessionId: eligible[0].id })); }, [eligible, form.sessionId, pending]);
  const policyInScope = useCallback((p: CommandPolicy) => !!p && p.task_id === task.id && p.project_id === task.project_id && p.scope === commandScope && p.validator_id === commandValidator && p.max_runs === 1 && revision(p.revision) && typeof p.revoked === 'boolean' && p.selection?.session_id === p.session_id && p.contract?.scope === commandScope && p.contract.validator_id === commandValidator && p.contract.file_contract?.subject?.task_id === task.id && p.contract.file_contract.subject.attempt_id === p.attempt_id && revision(p.contract.file_contract.subject.spec_revision) && typeof p.contract.program?.requirement?.expected_sha256 === 'string' && /^[a-fA-F0-9]{64}$/.test(p.contract.program.requirement.expected_sha256) && typeof p.contract_digest === 'string' && Array.isArray(p.contract.arguments) && typeof p.contract.argv0 === 'string' && typeof p.contract.cwd?.canonical_root === 'string' && Array.isArray(p.contract.file_contract.inputs) && Array.isArray(p.contract.file_contract.outputs), [task.id, task.project_id]);
  const recordInScope = useCallback((r: CommandRecord, p?: CommandPolicy) => {
    if (!r || !p || !policyInScope(p) || r.task_id !== task.id || r.scope !== commandScope || r.policy_id !== p.id || r.session_id !== p.session_id || r.workspace_id !== p.workspace_id || r.attempt_id !== p.attempt_id || !revision(r.revision) || r.correctness !== 'approved_fixed_command_observations_only' || r.task_acceptance !== 'not_established' || r.integration_authorized !== false) return false;
    const e = r.evidence;
    if (e && (e.validator_id !== commandValidator || e.scope !== commandScope || e.contract_digest !== p.contract_digest || e.approval_id !== p.approval_id || e.contract?.file_contract?.subject?.task_id !== task.id || e.contract.file_contract.subject.attempt_id !== r.attempt_id || e.task_acceptance !== 'not_established' || e.integration_authorized !== false || !e.cleanup || !e.stdout || !e.stderr)) return false;
    return r.state !== 'command_passed' || (e?.passed === true && e.process_started === true && e.cleanup.owned_leader_reaped === true && e.exit_observed_within_budget === true && e.cleanup_completed_within_budget === true && e.file_integrity?.passed === true);
  }, [policyInScope, task.id]);
  const refresh = useCallback(async (manual = false) => {
    if (manual) polls.current = 0;
    const previous = pendingRequests.get(task.id);
    if (previous) { const blocked = { ...previous, reconciled: false }; pendingRequests.set(task.id, blocked); setPending(blocked); }
    setFresh(false);
    if (!connected) { setError(t.disconnected); return; }
    const request = ++sequence.current; setLoading(true);
    const [p, r] = await Promise.allSettled([commandValidationApi.policiesList(task.id), commandValidationApi.recordsList(task.id)]);
    if (!mounted.current || request !== sequence.current) return;
    let problem = '';
    if (p.status === 'rejected') problem = p.reason instanceof Error ? p.reason.message : t.localError;
    else if (!Array.isArray(p.value) || p.value.some(item => !policyInScope(item)) || new Set(p.value.map(item => item.id)).size !== p.value.length) problem = t.scopeMismatch;
    if (r.status === 'rejected') problem ||= r.reason instanceof Error ? r.reason.message : t.localError;
    else if (!problem && (!Array.isArray(r.value) || p.status !== 'fulfilled' || !Array.isArray(p.value) || r.value.some(item => !item || !recordInScope(item, p.value.find(policy => policy.id === item.policy_id))) || new Set(r.value.map(item => item.policy_id)).size !== r.value.length)) problem = t.scopeMismatch;
    if (!problem && p.status === 'fulfilled' && r.status === 'fulfilled') {
      setPolicies(p.value); setRecords(r.value); setFresh(true);
      const unresolved = pendingRequests.get(task.id);
      // max_runs=1 makes any scoped run for this policy the only possible
      // execution. Observing it resolves the request without submitting again.
      // A scoped terminal failure also settles cancellation intent, including a
      // pre-spawn refusal with no receipt. It is NOT a claim of process stop;
      // only cleanup evidence below can describe a reaped owned leader.
      const resolved = unresolved && ((unresolved.kind === 'run' && r.value.some(item => item.policy_id === unresolved.policyId)) || (unresolved.kind === 'revoke' && p.value.some(item => item.id === unresolved.policyId && item.revoked)) || (unresolved.kind === 'cancel' && r.value.some(item => item.id === unresolved.runId && item.revision >= unresolved.revision! && (item.state === 'cancel_requested' || item.state === 'command_failed' || item.evidence?.cleanup.owned_leader_reaped))));
      if (resolved) { pendingRequests.delete(task.id); setPending(null); }
      else if (unresolved) { const next = { ...unresolved, reconciled: true }; pendingRequests.set(task.id, next); setPending(next); }
      else setPending(null);
    }
    setError(problem); setLoading(false); setObservation(value => value + 1);
  }, [connected, policyInScope, recordInScope, task.id, t.disconnected, t.localError, t.scopeMismatch]);
  useEffect(() => { void refresh(); }, [refresh]);
  useEffect(() => {
    if (!fresh || !connected || loading || busy || !records.some(record => active.has(record.state)) || polls.current >= commandPollLimit) return;
    const timer = window.setTimeout(() => { ++polls.current; void refresh(); }, 1000);
    return () => window.clearTimeout(timer);
  }, [fresh, connected, loading, busy, records, observation, refresh]);
  const retry = (kind: Pending['kind'], id?: string) => !!pending && pending.kind === kind && pending.uncertain && pending.reconciled && (!id || (kind === 'cancel' ? pending.runId : pending.policyId) === id);
  const disabled = !connected || busy || loading || !fresh;
  const operation = (kind: Pending['kind']): Pending => ({ kind, key: crypto.randomUUID(), form: { ...form, require: { ...form.require } }, uncertain: false, reconciled: false });
  function clear(op: Pending) { if (pendingRequests.get(task.id)?.key === op.key) pendingRequests.delete(task.id); if (mounted.current) setPending(null); }
  async function perform<T>(op: Pending, action: () => Promise<T>, apply: (value: T) => void) {
    if (!pendingRequests.has(task.id) && pendingRequests.size >= 32) { setError(t.pendingLimit); return; }
    const next = { ...op, uncertain: false, reconciled: false }; pendingRequests.set(task.id, next); setPending(next); setBusy(true); setError('');
    try { const value = await action(); apply(value); ++sequence.current; clear(op); }
    catch (failure) {
      if (uncertain(failure)) { const unknown = { ...op, uncertain: true, reconciled: false }; pendingRequests.set(task.id, unknown); if (mounted.current) { setPending(unknown); setError(t.unknown); } }
      else { clear(op); if (mounted.current) setError(`${failure instanceof DaemonError && failure.code === 'PERMISSION_DENIED' ? t.denied : t.localError} ${failure instanceof Error ? failure.message : ''}`); }
    } finally { if (mounted.current) setBusy(false); }
  }
  async function approve(event: FormEvent) {
    event.preventDefault(); if (disabled || (pending && !retry('approve'))) return;
    let op: Pending;
    try { if (retry('approve')) op = pending!; else { if (!eligible.some(s => s.id === form.sessionId)) throw new CommandFormError('empty'); op = { ...operation('approve'), selection: parseCommandSelection(form) }; } }
    catch (failure) { setError(failure instanceof CommandFormError ? failure.reason === 'arguments' ? t.argumentsError : failure.reason === 'environment' ? t.environmentError : t[failure.reason] : t.localError); return; }
    await perform(op, () => commandValidationApi.approve(op.selection!, op.key), value => { if (!policyInScope(value) || value.session_id !== op.selection!.session_id) throw new Error(t.scopeMismatch); if (mounted.current) setPolicies(previous => [value, ...previous.filter(item => item.id !== value.id)]); });
  }
  const fenced = (p: CommandPolicy) => records.some(record => record.workspace_id === p.workspace_id && record.state === 'outcome_unknown');
  const runDisabled = (p: CommandPolicy) => disabled || p.revoked || fenced(p) || !eligible.some(s => s.id === p.session_id) || p.contract.file_contract.subject.spec_revision !== task.spec_revision || (!retry('run', p.id) && records.some(record => record.policy_id === p.id)) || (!!pending && !retry('run', p.id));
  async function run(p: CommandPolicy) {
    if (runDisabled(p)) return;
    const op = retry('run', p.id) ? pending! : { ...operation('run'), policyId: p.id, revision: p.revision };
    await perform(op, () => commandValidationApi.run(p.id, op.revision!, op.key), value => { if (!recordInScope(value, p)) throw new Error(t.scopeMismatch); if (mounted.current) { polls.current = 0; setRecords(previous => [value, ...previous.filter(item => item.id !== value.id)]); } });
  }
  async function revoke(p: CommandPolicy) {
    if (disabled || p.revoked || (pending && !retry('revoke', p.id))) return;
    const op = retry('revoke', p.id) ? pending! : { ...operation('revoke'), policyId: p.id, revision: p.revision };
    await perform(op, () => commandValidationApi.revoke(p.id, op.revision!, op.key), value => { if (!policyInScope(value) || value.id !== p.id || !value.revoked) throw new Error(t.scopeMismatch); if (mounted.current) setPolicies(previous => previous.map(item => item.id === p.id ? value : item)); });
  }
  async function cancel(r: CommandRecord) {
    if (disabled || !active.has(r.state) || (r.state === 'cancel_requested' && !retry('cancel', r.id)) || (pending && !retry('cancel', r.id))) return;
    const op = retry('cancel', r.id) ? pending! : { ...operation('cancel'), runId: r.id, revision: r.revision };
    await perform(op, () => commandValidationApi.cancel(r.id, op.revision!, op.key), value => { if (!recordInScope(value, policies.find(item => item.id === r.policy_id)) || value.id !== r.id) throw new Error(t.scopeMismatch); if (mounted.current) setRecords(previous => previous.map(item => item.id === r.id ? value : item)); });
  }
  const update = (key: keyof CommandForm, value: string | boolean) => setForm(previous => ({ ...previous, [key]: value }));
  const field = (key: keyof CommandForm, label: string, maxLength = 4096) => <label>{label}<input value={String(form[key])} maxLength={maxLength} onChange={event => update(key, event.target.value)} /></label>;
  const count = (value: number | null) => value === null ? t.notObserved : Number.isSafeInteger(value) ? String(value) : t.limit;
  return <div aria-busy={busy || loading}><p>{t.warning}</p><p className="hint">{t.scopeWarning}</p><button type="button" onClick={() => void refresh(true)} disabled={!connected || busy || loading}>{t.refresh}</button>
    {error && <p role="alert" className="error">{error}</p>}{pending && <p role="status">{pending.uncertain ? t.unknown : t.waiting}</p>}{polls.current >= commandPollLimit && <p>{t.pollEnded}</p>}
    <form onSubmit={event => void approve(event)}><fieldset disabled={disabled || !!pending}><legend>{t.form}</legend><label>{t.session}<select value={form.sessionId} onChange={event => update('sessionId', event.target.value)}><option value="">{t.chooseSession}</option>{form.sessionId && !eligible.some(s => s.id === form.sessionId) && <option value={form.sessionId} disabled>{t.sessionUnavailable}</option>}{eligible.map(s => <option key={s.id} value={s.id}>{s.id} · {s.state}</option>)}</select></label><p className="hint">{t.sessionHint}</p>
      {field('executable', t.executable)}<label>{t.mode}<select value={form.mode} onChange={event => update('mode', event.target.value)}><option value="native_executable">{t.native}</option><option value="posix_sh_stdin">{t.scriptMode}</option></select></label>{form.mode === 'posix_sh_stdin' && <>{field('scriptPath', t.scriptPath, 240)}{field('scriptHash', t.scriptHash, 64)}</>}
      <label>{t.inputs}<textarea rows={2} maxLength={8192} value={form.inputs} onChange={event => update('inputs', event.target.value)} /></label><label>{t.outputs}<textarea rows={2} maxLength={8192} value={form.outputs} onChange={event => update('outputs', event.target.value)} /></label><p className="hint">{t.pathsHint}</p><label>{t.arguments}<textarea rows={2} maxLength={16384} value={form.arguments} onChange={event => update('arguments', event.target.value)} /></label><p className="hint">{t.argumentsHint}</p>
      <details><summary>{t.environment}</summary>{(['lang', 'lcAll'] as const).map(key => <label key={key}>{key === 'lang' ? 'LANG' : 'LC_ALL'}<select value={form[key]} onChange={event => update(key, event.target.value)}><option value="">{t.omitted}</option>{['C', 'C.UTF-8', 'en_US.UTF-8'].map(value => <option key={value}>{value}</option>)}</select></label>)}<label className="command-validation-checkbox"><input type="checkbox" checked={form.timezone} onChange={event => update('timezone', event.target.checked)} />{t.timezone}</label></details>
      <details><summary>{t.limits}</summary><div className="command-validation-fields">{field('cap', t.cap)}{field('exits', t.exits)}{field('wall', t.wall)}{field('cleanup', t.cleanup)}{field('stdout', t.stdout)}{field('stderr', t.stderr)}</div>{enforcementKeys.map(key => <label className="command-validation-checkbox" key={key}><input type="checkbox" checked={form.require[key]} onChange={event => setForm(previous => ({ ...previous, require: { ...previous.require, [key]: event.target.checked } }))} />{t[key]}</label>)}<p className="hint">{t.strictHint}</p></details>
    </fieldset><button className="primary" type="submit" disabled={disabled || !eligible.some(s => s.id === form.sessionId) || (!!pending && !retry('approve'))}>{retry('approve') ? t.approveAgain : t.approve}</button></form>
    {!policies.length && <p>{t.noPolicies}</p>}{policies.map(p => <details key={p.id}><summary>{p.revoked ? t.revoked : t.approved} · <time dateTime={p.created_at}>{p.created_at}</time></summary><p>{t.once}</p>{fenced(p) && <p>{t.unknownFence}</p>}<dl><dt>{t.programHash}</dt><dd><code>{p.contract.program.requirement.expected_sha256}</code></dd><dt>{t.executable}</dt><dd><code>{p.contract.argv0}</code></dd><dt>{t.cwd}</dt><dd><code>{p.contract.cwd.canonical_root}</code></dd>{p.contract.script && <><dt>{t.scriptHashLabel}</dt><dd><code>{p.contract.script.requirement.expected_sha256}</code></dd></>}<dt>{t.digestLabel}</dt><dd><code>{p.contract_digest}</code></dd></dl><details><summary>{t.policyDetails}</summary><ol>{p.contract.arguments.map((argument, i) => <li key={i}><code>{JSON.stringify(argument)}</code></li>)}</ol><p>{t.wall}: {count(p.contract.wall_time_ms)} · {t.cleanup}: {count(p.contract.cleanup_time_ms)}</p><p>{t.stdout}: {count(p.contract.stdout_limit)} · {t.stderr}: {count(p.contract.stderr_limit)}</p></details><div className="command-validation-actions"><button type="button" onClick={() => void run(p)} disabled={runDisabled(p)}>{retry('run', p.id) ? t.recoverRun : t.run}</button><button type="button" onClick={() => void revoke(p)} disabled={disabled || p.revoked || (!!pending && !retry('revoke', p.id))}>{t.revoke}</button></div></details>)}
    {!records.length && <p>{t.noRecords}</p>}{!!records.length && <p className="hint">{t.historical}</p>}{records.map(r => <details key={r.id}><summary>{knownStates.has(r.state) ? t[r.state as 'prepared'] : `${t.unknownState}: ${r.state}`} · <time dateTime={r.updated_at}>{r.updated_at}</time></summary><p>{t.scopeWarning}</p>{r.last_error && <p>{r.last_error.code}: {r.last_error.message}</p>}{r.state === 'outcome_unknown' && <p>{t.unknownFence}</p>}{active.has(r.state) && <button type="button" disabled={disabled || (r.state === 'cancel_requested' && !retry('cancel', r.id)) || (!!pending && !retry('cancel', r.id))} onClick={() => void cancel(r)}>{retry('cancel', r.id) ? t.retryCancel : t.cancel}</button>}{r.evidence && <><p>{r.evidence.cleanup.owned_leader_reaped ? t.stopConfirmed : t.stopUnknown}</p><dl><dt>{t.reason}</dt><dd>{r.evidence.reason}</dd><dt>{t.actualExit}</dt><dd>{count(r.evidence.exit_code)}</dd><dt>{t.elapsed}</dt><dd>{count(r.evidence.process_and_cleanup_elapsed_ms)}</dd><dt>{t.digestLabel}</dt><dd><code>{r.evidence.digest}</code></dd></dl><details><summary>{t.capture}</summary>{(['stdout', 'stderr'] as const).map(stream => <dl key={stream}><dt>{stream} {t.captured}</dt><dd>{count(r.evidence![stream].captured_bytes)}</dd><dt>{t.observed}</dt><dd>{count(r.evidence![stream].observed_bytes_at_least)}</dd><dt>SHA-256</dt><dd><code>{r.evidence![stream].sha256_of_captured}</code></dd><dt>{t.truncated}</dt><dd>{String(r.evidence![stream].truncated)}</dd><dt>{t.eof}</dt><dd>{String(r.evidence![stream].eof_observed)}</dd></dl>)}</details></>}</details>)}
  </div>;
}
