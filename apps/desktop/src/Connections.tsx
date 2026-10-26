import { useCallback, useEffect, useRef, useState } from 'react';
import { DaemonError } from './api';
import { sessionApi, type NamedConnection } from './session-api';
import { sessionMessages } from './session-i18n';
import type { Language } from './i18n';

export function Connections({ language, connected }: { language: Language; connected: boolean }) {
  const t = sessionMessages[language];
  const [records, setRecords] = useState<NamedConnection[]>([]);
  const [name, setName] = useState('');
  const [executable, setExecutable] = useState('');
  const [argumentsText, setArgumentsText] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const keys = useRef(new Map<string, string>());
  const refresh = useCallback(async () => {
    if (!connected) return;
    try { setRecords(await sessionApi.connectionsList()); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
  }, [connected]);
  useEffect(() => { void refresh(); }, [refresh]);
  const hasPendingProbe = records.some(record => record.probe_state === 'pending');
  useEffect(() => {
    if (!connected || !hasPendingProbe) return;
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    const deadline = Date.now() + 30_000;
    async function tick() {
      await refresh();
      if (disposed) return;
      if (Date.now() < deadline) timer = setTimeout(() => void tick(), 1000);
      else setError(t.probeWait);
    }
    timer = setTimeout(() => void tick(), 1000);
    return () => { disposed = true; clearTimeout(timer); };
  }, [connected, hasPendingProbe, refresh, t.probeWait]);
  async function register(event: React.SubmitEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!connected || busy || !name.trim() || !executable.trim()) return;
    const args = argumentsText.split('\n').map(value => value.trim()).filter(Boolean);
    const fingerprint = JSON.stringify([name.trim(), executable.trim(), args]);
    const key = keys.current.get(fingerprint) ?? crypto.randomUUID();
    keys.current.set(fingerprint, key);
    setBusy(true); setError('');
    try {
      await sessionApi.connectionsRegister(name.trim(), executable.trim(), args, key);
      keys.current.delete(fingerprint);
      setName(''); setExecutable(''); setArgumentsText('');
      await refresh();
    } catch (e) {
      setError((e instanceof Error ? e.message : String(e)) + (e instanceof DaemonError && e.sideEffectState === 'unknown' ? ` ${t.uncertain}` : ''));
    } finally { setBusy(false); }
  }
  return <section aria-labelledby="connections-title">
    <div className="section-heading"><h2 id="connections-title">{t.connections}</h2><button onClick={() => void refresh()} disabled={!connected || busy}>{t.refreshConnections}</button></div>
    {error && <div className="error" role="alert"><strong>{t.error}</strong><p>{error}</p></div>}
    {!connected && <p>{t.unavailable}</p>}
    <form className="connection-form" onSubmit={register}>
      <label htmlFor="connection-name">{t.label}</label><input id="connection-name" value={name} onChange={event => setName(event.target.value)} maxLength={128} disabled={busy} required />
      <label htmlFor="connection-executable">{t.executable}</label><input id="connection-executable" value={executable} onChange={event => setExecutable(event.target.value)} maxLength={4096} disabled={busy} required />
      <label htmlFor="connection-route">{t.route}</label><select id="connection-route" defaultValue="custom_acp" disabled><option value="custom_acp">Custom ACP</option><option disabled>Codex App Server — unsupported</option><option disabled>Qoder native ACP — unverified</option><option disabled>Claude adapter — unverified</option></select>
      <label htmlFor="connection-arguments">{t.arguments}</label><textarea id="connection-arguments" value={argumentsText} onChange={event => setArgumentsText(event.target.value)} disabled={busy} maxLength={8192} rows={3} aria-describedby="arguments-help" /><p id="arguments-help" className="hint">{t.argumentsHelp}</p>
      <p>{t.registerHelp}</p><p>{t.nativeConfirmation}</p><p className="hint">{t.noCredentials}</p><button className="primary" disabled={!connected || busy || !name.trim() || !executable.trim()}>{busy ? t.working : t.register}</button>
    </form>
    <p>{t.noFallback}</p><p className="hint">{t.probeOnly}</p>
    {!records.length && <p>{t.emptyConnections}</p>}
    <ul className="connection-list">{records.map(connection => <li key={connection.id}>
      <h3>{connection.name}</h3><dl><dt>{t.provider}</dt><dd>{connection.provider}</dd><dt>{t.executable}</dt><dd>{connection.executable}</dd><dt>{t.digest}</dt><dd>{connection.executable_sha256}</dd><dt>{t.version}</dt><dd>{connection.version_evidence?.version_line ?? t.unknown}</dd><dt>{t.probeState}</dt><dd>{connection.probe_state}</dd><dt>{t.source}</dt><dd>{connection.version_evidence ? `${connection.version_evidence.source} · ${new Date(connection.version_evidence.observed_at_unix_ms).toISOString()}` : t.unknown}</dd><dt>{t.host}</dt><dd>{connection.host_id}</dd><dt>{t.environment}</dt><dd>{connection.environment_id}</dd><dt>{t.configOwner}</dt><dd>{connection.config_status}</dd><dt>{t.identity}</dt><dd>{connection.identity_ref} ({connection.identity_status})</dd></dl>
      <p className="hint">{t.probeOnly}</p>
    </li>)}</ul>
  </section>;
}
