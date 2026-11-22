import { useCallback, useId, useLayoutEffect, useRef, useState, type FormEvent } from 'react';
import { DaemonError } from './api';
import { hostsApi } from './hosts-api';
import { hostMessages, type HostIssue } from './hosts-i18n';
import {
  HostWireError, maxHostMappings, parseHostRegistration, sameHostRegistration,
  type HostIntent, type HostPage, type HostReceipt, type HostRecord, type HostRegistration, type HostSubmission,
} from './hosts-types';
import type { Language } from './i18n';

interface Capture {
  key: string;
  registration: HostRegistration;
  entryId?: string;
  confirmed?: HostSubmission;
  failure?: HostIssue;
  refused?: boolean;
  acknowledgementAttempted?: boolean;
}

interface HostSession {
  capture: Capture | null;
  write: Promise<void> | null;
}

// Owned by App so closing Settings does not discard an unsettled original request.
export function useHostRegistrationSession() {
  return useRef<HostSession>({ capture: null, write: null });
}

type Session = ReturnType<typeof useHostRegistrationSession>;
type Copy = (typeof hostMessages)[Language];
type Phase = 'reading' | 'empty' | 'pending' | 'unresolved' | 'confirmed' | 'blocked' | 'cleared' | 'verifiedClear';
type Problem = HostIssue | 'recheck';
interface Recovery {
  phase: Phase;
  intent: HostIntent | null;
  confirmed: HostSubmission | null;
  captured?: Pick<Capture, 'key' | 'registration'>;
  problem?: Problem;
}

function failure(error: unknown, fallback: HostIssue): HostIssue {
  if (error instanceof HostWireError) {
    if (error.kind === 'unsupported' || error.kind === 'conflict') return error.kind;
    return fallback;
  }
  if (!(error instanceof DaemonError)) return fallback;
  switch (error.code) {
    case 'AUTH_REQUIRED': return 'auth';
    case 'VERSION_UNSUPPORTED':
    case 'CAPABILITY_UNSUPPORTED': return 'unsupported';
    case 'CONFIG_CONFLICT': return 'conflict';
    case 'RESOURCE_LIMIT': return 'limit';
    case 'CLIENT_RECOVERY_REQUIRED': return 'pending';
    case 'CLIENT_RECOVERY_UNAVAILABLE':
    case 'CLIENT_RECOVERY_BUSY': return 'local';
    case 'PROVIDER_DISCONNECTED': return fallback === 'unknown' ? 'unknown' : 'disconnected';
    case 'OUTCOME_UNKNOWN': return 'unknown';
    case 'INVALID_REQUEST': return fallback === 'unknown' ? 'badInput' : 'rejected';
    default: return fallback;
  }
}

function refusedBeforeDispatch(error: unknown): boolean {
  return error instanceof DaemonError && error.sideEffectState === 'not_started' &&
    ['INVALID_REQUEST', 'CLIENT_RECOVERY_UNAVAILABLE', 'CLIENT_RECOVERY_BUSY', 'CLIENT_RECOVERY_REQUIRED',
      'AUTH_REQUIRED', 'VERSION_UNSUPPORTED', 'CAPABILITY_UNSUPPORTED'].includes(error.code);
}

function matchesCapture(capture: Capture, intent: HostIntent): boolean {
  return capture.key === intent.request_key &&
    (capture.entryId === undefined || capture.entryId === intent.entry_id) &&
    sameHostRegistration(capture.registration, intent.registration);
}

function rememberReceipt(session: Session, intent: HostIntent, receipt: HostReceipt): HostSubmission {
  const confirmed = { entry_id: intent.entry_id, receipt };
  const previous = session.current.capture;
  session.current.capture = {
    key: intent.request_key, registration: intent.registration, entryId: intent.entry_id, confirmed,
    acknowledgementAttempted: previous?.entryId === intent.entry_id ? previous.acknowledgementAttempted : undefined,
  };
  return confirmed;
}

function matchesReceipt(record: HostRecord, key: string, receipt: HostReceipt): boolean {
  return record.id === receipt.host_id && record.creation_key === key &&
    record.created_sequence === receipt.created_sequence && record.created_at === receipt.created_at &&
    sameHostRegistration(record.registration, receipt.registration);
}

function Metadata({ registration: r, copy }: { registration: HostRegistration; copy: Copy }) {
  return <dl className="settings-list">
    <dt>{copy.name}</dt><dd>{r.name}</dd>
    <dt>{copy.platform}</dt><dd>{r.declared_platform === 'macos' ? 'macOS' : 'Linux'}</dd>
    <dt>{copy.protocol}</dt><dd>{r.endpoint.expected_host_identity}</dd>
    <dt>{copy.address}</dt><dd>{r.endpoint.address}</dd>
    <dt>{copy.user}</dt><dd>{r.endpoint.user}</dd>
    <dt>{copy.port}</dt><dd>{r.endpoint.port}</dd>
    <dt>{copy.alias}</dt><dd>{r.endpoint.host_key_alias}</dd>
    <dt>{copy.known}</dt><dd>{r.known_hosts.reference}</dd>
    <dt>{copy.checksum}</dt><dd>{r.known_hosts.file_sha256}</dd>
    <dt>{copy.identity}</dt><dd>{r.transport_identity_ref}</dd>
    <dt>{copy.config}</dt><dd>{r.configuration.reference}</dd>
    <dt>{copy.ownership}</dt><dd>{copy[r.configuration.declared_ownership]}</dd>
    <dt>{copy.mappings}</dt><dd>{r.directory_mappings.map(mapping => (
      <div key={mapping.id}>{mapping.id} · {mapping.remote_root}</div>
    ))}{r.directory_mappings.length === 0 && '—'}</dd>
  </dl>;
}

function RegisteredHost({ id, sequence, created, registration, copy }: {
  id: string;
  sequence: number;
  created: string;
  registration: HostRegistration;
  copy: Copy;
}) {
  return <>
    <p>{copy.registered} · {copy.syntax}</p>
    <p>{copy.refs} · {copy.uncontacted} · {copy.capabilities} · {copy.execution}</p>
    <dl className="settings-list">
      <dt>{copy.registryId}</dt><dd>{id}</dd>
      <dt>{copy.sequence}</dt><dd>{sequence}</dd>
      <dt>{copy.created}</dt><dd>{created}</dd>
    </dl>
    <Metadata registration={registration} copy={copy} />
  </>;
}

function HostForm({ copy, disabled, onRegister }: {
  copy: Copy;
  disabled: boolean;
  onRegister: (registration: HostRegistration) => void;
}) {
  const prefix = useId();
  const [mappings, setMappings] = useState<number[]>([]);
  const [invalid, setInvalid] = useState(false);
  const nextMapping = useRef(0);
  const inputs = [
    ['name', copy.name, 256],
    ['protocol', copy.protocol, 64],
    ['address', copy.address, 253],
    ['user', copy.user, 64],
    ['alias', copy.alias, 64],
    ['known', copy.known, 256],
    ['checksum', copy.checksum, 64],
    ['identity', copy.identity, 256],
    ['config', copy.config, 256],
  ] as const;

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (disabled) return;
    const data = new FormData(event.currentTarget);
    let registration: HostRegistration;
    try {
      registration = parseHostRegistration({
        schema_version: 1,
        name: data.get('name'),
        declared_platform: data.get('platform'),
        endpoint: {
          expected_host_identity: data.get('protocol'),
          address: data.get('address'),
          user: data.get('user'),
          port: Number(data.get('port')),
          host_key_alias: data.get('alias'),
        },
        known_hosts: { reference: data.get('known'), file_sha256: data.get('checksum') },
        transport_identity_ref: data.get('identity'),
        configuration: { reference: data.get('config'), declared_ownership: data.get('ownership') },
        directory_mappings: mappings.map(key => ({ id: data.get(`mapping-${key}`), remote_root: data.get(`root-${key}`) })),
      });
    } catch {
      setInvalid(true);
      return;
    }
    setInvalid(false);
    onRegister(registration);
  }

  return <form onSubmit={submit}>
    <fieldset className="connection-form" disabled={disabled}>
      <legend>{copy.register}</legend>
      <label htmlFor={`${prefix}-platform`}>{copy.platform}</label>
      <select id={`${prefix}-platform`} name="platform" defaultValue="" required>
        <option value="">{copy.choice}</option><option value="linux">Linux</option><option value="macos">macOS</option>
      </select>
      {inputs.map(([key, label, maximum]) => <Input key={key} id={`${prefix}-${key}`}
        name={key} label={label} maximum={maximum} describedBy={key === 'checksum' ? `${prefix}-checksum-help` : undefined} />)}
      <p className="hint" id={`${prefix}-checksum-help`}>{copy.checksumHint}</p>
      <label htmlFor={`${prefix}-port`}>{copy.port}</label>
      <input id={`${prefix}-port`} name="port" type="number" min={1} max={65_535} step={1} defaultValue={22} required />
      <label htmlFor={`${prefix}-ownership`}>{copy.ownership}</label>
      <select id={`${prefix}-ownership`} name="ownership" defaultValue="" required>
        <option value="">{copy.choice}</option>
        <option value="baton_managed">{copy.baton_managed}</option>
        <option value="external_read_only">{copy.external_read_only}</option>
      </select>
      {mappings.map((key, index) => <Mapping key={key} id={`${prefix}-mapping-${key}`} name={key} index={index}
        copy={copy} remove={() => setMappings(current => current.filter(item => item !== key))} />)}
      <button type="button" disabled={mappings.length >= maxHostMappings} onClick={() => {
        const key = nextMapping.current++;
        setMappings(current => current.length >= maxHostMappings ? current : [...current, key]);
      }}>{copy.add}</button>
      {invalid && <p role="alert">{copy.issues.badInput}</p>}
      <button type="submit">{copy.register}</button>
    </fieldset>
  </form>;
}

function Input({ id, name, label, maximum, describedBy }: {
  id: string; name: string; label: string; maximum: number; describedBy?: string;
}) {
  return <>
    <label htmlFor={id}>{label}</label>
    <input id={id} name={name} maxLength={maximum} aria-describedby={describedBy} required />
  </>;
}

function Mapping({ id, name, index, copy, remove }: {
  id: string; name: number; index: number; copy: Copy; remove: () => void;
}) {
  return <>
    <Input id={`${id}-id`} name={`mapping-${name}`} label={`${copy.mappingId} ${index + 1}`} maximum={64} />
    <Input id={`${id}-root`} name={`root-${name}`} label={`${copy.root} ${index + 1}`} maximum={512} />
    <button type="button" onClick={remove}>{copy.remove} {index + 1}</button>
  </>;
}

function HostSettingsPanel({ copy, daemonConnected, session, embedded = false }: { copy: Copy; daemonConnected: boolean; session: Session; embedded?: boolean }) {
  const recoveryRead = useRef(0);
  const listRead = useRef(0);
  const detailRead = useRef(0);
  const recoveryBusy = useRef(true);
  const [recovery, setRecovery] = useState<Recovery>({ phase: 'reading', intent: null, confirmed: null });
  const [formVersion, setFormVersion] = useState(0);
  const [inventory, setInventory] = useState<HostPage | null>(null);
  const [listing, setListing] = useState(false);
  const [registryReady, setRegistryReady] = useState(false);
  const [listIssue, setListIssue] = useState<HostIssue | null>(null);
  const [detail, setDetail] = useState<{ record: HostRecord; loading: boolean; failure?: HostIssue } | null>(null);

  const loadInventory = useCallback(async (after: number | null, previous?: HostPage) => {
    if (!daemonConnected) return;
    const token = ++listRead.current;
    setListing(true);
    if (after === null) setRegistryReady(false);
    setListIssue(null);
    try {
      const result = await hostsApi.list(after);
      if (token !== listRead.current) return;
      if (previous) {
        const ids = new Set(previous.hosts.map(host => host.id));
        const keys = new Set(previous.hosts.map(host => host.creation_key));
        if (result.hosts.some(host => ids.has(host.id) || keys.has(host.creation_key))) throw new HostWireError('conflict');
      }
      setInventory(previous ? { ...result, hosts: [...previous.hosts, ...result.hosts] } : result);
      if (after === null) setRegistryReady(true);
    } catch (error) {
      if (token === listRead.current) {
        setRegistryReady(false);
        setListIssue(failure(error, 'badResult'));
      }
    } finally {
      if (token === listRead.current) setListing(false);
    }
  }, [daemonConnected]);

  const loadRecovery = useCallback(async (reconcile: boolean) => {
    const token = ++recoveryRead.current;
    const current = () => token === recoveryRead.current;
    const previousWrite = session.current.write;
    recoveryBusy.current = true;
    const initialCapture = session.current.capture;
    setRecovery(state => ({
      ...state, phase: 'reading', problem: undefined, captured: initialCapture ?? undefined,
      confirmed: initialCapture ? initialCapture.confirmed ?? null : state.confirmed,
    }));
    try {
      let intent = await hostsApi.pending();
      if (!current()) return;
      if (previousWrite) {
        await previousWrite;
        if (!current()) return;
        intent = await hostsApi.pending();
        if (!current()) return;
      }
      const capture = session.current.capture;
      if (!intent) {
        if (capture?.acknowledgementAttempted && capture.confirmed && daemonConnected) {
          const { confirmed } = capture;
          const registered = await hostsApi.get(confirmed.receipt.host_id);
          if (!current()) return;
          if (!matchesReceipt(registered, capture.key, confirmed.receipt)) throw new HostWireError('conflict');
          const stillPending = await hostsApi.pending();
          if (!current()) return;
          if (stillPending !== null || session.current.capture !== capture) throw new HostWireError('conflict');
          session.current.capture = null;
          setRecovery({ intent: null, confirmed, phase: 'verifiedClear' });
          setFormVersion(version => version + 1);
          return;
        }
        if (capture?.refused) session.current.capture = null;
        const remaining = session.current.capture;
        setRecovery(state => ({
          ...state, intent: null, captured: remaining ?? undefined,
          phase: remaining ? 'blocked' : 'empty',
          confirmed: capture?.confirmed ?? state.confirmed,
          problem: remaining ? (remaining.confirmed ? 'recheck' : remaining.failure ?? 'unknown') : capture?.failure,
        }));
        return;
      }
      if (capture && !matchesCapture(capture, intent)) throw new HostWireError('conflict');
      if (capture) capture.entryId = intent.entry_id;
      const confirmed = capture?.confirmed ?? null;
      setRecovery({ intent, confirmed, phase: reconcile && daemonConnected ? 'reading' : confirmed ? 'confirmed' : 'pending',
        problem: reconcile && daemonConnected ? undefined : capture?.failure });
      if (!reconcile || !daemonConnected) return;

      const receipt = await hostsApi.reconcile(intent);
      if (!current()) return;
      if (confirmed && (!receipt || JSON.stringify(confirmed.receipt) !== JSON.stringify(receipt))) {
        throw new HostWireError('conflict');
      }
      setRecovery({
        intent,
        confirmed: receipt ? rememberReceipt(session, intent, receipt) : null,
        phase: receipt ? 'confirmed' : 'unresolved',
      });
    } catch (error) {
      if (!current()) return;
      const captured = session.current.capture;
      setRecovery(state => ({
        ...state, phase: 'blocked', problem: failure(error, 'badResult'),
        captured: captured ?? undefined, confirmed: captured?.confirmed ?? state.confirmed,
      }));
    } finally {
      if (current()) recoveryBusy.current = false;
    }
  }, [daemonConnected, session]);

  const refresh = useCallback(() => {
    void loadRecovery(true);
    void loadInventory(null);
  }, [loadRecovery, loadInventory]);

  useLayoutEffect(() => {
    let open = true;
    recoveryBusy.current = true;
    setRegistryReady(false);
    queueMicrotask(() => {
      if (!open) return;
      setListing(false);
      setDetail(state => state?.loading ? { ...state, loading: false } : state);
      refresh();
    });
    return () => {
      open = false;
      recoveryBusy.current = true;
      ++recoveryRead.current;
      ++listRead.current;
      ++detailRead.current;
    };
  }, [refresh]);

  async function inspect(record: HostRecord) {
    if (!daemonConnected) return;
    const token = ++detailRead.current;
    setDetail({ record, loading: true });
    try {
      const fresh = await hostsApi.get(record.id);
      if (token === detailRead.current) setDetail({ record: fresh, loading: false });
    } catch (error) {
      if (token === detailRead.current) setDetail({ record, loading: false, failure: failure(error, 'badResult') });
    }
  }

  async function write<T>(work: () => Promise<T>, applied: (result: T) => void, fallback: HostIssue = 'unknown') {
    if (!daemonConnected || recoveryBusy.current || session.current.write) return;
    const token = ++recoveryRead.current;
    recoveryBusy.current = true;
    setRecovery(state => ({ ...state, phase: 'reading', problem: undefined }));
    const outcome = Promise.resolve().then(work).then(
      result => ({ ok: true as const, result }),
      error => ({ ok: false as const, error: error as unknown }),
    );
    const joined = outcome.then(() => { session.current.write = null; });
    session.current.write = joined;
    const result = await outcome;
    if (token !== recoveryRead.current) return;
    recoveryBusy.current = false;
    if (result.ok) applied(result.result);
    else setRecovery(state => ({ ...state, phase: 'blocked', problem: failure(result.error, fallback) }));
  }

  function register(registration: HostRegistration) {
    if (!ready || session.current.capture) return;
    void write(async () => {
      const capture: Capture = { key: crypto.randomUUID(), registration };
      session.current.capture = capture;
      try {
        capture.confirmed = await hostsApi.register(registration, capture.key);
        capture.entryId = capture.confirmed.entry_id;
      } catch (error) {
        capture.failure = failure(error, 'unknown');
        capture.refused = refusedBeforeDispatch(error);
      }
      return capture.confirmed;
    }, confirmed => {
      if (confirmed) void loadInventory(null);
      void loadRecovery(false);
    });
  }

  function retry(intent: HostIntent) {
    if (recovery.phase !== 'unresolved') return;
    void write(async () => {
      const confirmed = await hostsApi.retry(intent);
      return rememberReceipt(session, intent, confirmed.receipt);
    }, confirmed => {
      setRecovery({ intent, confirmed, phase: 'confirmed' });
      void loadInventory(null);
    });
  }

  function acknowledge(confirmed: HostSubmission) {
    if (recovery.phase !== 'confirmed' || recovery.intent?.entry_id !== confirmed.entry_id) return;
    const capture = session.current.capture;
    if (!capture || capture.confirmed !== confirmed) return;
    void write(async () => {
      capture.acknowledgementAttempted = true;
      const cleared = await hostsApi.acknowledge(confirmed.entry_id, confirmed.receipt);
      if (cleared && session.current.capture === capture) session.current.capture = null;
      return cleared;
    }, cleared => {
      if (cleared) {
        setRecovery({ intent: null, confirmed, phase: 'cleared' });
        setFormVersion(version => version + 1);
      } else {
        setRecovery(state => ({ ...state, phase: 'blocked', problem: 'recheck' }));
      }
    }, 'local');
  }

  const { phase, intent, confirmed, captured, problem } = recovery;
  const original = captured?.registration ?? intent?.registration;
  const ready = daemonConnected && registryReady && !captured &&
    (phase === 'empty' || phase === 'cleared' || phase === 'verifiedClear');
  return <section aria-label={copy.title}>
    {!daemonConnected && <p role="status">{copy.disconnected}</p>}
    <h3>{copy.inventory}</h3>
    <button type="button" disabled={phase === 'reading' || listing} onClick={refresh}>{copy.refresh}</button>
    {listing && <p role="status">{copy.reading}</p>}
    {listIssue && <p role="alert">{copy.inventoryFailed} {copy.issues[listIssue]}</p>}
    {inventory && (listIssue || !daemonConnected) && <p className="hint">{copy.stale}</p>}
    {inventory?.hosts.length === 0 && !listIssue && daemonConnected && <p>{copy.noHosts}</p>}
    {inventory && <ul className="connection-list">{inventory.hosts.map(record => <li key={record.id}>
      <h4>{record.registration.name}</h4>
      <p>{copy.registered} · {copy.platform} {record.registration.declared_platform === 'macos' ? 'macOS' : 'Linux'}</p>
      <button type="button" disabled={!daemonConnected} onClick={() => void inspect(record)}
        aria-label={`${copy.details} · ${record.registration.name}`}>{copy.details}</button>
      {detail?.record.id === record.id && <>
        {detail.loading && <p role="status">{copy.reading}</p>}
        {detail.failure && <p role="alert">{copy.detailFailed} {copy.issues[detail.failure]}</p>}
        <RegisteredHost id={detail.record.id} sequence={detail.record.created_sequence} created={detail.record.created_at}
          registration={detail.record.registration} copy={copy} />
      </>}
    </li>)}</ul>}
    {inventory?.next_created_sequence != null && <button type="button" disabled={!daemonConnected || listing}
      onClick={() => void loadInventory(inventory.next_created_sequence, inventory)}>{copy.more}</button>}

    {phase === 'reading' && <p role="status">{captured ? copy.working : copy.checking}</p>}
    {problem && <p role="alert">{problem === 'recheck' ? copy.recheck : copy.issues[problem]}</p>}
    {(intent || captured) && <section aria-label={copy.pending}>
      <h3>{copy.pending}</h3>
      <dl className="settings-list"><dt>{copy.key}</dt><dd>{captured?.key ?? intent?.request_key}</dd></dl>
      {!confirmed && original && <Metadata registration={original} copy={copy} />}
      {phase === 'unresolved' && <p role="status">{copy.unresolved}</p>}
      <button type="button" disabled={!daemonConnected || phase === 'reading'} onClick={() => void loadRecovery(true)}>{copy.check}</button>
      {intent && phase === 'unresolved' && <button type="button" disabled={!daemonConnected}
        onClick={() => retry(intent)}>{copy.retry}</button>}
    </section>}
    {confirmed && <section aria-label={copy.registered}>
      <RegisteredHost id={confirmed.receipt.host_id} sequence={confirmed.receipt.created_sequence}
        created={confirmed.receipt.created_at} registration={confirmed.receipt.registration} copy={copy} />
      {phase === 'confirmed' && <button type="button" disabled={!daemonConnected} onClick={() => acknowledge(confirmed)}>{copy.acknowledge}</button>}
      {phase === 'cleared' && <p role="status">{copy.acknowledged}</p>}
      {phase === 'verifiedClear' && <p role="status">{copy.verifiedClear}</p>}
    </section>}
    {embedded ? <details className="management-disclosure">
      <summary>{copy.register}</summary>
      <HostForm key={formVersion} copy={copy} disabled={!ready} onRegister={register} />
    </details> : <HostForm key={formVersion} copy={copy} disabled={!ready} onRegister={register} />}
  </section>;
}

export function Hosts({ language, daemonConnected, session, embedded = false }: { language: Language; daemonConnected: boolean; session: Session; embedded?: boolean }) {
  const [expanded, setExpanded] = useState(false);
  const copy = hostMessages[language];
  if (embedded) return <HostSettingsPanel copy={copy} daemonConnected={daemonConnected} session={session} embedded />;
  return <details className="settings-connection" onToggle={event => setExpanded(event.currentTarget.open)}>
    <summary>{copy.title}</summary>
    {expanded && <HostSettingsPanel copy={copy} daemonConnected={daemonConnected} session={session} />}
  </details>;
}
