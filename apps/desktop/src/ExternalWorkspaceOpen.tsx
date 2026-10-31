import { useCallback, useEffect, useRef, useState } from 'react';
import type { Task } from './api';
import type { Language } from './i18n';
import { workspaceOpenApi, type WorkspaceOpenChoice } from './workspace-open-api';
import { workspaceOpenMessages } from './workspace-open-i18n';
import { validateWorkspaceOpenSelection, workspaceOpenId, type WorkspaceOpenResult } from './workspace-open-wire';

type Props = { task: Task; language: Language; connected: boolean; open: boolean; blocked?: boolean };
// Same-document uncertainty only. A remount cannot turn an unresolved handoff
// into permission to resend. Neither set stores a path, credential or authority.
const outstanding = new Set<string>(), uncertain = new Set<string>();
const key = (p: Props) => JSON.stringify([p.task.id, p.task.project_id]);
const identity = (p: Props) => JSON.stringify([p.task.id, p.task.project_id, p.task.revision, p.task.spec_revision, p.task.state, p.open, p.connected, !!p.blocked]);
export function ExternalWorkspaceOpen(props: Props) {
  const t = workspaceOpenMessages[props.language];
  const current = useRef(props); current.current = props;
  const mounted = useRef(true), epoch = useRef(0), writing = useRef(false);
  const [choices, setChoices] = useState<WorkspaceOpenChoice[]>([]), [selected, setSelected] = useState('');
  const [readIdentity, setReadIdentity] = useState(''), [fresh, setFresh] = useState(false), [loading, setLoading] = useState(false), [busy, setBusy] = useState(false);
  const [unknown, setUnknown] = useState(uncertain.has(key(props))), [retryChecked, setRetryChecked] = useState(false), [failed, setFailed] = useState(false), [result, setResult] = useState<WorkspaceOpenResult | null>(null);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; ++epoch.current; }; }, []);
  const valid = (stamp: number, expected: string) => mounted.current && epoch.current === stamp && identity(current.current) === expected && current.current.connected && current.current.open && !current.current.blocked;
  const refresh = useCallback(async (manual: boolean) => {
    if (writing.current || outstanding.size > 0) return;
    const p = current.current, expected = identity(p), stamp = ++epoch.current;
    setFresh(false); setFailed(false); setResult(null); setRetryChecked(false);
    if (!p.connected || !p.open || p.blocked || !workspaceOpenId(p.task.project_id)) { setLoading(false); return; }
    setLoading(true);
    try {
      const rows = await workspaceOpenApi.choices(p.task.project_id);
      if (!valid(stamp, expected)) return;
      setChoices(rows); setSelected(previous => rows.some(v => JSON.stringify(v.selection) === previous) ? previous : '');
      setReadIdentity(expected); setFresh(true); setRetryChecked(manual);
    } catch { if (valid(stamp, expected)) setFailed(true); }
    finally { if (mounted.current && stamp === epoch.current) setLoading(false); }
  }, []);
  const parentIdentity = identity(props), memoryKey = key(props);
  useEffect(() => {
    ++epoch.current; setFresh(false); setResult(null); setSelected(''); setChoices([]); setRetryChecked(false); setLoading(false); setUnknown(uncertain.has(key(current.current)));
    if (props.open && props.connected && !props.blocked) void refresh(false);
  }, [parentIdentity, refresh]);
  const capacity = uncertain.size >= 32 && !uncertain.has(memoryKey);
  const disabled = !props.open || !props.connected || !!props.blocked || !fresh || readIdentity !== parentIdentity || loading || busy || writing.current || outstanding.size > 0 || capacity;
  async function openDirectory() {
    const p = current.current, expected = identity(p), operationKey = key(p);
    if (disabled || !selected || writing.current || outstanding.size > 0 || unknown && !retryChecked || readIdentity !== expected) return;
    const choice = choices.find(v => JSON.stringify(v.selection) === selected); if (!choice) return;
    const selection = validateWorkspaceOpenSelection(choice.selection), stamp = ++epoch.current, priorUnknown = uncertain.has(operationKey);
    writing.current = true; outstanding.add(operationKey); uncertain.add(operationKey);
    setBusy(true); setUnknown(true); setResult(null); setFresh(false); setRetryChecked(false);
    try {
      const value = await workspaceOpenApi.open({ selection, terminal: 'macos_terminal' });
      if (!valid(stamp, expected)) return;
      setResult(value);
      if (value.status === 'handoff_accepted' || value.status === 'refused' && !priorUnknown) { uncertain.delete(operationKey); setUnknown(false); }
    } catch {
      if (valid(stamp, expected)) setResult({ status: 'outcome_unknown', code: 'transport_unknown' });
    } finally {
      writing.current = false; outstanding.delete(operationKey);
      // Old replies are never displayed as current success/refusal. Preserve
      // uncertainty under their original context; no automatic reconciliation.
      if (mounted.current) { setBusy(false); setUnknown(uncertain.has(key(current.current))); }
    }
  }
  return <section aria-label={t.title} aria-busy={busy || loading}>
    <h4>{t.title}</h4><p>{t.boundary}</p>
    {!props.connected && <p>{t.disconnected}</p>}{props.blocked && <p>{t.unavailable}</p>}{!workspaceOpenId(props.task.project_id) && <p>{t.noProject}</p>}
    <button type="button" disabled={!props.open || !props.connected || !!props.blocked || busy || loading || outstanding.size > 0} onClick={() => void refresh(true)}>{t.refresh}</button>
    {loading && <p role={props.open ? 'status' : undefined}>{t.loading}</p>}{busy && <p role={props.open ? 'status' : undefined}>{t.pending}</p>}
    {unknown && !busy && <p role={props.open ? 'status' : undefined}>{t.unknown}</p>}
    {capacity && <p>{t.capacity}</p>}
    {failed && <p role={props.open ? 'alert' : undefined}>{t.readFailed}</p>}
    <label>{t.target}<select aria-label={t.target} value={selected} disabled={disabled} onChange={event => {
      if (busy || writing.current || loading) return;
      ++epoch.current; setSelected(event.target.value); setResult(null);
    }}><option value="">{t.target}</option>{choices.map(choice => <option key={JSON.stringify(choice.selection)} value={JSON.stringify(choice.selection)}>{t[choice.label]} · {choice.recordedPath}</option>)}</select></label>
    <p>{t.recorded}</p><label>{t.terminal}<select aria-label={t.terminal} value="macos_terminal" disabled><option value="macos_terminal">{t.terminalName}</option></select></label>
    <button type="button" disabled={disabled || !selected || unknown && !retryChecked} onClick={() => void openDirectory()}>{t.open}</button>
    {result && <p role={props.open ? result.status === 'refused' ? 'alert' : 'status' : undefined}>{result.status === 'handoff_accepted' ? t.accepted : result.status === 'refused' ? t.refused : t.unknown} {result.code !== 'accepted' && t[result.code]}</p>}
  </section>;
}
