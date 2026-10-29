import { useCallback, useEffect, useRef, useState } from 'react';
import { api, DaemonError, type Project, type Task } from './api';
import type { Language } from './i18n';
import { parseTaskWithWorkspaceBinding, parseTaskWorkspaceBindingView, validateTaskGitWorkspacePreparation, type TaskGitWorkspacePreparation, type TaskWorkspaceBindingView, type TaskWorkspaceReservationReceipt } from './task-workspace-binding-wire';
import { parseCodeWorkspacePreparation, type CodeWorkspacePreparation } from './workspace-preparation-wire';
import { taskCodeWorkspaceMessages } from './task-code-workspace-i18n';

type Props = { task: Task; language: Language; connected: boolean };
type Intent = { projectId: string; selection: TaskGitWorkspacePreparation; key: string; uncertain: boolean; reconciled: boolean };
type Observation = { value: TaskWorkspaceBindingView; at: string };
type Memory = { active: number; pending: Intent | null; observed: Observation | null; taskFloor: number; bindingFloor: number; receiptFloor: TaskWorkspaceReservationReceipt | null; writing: boolean };
// Same-document intent retention across task-detail remounts. No background work
// or persistence of private identity/credentials; the Core receipt remains truth.
const memories = new Map<string, Memory>();
const fingerprint = (task: Task) => JSON.stringify([task.id, task.project_id, task.revision, task.spec_revision, task.state]);
const equal = (left: unknown, right: unknown): boolean => {
  if (left === right) return true;
  if (!left || !right || typeof left !== 'object' || typeof right !== 'object' || Array.isArray(left) !== Array.isArray(right)) return false;
  const a = left as Record<string, unknown>, b = right as Record<string, unknown>;
  return Object.keys(a).length === Object.keys(b).length && Object.keys(a).every(key => Object.hasOwn(b, key) && equal(a[key], b[key]));
};
const fromReceipt = (r: TaskWorkspaceReservationReceipt): TaskGitWorkspacePreparation => validateTaskGitWorkspacePreparation({ task_id: r.task_id, expected_task_revision: r.original_task_revision, expected_spec_revision: r.spec_revision, expected_project_revision: r.project_revision, host_id: r.host_id, expected_source_pin: r.source_pin, preparation_key: r.preparation_key }, r.request_key);
const unknown = (error: unknown) => !(error instanceof DaemonError) || !['none', 'not_started'].includes(error.sideEffectState) || error.code === 'OUTCOME_UNKNOWN';
type Read = { task: Task; project: Project | null; preparation: CodeWorkspacePreparation | null; parentIdentity: string };

export function TaskCodeWorkspace(props: Props) {
  const [open, setOpen] = useState(false), [started, setStarted] = useState(false);
  return <details className="task-code-workspace" data-testid="task-code-workspace" onToggle={event => { setOpen(event.currentTarget.open); if (event.currentTarget.open) setStarted(true); }}>
    <summary>{taskCodeWorkspaceMessages[props.language].title}</summary>
    {started && <WorkspacePanel key={props.task.id} {...props} open={open} />}
  </details>;
}
function WorkspacePanel(props: Props & { open: boolean }) {
  const t = taskCodeWorkspaceMessages[props.language];
  const [saved] = useState<Memory>(() => {
    const prior = memories.get(props.task.id); if (prior) return prior;
    if (memories.size >= 32) { const idle = [...memories].find(([, value]) => !value.active && !value.pending && !value.writing); if (idle) memories.delete(idle[0]); }
    const value: Memory = { active: 0, pending: null, observed: null, taskFloor: 0, bindingFloor: 0, receiptFloor: null, writing: false };
    if (memories.size < 32) memories.set(props.task.id, value);
    return value;
  });
  const capacity = memories.get(props.task.id) !== saved;
  const current = useRef(props), mounted = useRef(true), epoch = useRef(0); current.current = props;
  const [read, setRead] = useState<Read | null>(null), [fresh, setFresh] = useState(false), [loading, setLoading] = useState(false), [busy, setBusy] = useState(false), [error, setError] = useState(''), [pending, setPending] = useState(saved.pending), [observed, setObserved] = useState(saved.observed);
  useEffect(() => { mounted.current = true; ++saved.active; return () => { mounted.current = false; --saved.active; ++epoch.current; }; }, [saved]);
  function remember(op: Intent | null) { saved.pending = op; if (mounted.current) setPending(op); }
  const refresh = useCallback(async () => {
    const stamp = ++epoch.current, initial = current.current, identity = fingerprint(initial.task);
    setFresh(false); setError('');
    if (!initial.connected) { setError(t.disconnected); return; }
    setLoading(true);
    try {
      const task = parseTaskWithWorkspaceBinding(await api.taskGet(initial.task.id), initial.task.id);
      const projects = await api.projectsList();
      if (!Array.isArray(projects) || projects.filter(p => p.id === task.project_id).length > 1) throw new Error(t.invalid);
      const project = projects.find(p => p.id === task.project_id) ?? null;
      if (task.revision < saved.taskFloor || task.revision < initial.task.revision || task.project_id !== initial.task.project_id && !(task.project_id === 'local' && !initial.task.project_id) || task.spec_revision !== initial.task.spec_revision && initial.task.spec_revision !== undefined) throw new Error(t.invalid);
      if (task.project_id !== 'local' && (!project || !Number.isSafeInteger(project.revision) || project.revision < 1)) throw new Error(t.invalid);
      const binding = task.code_workspace_binding;
      if (binding && (binding.revision < saved.bindingFloor || saved.receiptFloor && !equal(binding.receipt, saved.receiptFloor)) || !binding && saved.bindingFloor > 0) throw new Error(t.invalid);
      let preparation: CodeWorkspacePreparation | null = null;
      // Archived projects still expose Task/receipt history. Their resolver
      // correctly refuses new code preparation, so do not invoke it for history.
      if (!binding && !project?.archived) {
        const selection = { task_id: task.id, expected_task_revision: task.revision, expected_project_revision: project?.revision ?? 0, host_id: 'local' as const };
        preparation = parseCodeWorkspacePreparation(await api.workspacesResolve(selection), selection);
        if (preparation.project_id !== task.project_id || preparation.spec_revision !== task.spec_revision) throw new Error(t.invalid);
      }
      if (!mounted.current || stamp !== epoch.current || !current.current.connected || fingerprint(current.current.task) !== identity) return;
      saved.taskFloor = task.revision; saved.bindingFloor = binding?.revision ?? saved.bindingFloor;
      if (binding) saved.receiptFloor = structuredClone(binding.receipt);
      const op = saved.pending;
      if (op) {
        if (op.projectId !== task.project_id) throw new Error(t.invalid);
        if (binding && (!equal(fromReceipt(binding.receipt), op.selection) || binding.receipt.request_key !== op.key)) throw new Error(t.invalid);
        if (binding?.state === 'ready' && saved.observed && equal(binding, saved.observed.value.binding) && task.revision >= saved.observed.value.task_revision) remember(null);
        else remember({ ...op, reconciled: true });
      }
      const evidence = saved.observed;
      if (evidence && (!binding || !equal(binding, evidence.value.binding) || task.revision < evidence.value.task_revision)) { saved.observed = null; setObserved(null); }
      setRead({ task, project, preparation, parentIdentity: identity }); setFresh(true);
    } catch (failure) {
      if (mounted.current && stamp === epoch.current) { setFresh(false); setError(`${t.error} ${failure instanceof Error ? failure.message : ''}`); }
    } finally { if (mounted.current && stamp === epoch.current) setLoading(false); }
  }, [saved, t]);
  useEffect(() => { ++epoch.current; setFresh(false); if (props.open) void refresh(); }, [props.open, props.connected, props.task.id, props.task.project_id, props.task.revision, props.task.spec_revision, props.task.state, refresh]);
  const currentBinding = read?.task.code_workspace_binding;
  const staleBinding = !!currentBinding && (currentBinding.receipt.project_id !== read?.task.project_id || currentBinding.receipt.spec_revision !== read?.task.spec_revision || currentBinding.receipt.project_revision !== read?.project?.revision);
  const disabled = !props.connected || !fresh || !read || read.parentIdentity !== fingerprint(props.task) || loading || busy || saved.writing || capacity;
  const canPrepare = !disabled && !pending && !currentBinding && read?.task.state === 'draft' && !read.project?.archived && read.preparation?.state === 'needs_git_workspace' && read.preparation.source?.kind.kind === 'git' && !!read.preparation.preparation_key;
  async function perform(op: Intent, recovery: boolean) {
    if (disabled || saved.writing || !read || op.projectId !== read.task.project_id || !current.current.connected || read.parentIdentity !== fingerprint(current.current.task) || recovery && (!op.reconciled || saved.pending?.key !== op.key) || !recovery && !canPrepare) return;
    const identity = fingerprint(current.current.task);
    saved.writing = true; setBusy(true); setError(''); setFresh(false); ++epoch.current;
    remember({ ...op, reconciled: false });
    try {
      const value = parseTaskWorkspaceBindingView(await api.workspacesPrepareTaskGit(op.selection, op.key), op.selection, op.key);
      if (value.original_receipt.project_id !== read.task.project_id || value.task_revision < saved.taskFloor || value.binding.revision < saved.bindingFloor) throw new Error(t.invalid);
      saved.observed = { value, at: new Date().toISOString() };
      if (mounted.current) { setObserved(saved.observed); if (fingerprint(current.current.task) === identity) await refresh(); }
    } catch (failure) {
      if (op.uncertain || unknown(failure)) remember({ ...op, uncertain: true, reconciled: false });
      else remember(null);
      if (mounted.current) setError(`${t.error} ${failure instanceof Error ? failure.message : ''}`);
    } finally { saved.writing = false; if (mounted.current) setBusy(false); }
  }
  function prepare() {
    const p = read?.preparation, source = p?.source;
    if (!canPrepare || !p || !source || source.kind.kind !== 'git' || p.project_revision === null || p.preparation_key === null) return;
    const key = crypto.randomUUID();
    const selection = validateTaskGitWorkspacePreparation({ task_id: p.task_id, expected_task_revision: p.task_revision, expected_spec_revision: p.spec_revision, expected_project_revision: p.project_revision, host_id: p.host_id, expected_source_pin: { canonical_root: source.canonical_root, device: source.device, inode: source.inode, common_dir: source.kind.common_dir, head: source.kind.head }, preparation_key: p.preparation_key }, key);
    void perform({ projectId: read!.task.project_id!, selection, key, uncertain: false, reconciled: false }, false);
  }
  function revalidate() {
    if (disabled || pending || !currentBinding) return;
    const op = { projectId: currentBinding.receipt.project_id, selection: fromReceipt(currentBinding.receipt), key: currentBinding.receipt.request_key, uncertain: false, reconciled: true };
    remember(op); void perform(op, true);
  }
  const receipt = currentBinding?.receipt ?? observed?.value.original_receipt;
  const revalidated = !!observed && !!currentBinding && equal(observed.value.binding, currentBinding) && !staleBinding && fresh && read?.parentIdentity === fingerprint(props.task);
  return <section aria-label={t.title} aria-busy={loading || busy}>
    <p>{t.intro}</p><button type="button" disabled={!props.connected || loading || busy || saved.writing} onClick={() => void refresh()}>{t.refresh}</button>
    {!props.connected && <p>{t.disconnected}</p>}{loading && <p role={props.open ? 'status' : undefined}>{t.loading}</p>}{error && <p className="error" role={props.open ? 'alert' : undefined}>{error}</p>}{capacity && <p>{t.capacity}</p>}
    {pending && <p role={props.open ? 'status' : undefined}>{pending.uncertain ? t.uncertain : t.acknowledged}</p>}
    {pending && <button type="button" disabled={disabled || !pending.reconciled} onClick={() => void perform(pending, true)}>{t.recover}</button>}
    {read && <>
      <dl><dt>{t.project}</dt><dd>{read.project?.name ?? read.task.project_id}</dd><dt>{t.host}</dt><dd>{t.local}</dd><dt>{t.currentRevision}</dt><dd>{read.task.revision}</dd></dl>
      {read.project?.archived && <p>{t.archived}</p>}
      {read.preparation?.state === 'needs_registered_project' && <p>{t.personal}</p>}
      {read.preparation?.source && <dl><dt>{t.source}</dt><dd><code>{read.preparation.source.canonical_root}</code></dd>{read.preparation.source_head && <><dt>{t.head}</dt><dd><code>{read.preparation.source_head}</code></dd></>}</dl>}
      {read.preparation?.source?.kind.kind === 'git' && read.preparation.source.kind.dirty && <p>{t.dirty}</p>}
      {read.preparation?.state === 'needs_dataset_selection' && <p>{t.directory}</p>}
      {read.preparation?.state === 'blocked' && <p>{t.blocked}</p>}
      {!currentBinding && <><p>{t.none}</p><p>{t.confirm}</p><button className="primary" type="button" disabled={!canPrepare} onClick={prepare}>{t.prepare}</button></>}
      {read.preparation && read.preparation.candidates.length > 0 && <details><summary>{t.candidates}</summary><p>{t.candidatesHint}</p><ul>{read.preparation.candidates.map(row => <li key={row.workspace_id}><code>{row.root}</code> · {row.recorded_state}{row.has_unreconciled_lease && <span> · {t.lease}</span>}</li>)}</ul></details>}
      {currentBinding && <><h4>{t.current}</h4><p>{currentBinding.state === 'ready' ? t.ready : currentBinding.state === 'pending' ? t.pending : t.unknown}</p><p>{revalidated ? t.identity : t.recordedOnly}</p>{staleBinding && <p>{t.stale}</p>}<button type="button" disabled={disabled || !!pending} onClick={revalidate}>{t.revalidate}</button></>}
    </>}
    {receipt && <details><summary>{t.details}</summary><h4>{t.receipt}</h4><dl><dt>{t.bindingId}</dt><dd>{receipt.binding_id}</dd><dt>{t.workspaceId}</dt><dd>{receipt.workspace_id}</dd><dt>{t.host}</dt><dd>{receipt.host_id === 'local' ? t.local : receipt.host_id}</dd><dt>{t.originalRevision}</dt><dd>{receipt.original_task_revision}</dd><dt>{t.reservedRevision}</dt><dd>{receipt.reserved_task_revision}</dd><dt>{t.spec}</dt><dd>{receipt.spec_revision}</dd><dt>{t.projectRevision}</dt><dd>{receipt.project_revision}</dd><dt>{t.destination}</dt><dd><code>{receipt.managed_root.replace(/\/$/, '') + '/' + receipt.workspace_id}</code></dd><dt>{t.source}</dt><dd><code>{receipt.source_pin.canonical_root}</code></dd><dt>{t.head}</dt><dd><code>{receipt.source_pin.head}</code></dd><dt>{t.sourceDevice}</dt><dd><code>{receipt.source_pin.device}</code></dd><dt>{t.sourceInode}</dt><dd><code>{receipt.source_pin.inode}</code></dd><dt>{t.requestDigest}</dt><dd><code>{receipt.request_digest}</code></dd><dt>{t.key}</dt><dd><code>{receipt.request_key}</code></dd><dt>{t.preparationKey}</dt><dd><code>{receipt.preparation_key}</code></dd>{currentBinding && <><dt>{t.bindingRevision}</dt><dd>{currentBinding.revision}</dd></>}{observed && <><dt>{t.observed}</dt><dd>{observed.at}</dd><dt>{t.observedRoot}</dt><dd><code>{observed.value.observed_managed_pin.canonical_root}</code></dd><dt>{t.managedHead}</dt><dd><code>{observed.value.observed_managed_pin.head}</code></dd></>}</dl></details>}
    {!fresh && !loading && read && <p>{t.unavailable}</p>}<p className="hint">{t.noAuthority}</p>
  </section>;
}
