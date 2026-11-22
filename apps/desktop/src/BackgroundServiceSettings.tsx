import { useEffect, useRef, useState, useSyncExternalStore } from 'react';
import type { Language } from './i18n';
import { actionableBackgroundReceipt, backgroundServiceSession, type BackgroundOperation, type BackgroundReceipt } from './background-login-api';
import { backgroundServiceMessages } from './background-login-i18n';
import { managementMessages } from './management-i18n';

type Copy = typeof backgroundServiceMessages.en | typeof backgroundServiceMessages.zh;
function receiptMessage(receipt: BackgroundReceipt, copy: Copy): string | null {
  switch (receipt.code) {
    case 'ok': return null;
    case 'cancelled': return copy.cancelled;
    case 'unsupported_platform': return copy.unsupportedPlatform;
    case 'unsupported_installation': return copy.unsupportedInstallation;
    case 'home_unavailable': case 'home_mismatch': return copy.home;
    case 'custom_runtime': return copy.customRuntime;
    case 'unsafe_path': return copy.unsafe;
    case 'ownership_conflict': case 'content_conflict': return copy.conflictHelp;
    case 'installation_changed': case 'state_changed': return copy.changed;
    case 'busy': return copy.busy;
    case 'native_parent_unavailable': return copy.nativeParent;
    case 'io_failed': return copy.io;
    case 'partial_commit': case 'reconcile_required': return receipt.outcome === 'partial' ? copy.partial : copy.transport;
  }
}

function BackgroundServicePanel({ language, embedded = false }: { language: Language; embedded?: boolean }) {
  const copy = backgroundServiceMessages[language];
  const session = useSyncExternalStore(backgroundServiceSession.subscribe, backgroundServiceSession.getSnapshot);
  const context = useRef<symbol | null>(null);
  const [observation, setObservation] = useState<{ receipt: BackgroundReceipt; generation: number } | null>(null);
  const [failed, setFailed] = useState(false);
  const mounted = useRef(false);
  const switchRef = useRef<HTMLButtonElement>(null);
  const refreshRef = useRef<HTMLButtonElement>(null);
  const [switchBeforeRequest, setSwitchBeforeRequest] = useState<boolean | null>(null);
  const request = async (operation: BackgroundOperation, owner: symbol) => {
    setObservation(null); setFailed(false);
    try {
      const result = await backgroundServiceSession.perform(owner, operation);
      if (mounted.current && context.current === owner) setObservation(result);
    } catch {
      if (mounted.current && context.current === owner) setFailed(true);
    }
  };
  useEffect(() => {
    mounted.current = true;
    const owner = backgroundServiceSession.open();
    context.current = owner;
    let live = true;
    // Defer the initial read so StrictMode's discarded mount cannot issue it.
    queueMicrotask(() => {
      const current = backgroundServiceSession.getSnapshot();
      if (live && !current.needsRefresh && current.pending === null) void request('status', owner);
    });
    return () => {
      live = false; mounted.current = false; context.current = null;
      backgroundServiceSession.close(owner);
    };
  }, []);
  const receipt = observation?.generation === session.generation ? observation.receipt : null;
  const actionable = receipt !== null && actionableBackgroundReceipt(receipt) && !session.needsRefresh;
  const state = actionable ? (receipt.login_registration === 'enabled_next_login' ? copy.enabled : copy.disabled)
    : receipt?.outcome === 'unsupported' ? copy.unavailable
    : receipt?.login_registration === 'conflict' ? copy.conflict
    : session.needsRefresh || failed || receipt !== null ? copy.unknown : copy.unread;
  const message = receipt ? receiptMessage(receipt, copy) : failed ? copy.transport : null;
  const operation = receipt?.login_registration === 'enabled_next_login' ? 'disable' : 'enable';
  const perform = (next: BackgroundOperation) => { const owner = context.current; if (owner !== null) void request(next, owner); };
  const awaitingSwitchReceipt = switchBeforeRequest !== null && !observation && !failed;
  const switchState = actionable ? operation === 'disable' : awaitingSwitchReceipt ? switchBeforeRequest : null;
  useEffect(() => {
    if (session.pending !== null || switchBeforeRequest === null || (!observation && !failed)) return;
    setSwitchBeforeRequest(null);
    const target = switchRef.current ?? refreshRef.current;
    if (document.activeElement === document.body && target && !target.closest('[hidden]')) target.focus();
  }, [session.pending, observation, failed, switchBeforeRequest]);
  if (embedded) return <section aria-label={copy.registration} aria-busy={session.pending !== null}>
    <div className="management-setting">
      <div><h3>{copy.registration}</h3><p className="hint">{managementMessages[language].loginHelp}</p></div>
      {switchState !== null && <button ref={switchRef} type="button" className="management-switch" role="switch" aria-label={copy.registration}
        aria-checked={switchState} aria-disabled={!actionable || session.pending !== null} onClick={() => {
          if (!actionable || session.pending !== null) return;
          setSwitchBeforeRequest(operation === 'disable');
          perform(operation);
        }}>
        <span aria-hidden="true" />
      </button>}
    </div>
    <div className="management-setting-extra">
      <p role="status">{session.pending === 'status' ? copy.loading : session.pending !== null ? copy.working : state}</p>
      {message && <p role="alert">{message}</p>}
      {session.needsRefresh && <p>{copy.refreshFirst}</p>}
      {!actionable && <button ref={refreshRef} type="button" onClick={() => perform('status')} disabled={session.pending !== null}>{copy.refresh}</button>}
      <details className="management-disclosure"><summary>{copy.confirmation}</summary>
        <p>{copy.help}</p><p>{copy.boundary}</p><p>{copy.observation}</p>
        {actionable && <button type="button" onClick={() => perform('status')} disabled={session.pending !== null}>{copy.refresh}</button>}
      </details>
    </div>
  </section>;
  return <section aria-label={copy.registration} aria-busy={session.pending !== null}>
    <p>{copy.help}</p>
    <p role="status">{session.pending === 'status' ? copy.loading : session.pending !== null ? copy.working : state}</p>
    {message && <p role="alert">{message}</p>}
    {session.needsRefresh && <p>{copy.refreshFirst}</p>}
    <div className="dialog-actions">
      <button onClick={() => perform('status')} disabled={session.pending !== null}>{copy.refresh}</button>
      <button onClick={() => perform(operation)} disabled={session.pending !== null || !actionable}>
        {operation === 'enable' ? copy.enable : copy.disable}
      </button>
    </div>
    <p className="hint">{copy.confirmation}</p>
    <p className="hint">{copy.boundary}</p>
    <p className="hint">{copy.observation}</p>
  </section>;
}

export function BackgroundServiceSettings({ language, embedded = false }: { language: Language; embedded?: boolean }) {
  const [expanded, setExpanded] = useState(false);
  if (embedded) return <BackgroundServicePanel language={language} embedded />;
  return <details className="settings-connection" onToggle={event => setExpanded(event.currentTarget.open)}>
    <summary>{backgroundServiceMessages[language].title}</summary>
    {expanded && <BackgroundServicePanel language={language} />}
  </details>;
}
