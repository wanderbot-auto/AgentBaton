import { useCallback, useEffect, useRef, useState } from 'react';
import { DaemonError } from './api';
import { sessionApi, type NamedConnection, type ProviderChoice } from './session-api';
import { sessionMessages } from './session-i18n';
import {ConnectionDiagnostics} from './ConnectionDiagnostics';
import {connectionDiagnosticMessages} from './connection-diagnostics-i18n';
import {connectionMeetsFloor} from './connection-diagnostics-api';
import type { Language } from './i18n';

export function Connections({ language, connected, compact = false }: { language: Language; connected: boolean; compact?: boolean }) {
  const t = sessionMessages[language];
  const diagnostic = connectionDiagnosticMessages[language];
  const copy = language === 'zh'
    ? { add: '添加连接', evidence: '程序与配置证据', advanced: '启动参数与配置来源', exit: '退出码', probeFailed: '未取得版本输出' }
    : { add: 'Add connection', evidence: 'Program and configuration evidence', advanced: 'Arguments and configuration source', exit: 'Exit code', probeFailed: 'No version output recorded' };
  const shortTime = new Intl.DateTimeFormat(language, { dateStyle: 'medium', timeStyle: 'short' });
  const [records, setRecords] = useState<NamedConnection[]>([]);
  const [name, setName] = useState('');
  const [executable, setExecutable] = useState('');
  const [argumentsText, setArgumentsText] = useState('');
  const [provider, setProvider] = useState<ProviderChoice>('custom_acp');
  const [configurationMode, setConfigurationMode] = useState<'default' | 'claude_restrictions' | 'codex_policy'>('default');
  const [configurationFile, setConfigurationFile] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const keys = useRef(new Map<string, string>());
  const unresolvedImports = useRef(new Set<string>());
  const readSequence=useRef(0),alive=useRef(false),connectionFlag=useRef(connected),floors=useRef(new Map<string,NamedConnection>());
  if(connectionFlag.current!==connected){connectionFlag.current=connected;++readSequence.current;}
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;++readSequence.current;};},[]);
  const refresh = useCallback(async () => {
    if (!connected) return;
    const stamp=++readSequence.current;
    try { const next=await sessionApi.connectionsList();if(!alive.current||!connectionFlag.current||stamp!==readSequence.current)return;
      if(!Array.isArray(next)||new Set(next.map(c=>c.id)).size!==next.length||next.some(c=>!Number.isSafeInteger(c.revision)||c.revision<1||floors.current.has(c.id)&&!connectionMeetsFloor(c,floors.current.get(c.id)!)))throw new Error('Connection records could not be verified');
      for(const c of next)floors.current.set(c.id,c);setRecords(next);
    }
    catch (e) { if(alive.current&&connectionFlag.current&&stamp===readSequence.current)setError(e instanceof Error ? e.message : String(e)); }
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
    const args = provider === 'custom_acp' ? argumentsText.split('\n').map(value => value.trim()).filter(Boolean) : [];
    const source = ((provider === 'claude_code' && configurationMode === 'claude_restrictions') || (provider === 'codex' && configurationMode === 'codex_policy')) ? configurationFile.trim() : undefined;
    if (source !== undefined && !source) return;
    const fingerprint = JSON.stringify([name.trim(), executable.trim(), provider, args, source]);
    const key = keys.current.get(fingerprint) ?? crypto.randomUUID();
    keys.current.set(fingerprint, key);
    setBusy(true); setError('');
    try {
      if (source === undefined) await sessionApi.connectionsRegister(name.trim(), executable.trim(), args, key, provider);
      else await sessionApi.connectionsRegister(name.trim(), executable.trim(), args, key, provider, source);
      keys.current.delete(fingerprint);
      unresolvedImports.current.delete(fingerprint);
      setName(''); setExecutable(''); setArgumentsText(''); setConfigurationFile(''); setConfigurationMode('default');
      await refresh();
    } catch (e) {
      // A known pre-commit rejection can be reviewed as a fresh proposal on the
      // next user action. Unknown/confirmed outcomes retain their exact key.
      if (source !== undefined && (!(e instanceof DaemonError) || ['unknown', 'confirmed'].includes(e.sideEffectState))) unresolvedImports.current.add(fingerprint);
      if (source !== undefined && !unresolvedImports.current.has(fingerprint) && e instanceof DaemonError && e.sideEffectState === 'not_started'
          && ['INPUT_CHANGED', 'PERMISSION_DENIED', 'STALE_REVISION'].includes(e.code)) keys.current.delete(fingerprint);
      setError((e instanceof Error ? e.message : String(e)) + ((e instanceof DaemonError && e.sideEffectState === 'unknown') || unresolvedImports.current.has(fingerprint) ? ` ${t.uncertain}` : ''));
    } finally { setBusy(false); }
  }
  return <section aria-labelledby="connections-title">
    <div className="section-heading"><h2 id="connections-title">{t.connections}</h2><button onClick={() => void refresh()} disabled={!connected || busy}>{t.refreshConnections}</button></div>
    {error && <div className="error" role="alert"><strong>{t.error}</strong><p>{error}</p></div>}
    {!connected && <p>{t.unavailable}</p>}
    <details className="connection-registration secondary-section" open={!compact || undefined}><summary>{compact ? copy.add : t.register}</summary><form className="connection-form" onSubmit={register}>
      <label htmlFor="connection-name">{t.label}</label><input id="connection-name" value={name} onChange={event => setName(event.target.value)} maxLength={128} disabled={busy} required />
      <label htmlFor="connection-executable">{t.executable}</label><input id="connection-executable" value={executable} onChange={event => setExecutable(event.target.value)} maxLength={4096} disabled={busy} required />
      <label htmlFor="connection-route">{t.route}</label><select id="connection-route" value={provider} onChange={event => { setProvider(event.target.value as ProviderChoice); setConfigurationMode('default'); setConfigurationFile(''); }} disabled={busy}><option value="custom_acp">Custom ACP</option><option value="codex">Codex App Server</option><option value="qoder">Qoder ACP</option><option value="claude_code">Claude Agent ACP adapter</option></select>
      <details open={!compact || undefined}><summary>{copy.advanced}</summary>
      <label htmlFor="connection-arguments">{t.arguments}</label><textarea id="connection-arguments" value={provider === 'custom_acp' ? argumentsText : ''} onChange={event => setArgumentsText(event.target.value)} disabled={busy || provider !== 'custom_acp'} maxLength={8192} rows={3} aria-describedby="arguments-help" /><p id="arguments-help" className="hint">{provider === 'custom_acp' ? t.argumentsHelp : t.generatedArguments}</p>
      <label htmlFor="connection-config-source">{t.configSource}</label><select id="connection-config-source" value={configurationMode} disabled={busy} onChange={event => setConfigurationMode(event.target.value as 'default' | 'claude_restrictions' | 'codex_policy')}><option value="default">{t.configDefault}</option>{provider === 'claude_code' && <option value="claude_restrictions">{t.configSelected}</option>}{provider === 'codex' && <option value="codex_policy">{t.codexConfigSelected}</option>}</select>
      {configurationMode !== 'default' && <><label htmlFor="connection-config-file">{provider === 'codex' ? t.codexConfigFile : t.configFile}</label><input id="connection-config-file" value={configurationFile} onChange={event => setConfigurationFile(event.target.value)} maxLength={4096} required disabled={busy} aria-describedby="connection-config-help" /><p id="connection-config-help" className="hint">{provider === 'codex' ? t.codexConfigImportHelp : t.configImportHelp}</p></>}
      <p className="hint">{t.noCredentials}</p>
      </details>
      <p>{t.registerHelp}</p><p>{t.nativeConfirmation}</p><button className="primary" disabled={!connected || busy || !name.trim() || !executable.trim() || (configurationMode !== 'default' && !configurationFile.trim())}>{busy ? t.working : t.register}</button>
    </form></details>
    {!records.length && connected && <p>{t.emptyConnections}</p>}
    <ul className="connection-list">{records.map(connection => {
      const probe = connection.version_evidence;
      const observed = probe ? new Date(probe.observed_at_unix_ms) : null;
      const status = connection.probe_state === 'pending' ? t.connecting : t[connection.probe_state] ?? t.unknown;
      return <li key={connection.id}>
        <div className="section-heading"><strong>{connection.name}</strong><span className="task-state">{status}</span></div>
        <div className="task-facts">
          <span>{diagnostic.historicalProbe}</span>{observed && Number.isFinite(observed.getTime()) && <time dateTime={observed.toISOString()}>{shortTime.format(observed)}</time>}
          {connection.probe_state === 'failed' && <span>{probe?.exit_code != null ? `${copy.exit} ${probe.exit_code}` : copy.probeFailed}</span>}
        </div>
        <ConnectionDiagnostics connection={connection} language={language} connected={connected} onRefresh={() => void refresh()} />
        <details className="connection-evidence" open={!compact || undefined}><summary>{copy.evidence}</summary>
          <dl>
            <dt>{t.provider}</dt><dd>{connection.provider}</dd>
            <dt>{t.executable}</dt><dd><code>{connection.executable}</code></dd>
            <dt>{t.digest}</dt><dd><code>{connection.executable_sha256}</code></dd>
            <dt>{t.version}</dt><dd>{probe?.version_line ?? t.unknown}</dd>
            <dt>{t.probeState}</dt><dd>{probe?.state ?? status}</dd>
            <dt>{t.source}</dt><dd>{probe?.source ?? t.unknown}</dd>
            <dt>{t.host}</dt><dd>{connection.host_id}</dd>
            <dt>{t.environment}</dt><dd>{connection.environment_id}</dd>
            <dt>{t.configOwner}</dt><dd>{connection.config_status}</dd>
            <dt>{t.identity}</dt><dd>{connection.identity_ref} ({connection.identity_status})</dd>
          </dl>
          <p className="hint">{t.probeOnly}</p>
        </details>
      </li>;
    })}</ul>
  </section>;
}
