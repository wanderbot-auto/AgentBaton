import { Fragment, useEffect, useId, useRef, useState } from 'react';
import type { Project } from './api';
import { Connections, ConnectionEvidence } from './Connections';
import { connectionMeetsFloor } from './connection-diagnostics-api';
import { Icon, type IconName } from './Icons';
import { messages, type Language } from './i18n';
import { ManagedMcps } from './ManagedMcps';
import { ManagedSkills } from './ManagedSkills';
import { managedMcpsApi, profileMatchesConnection as mcpMatches, type ManagedMcpProfile } from './managed-mcps-api';
import { managedMcpMessages } from './managed-mcps-i18n';
import { managedSkillsApi, profileMatchesConnection as skillMatches, type ManagedSkillProfile } from './managed-skills-api';
import { managedSkillMessages } from './managed-skills-i18n';
import { managementMessages } from './management-i18n';
import { subscribeRecoveryTerminals } from './recovery-center-api';
import { sessionApi, type ManagedSession, type NamedConnection } from './session-api';
import { sessionMessages } from './session-i18n';
import './management.css';

type Kind = 'connection' | 'skill' | 'mcp';
type Filter = 'all' | Kind;
type Unavailable = { code: string };
type Row = { key: string; connection: NamedConnection } & (
  { kind: 'connection' }
  | { kind: 'skill'; profile: ManagedSkillProfile | Unavailable }
  | { kind: 'mcp'; profile: ManagedMcpProfile | Unavailable }
);
const kinds: readonly Kind[] = ['connection', 'skill', 'mcp'];
const filters: readonly Filter[] = ['all', ...kinds];
const icons: Record<Kind, IconName> = { connection: 'capabilities', skill: 'file', mcp: 'grid' };
const wideDetailQuery = '(min-width:1050px)';

type Copy = typeof managementMessages[Language];
function rowName(row: Row) {
  if (row.kind === 'skill' && 'managed_skill' in row.profile) return row.profile.managed_skill?.name ?? row.connection.name;
  if (row.kind === 'mcp' && 'managed_mcp' in row.profile) return row.profile.managed_mcp?.name ?? row.connection.name;
  return row.connection.name;
}
function needsReview(row: Row) {
  return row.kind === 'connection' ? row.connection.probe_state === 'failed' : 'code' in row.profile && row.profile.code !== 'CAPABILITY_UNSUPPORTED';
}
function rowStatus(row: Row, copy: Copy) {
  if (row.kind === 'connection') {
    return row.connection.probe_state === 'ready' ? copy.versionChecked
      : row.connection.probe_state === 'failed' ? copy.versionFailed : copy.versionPending;
  }
  return 'code' in row.profile
    ? row.profile.code === 'CAPABILITY_UNSUPPORTED' ? copy.unsupported : copy.configurationUnavailable
    : copy.configured;
}

function useCatalog(connected: boolean, revision: number) {
  const [catalog, setCatalog] = useState<{ rows: Row[]; unavailable: boolean } | null>(null);
  const [reading, setReading] = useState(false);
  const floors = useRef(new Map<string, NamedConnection>());

  useEffect(() => {
    if (!connected) {
      setCatalog(null);
      setReading(false);
      return;
    }
    let current = true;
    setReading(true);
    void Promise.allSettled([sessionApi.connectionsList(), managedSkillsApi.list(), managedMcpsApi.list()]).then(([source, skills, mcps]) => {
      if (!current) return;
      const entries = source.status === 'fulfilled' ? source.value : [];
      const verified = source.status === 'fulfilled' && Array.isArray(entries)
        && new Set(entries.map(item => item.id)).size === entries.length
        && entries.every(item => Number.isSafeInteger(item.revision) && item.revision > 0
          && (!floors.current.has(item.id) || connectionMeetsFloor(item, floors.current.get(item.id)!)));
      const connections = new Map(verified ? entries.map(item => [item.id, item] as const) : []);
      for (const record of connections.values()) floors.current.set(record.id, record);
      const rows: Row[] = Array.from(connections.values(), connection => ({ key: `connection:${connection.id}`, kind: 'connection', connection }));
      let unavailable = !verified || skills.status !== 'fulfilled' || !skills.value || mcps.status !== 'fulfilled' || !mcps.value;
      if (skills.status === 'fulfilled' && skills.value) {
        for (const profile of skills.value.profiles) {
          const connection = connections.get(profile.connection_id);
          if (!connection || !skillMatches(profile, connection)) { unavailable = true; continue; }
          if (profile.managed_skill) rows.push({ key: `skill:${connection.id}`, kind: 'skill', connection, profile });
        }
        for (const profile of skills.value.unavailable) {
          const connection = connections.get(profile.connection_id);
          if (connection) rows.push({ key: `skill:${connection.id}`, kind: 'skill', connection, profile });
          else unavailable = true;
        }
      }
      if (mcps.status === 'fulfilled' && mcps.value) {
        for (const profile of mcps.value.profiles) {
          const connection = connections.get(profile.connection_id);
          if (!connection || !mcpMatches(profile, connection)) { unavailable = true; continue; }
          if (profile.managed_mcp) rows.push({ key: `mcp:${connection.id}`, kind: 'mcp', connection, profile });
        }
        for (const profile of mcps.value.unavailable) {
          const connection = connections.get(profile.connection_id);
          if (connection) rows.push({ key: `mcp:${connection.id}`, kind: 'mcp', connection, profile });
          else unavailable = true;
        }
      }
      setCatalog({ rows, unavailable });
      setReading(false);
    }).catch(() => {
      if (current) { setCatalog({ rows: [], unavailable: true }); setReading(false); }
    });
    return () => { current = false; };
  }, [connected, revision]);

  return { catalog: connected ? catalog : null, reading: connected && (reading || catalog === null) };
}

function RecordDetails({ row, language, connected, onRefresh }: { row: Row; language: Language; connected: boolean; onRefresh: () => void }) {
  const m = managementMessages[language];
  if (row.kind === 'connection') return <ConnectionEvidence connection={row.connection} language={language} connected={connected} onRefresh={onRefresh} />;
  return <div className="management-record">
    <dl><dt>{m.status}</dt><dd>{rowStatus(row, m)}</dd><dt>{m.base}</dt><dd>{row.connection.name}</dd>
      {'code' in row.profile ? <><dt>{m.recordedEvidence}</dt><dd><code>{row.profile.code}</code></dd></> : <>
        <dt>{m.source}</dt><dd>{m.managed}</dd><dt>{m.revision}</dt><dd><code>{row.profile.config_revision}</code></dd>
        {row.kind === 'skill' && 'managed_skill' in row.profile && row.profile.managed_skill &&
          <><dt>{managedSkillMessages[language].hash}</dt><dd><code>{row.profile.managed_skill.sha256}</code></dd></>}
        {row.kind === 'mcp' && 'managed_mcp' in row.profile && row.profile.managed_mcp &&
          <><dt>{managedMcpMessages[language].tools}</dt><dd>{row.profile.managed_mcp.enabled_tools.join(', ')}</dd></>}
      </>}
    </dl>
    {'code' in row.profile ? null : <p className="hint">{row.kind === 'skill' ? managedSkillMessages[language].loaded : managedMcpMessages[language].loaded}</p>}
  </div>;
}

type Props = {
  language: Language; connected: boolean; projects: Project[]; projectId: string; sessions: ManagedSession[];
  sessionsFresh: boolean; refreshScope: () => void;
};
export function CapabilitiesPage({ language, connected, projects, projectId, sessions, sessionsFresh, refreshScope }: Props) {
  const m = managementMessages[language], t = messages[language], connectionCopy = sessionMessages[language];
  const id = useId();
  const [revision, setRevision] = useState(0);
  const { catalog, reading } = useCatalog(connected, revision);
  const refresh = () => setRevision(current => current + 1);
  const [filter, setFilter] = useState<Filter>('all');
  const [query, setQuery] = useState('');
  const [scopeOverride, setScopeOverride] = useState<string | null>(null);
  const [attention, setAttention] = useState(false);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [addChoices, setAddChoices] = useState(false);
  const [editor, setEditor] = useState<Kind | null>(null);
  const [opened, setOpened] = useState<Partial<Record<Kind, { base: string; visit: number }>>>({});
  const [wide, setWide] = useState(() => window.matchMedia?.(wideDetailQuery).matches ?? false);
  const rowControls = useRef(new Map<string, HTMLButtonElement>());
  const detailsTitle = useRef<HTMLHeadingElement>(null);
  const addControl = useRef<HTMLButtonElement>(null);
  const pollingDeadline = useRef(0);
  const editorTitle = useRef<HTMLHeadingElement>(null);
  const editorTrigger = useRef<HTMLElement | null>(null);
  const liveProjects = projects.filter(item => !item.archived);
  const requestedScope = scopeOverride ?? projectId;
  const scope = liveProjects.some(item => item.id === requestedScope) ? requestedScope : '';
  const scopedIds = scope && sessionsFresh ? new Set(sessions.filter(session => session.project_id === scope).map(session => session.connection_id)) : null;
  const rows = catalog?.rows ?? [];
  const scoped = scope ? rows.filter(row => scopedIds?.has(row.connection.id)) : rows;
  const term = query.trim().toLocaleLowerCase(language);
  const matches = scoped.filter(row => (filter === 'all' || filter === row.kind)
    && (!term || [rowName(row), row.connection.name, row.connection.provider, row.connection.protocol_route ?? '']
      .some(value => value.toLocaleLowerCase(language).includes(term))));
  const attentionCount = matches.filter(needsReview).length;
  const shown = attention ? matches.filter(needsReview) : matches;
  const selected = shown.find(row => row.key === selectedKey);
  const selectedRecord = selected?.key ?? null;
  const pending = rows.some(row => row.kind === 'connection' && row.connection.probe_state === 'pending');

  useEffect(() => subscribeRecoveryTerminals(status => {
    if (status.state === 'confirmed') setRevision(current => current + 1);
  }), []);
  useEffect(() => {
    const media = window.matchMedia?.(wideDetailQuery);
    if (!media) return;
    const change = () => setWide(media.matches);
    media.addEventListener('change', change);
    return () => media.removeEventListener('change', change);
  }, []);
  useEffect(() => {
    pollingDeadline.current = connected && pending ? Date.now() + 30_000 : 0;
  }, [connected, pending]);
  useEffect(() => {
    if (!connected || !pending || reading || Date.now() >= pollingDeadline.current) return;
    const timer = setTimeout(() => setRevision(current => current + 1), 1500);
    return () => clearTimeout(timer);
  }, [connected, pending, reading, revision]);
  useEffect(() => { if (selectedRecord) detailsTitle.current?.focus({ preventScroll: wide }); }, [selectedRecord, wide]);
  useEffect(() => { if (editor) editorTitle.current?.focus(); }, [editor]);

  function closeDetails() {
    const previous = selectedKey;
    setSelectedKey(null);
    if (previous) rowControls.current.get(previous)?.focus({ preventScroll: true });
  }

  function openEditor(kind: Kind, connectionId = '') {
    setAddChoices(false);
    editorTrigger.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setOpened(current => ({
      ...current,
      [kind]: { base: connectionId || current[kind]?.base || '', visit: (current[kind]?.visit ?? 0) + 1 },
    }));
    setEditor(kind);
  }

  function changeFilter(next: Filter) {
    setFilter(next);
    setAttention(false);
    setSelectedKey(null);
  }

  const addLabel = filter === 'connection' ? connectionCopy.register : filter === 'skill' ? m.editSkill : filter === 'mcp' ? m.editMcp : m.add;
  const details = selected ? <aside id={`${id}-details`} className={`management-inspector${wide ? '' : ' management-inline-record'}`}
    aria-labelledby={`${id}-record-title`} onKeyDown={event => {
      if (event.key === 'Escape') { event.stopPropagation(); closeDetails(); }
    }}>
    <header>
      <h2 id={`${id}-record-title`} tabIndex={-1} ref={detailsTitle}>{rowName(selected)}</h2>
      <button type="button" className="icon-button" aria-label={m.closeDetails} onClick={closeDetails}><Icon name="close" /></button>
    </header>
    <div>
      <RecordDetails row={selected} language={language} connected={connected} onRefresh={refresh} />
      {selected.kind !== 'connection' && !('code' in selected.profile) && (
        <button type="button" disabled={!connected} onClick={() => {
          if (!wide) closeDetails();
          openEditor(selected.kind, selected.connection.id);
        }}>{selected.kind === 'skill' ? m.editSkill : m.editMcp}</button>
      )}
    </div>
  </aside> : null;

  return <section className="management-page" aria-labelledby={`${id}-title`}>
    <header className="management-header"><div><h1 id={`${id}-title`}>{t.capabilities}</h1><p>{m.intro}</p></div>
      <button ref={addControl} type="button" className="primary" disabled={!connected} aria-expanded={filter === 'all' ? addChoices : undefined}
        onClick={() => filter === 'all' ? setAddChoices(!addChoices) : openEditor(filter)}>
        <Icon name="plus" />{addLabel}
      </button>
    </header>
    {addChoices && filter === 'all' && <div className="management-add" role="group" aria-label={m.add}>
      <button onClick={() => openEditor('connection')}>{connectionCopy.register}</button>
      <button onClick={() => openEditor('skill')}>{m.editSkill}</button>
      <button onClick={() => openEditor('mcp')}>{m.editMcp}</button>
    </div>}
    <div className="management-toolbar">
      <div className="management-tabs" role="group" aria-label={t.capabilities}>
        {filters.map(type => <button key={type} type="button" aria-pressed={filter === type} onClick={() => changeFilter(type)}>
          {m[type]}{catalog && <small aria-hidden="true">{type === 'all' ? scoped.length : scoped.filter(row => row.kind === type).length}</small>}
        </button>)}
      </div>
      <div className="management-filters">
        <label className="management-search"><span className="sr-only">{m.search}</span><Icon name="search" />
          <input type="search" value={query} placeholder={m.search} onChange={event => { setSelectedKey(null); setQuery(event.target.value); }} />
        </label>
        {liveProjects.length > 0 && <label><span className="sr-only">{m.scope}</span>
          <select value={scope} onChange={event => { setScopeOverride(event.target.value); changeFilter(filter); }}>
            <option value="">{m.global}</option>
            {liveProjects.map(item => <option key={item.id} value={item.id}>{item.name}</option>)}
          </select>
        </label>}
      </div>
    </div>
    {scope && <p className="management-scope hint">{sessionsFresh ? m.projectScope : m.projectUnavailable}</p>}
    {catalog?.unavailable && <p role="status" className="management-notice">{m.unavailable}</p>}
    {reading && <p role="status" className="management-scope hint">{m.reading}</p>}
    <div className={`management-collection${selected && wide ? ' with-record' : ''}`}>
      <div>
        <div className="management-list">
          {shown.length > 0 && <div className="management-grid management-list-head" aria-hidden="true">
            <span>{m.name}</span><span>{m.status}</span><span>{m.source}</span><span />
          </div>}
          {shown.map(row => <Fragment key={row.key}>
            <button type="button" className="management-grid management-list-row"
              aria-controls={selectedKey === row.key ? `${id}-details` : undefined}
              aria-expanded={selectedKey === row.key}
              ref={node => { if (node) rowControls.current.set(row.key, node); else rowControls.current.delete(row.key); }}
              onClick={() => selectedKey === row.key ? closeDetails() : setSelectedKey(row.key)}>
              <span className="management-row-name"><span className="management-row-icon"><Icon name={icons[row.kind]} /></span><span>
                <strong>{rowName(row)}</strong><small>{row.kind === 'connection' ? row.connection.provider : `${m[row.kind]} · ${row.connection.name}`}</small>
              </span></span>
              <span className={`management-status ${needsReview(row) ? 'warning' : row.kind === 'connection' && row.connection.probe_state === 'ready' ? 'good' : ''}`}>{rowStatus(row, m)}</span>
              <span className="management-source">{row.kind === 'connection' ? row.connection.config_status === 'private_environment_not_sandbox' ? m.privateEnvironment : t.unknown
                : 'code' in row.profile ? t.unknown : m.managed}</span>
              <Icon name="arrow" />
            </button>
            {!wide && selectedKey === row.key && details}
          </Fragment>)}
          {!shown.length && !reading && <div className="management-empty">
            <p>{!connected ? t.disconnected : catalog?.unavailable ? m.unavailable : query || attention ? m.noResults : scope ? m.noProject : rows.length ? m.noCapabilities : m.noConnections}</p>
            {connected && !catalog?.unavailable && !query && !attention && !scope && !rows.length && filter === 'all'
              && <button type="button" onClick={() => openEditor('connection')}>{connectionCopy.register}</button>}
          </div>}
        </div>
        <div className="management-list-foot">
          <span role="status">{catalog && m.results(shown.length)}</span>
          <div>
            {(attentionCount > 0 || attention) && <button type="button" aria-pressed={attention}
              className="management-attention" onClick={() => { setAttention(!attention); setSelectedKey(null); }}>{m.attention(attentionCount)}</button>}
            <button type="button" className="icon-button" title={m.refresh} aria-label={m.refresh}
              disabled={!connected || reading} onClick={() => { refresh(); if (scope) refreshScope(); }}><Icon name="refresh" /></button>
          </div>
        </div>
      </div>
      {wide && details}
    </div>
    {kinds.map(kind => Object.hasOwn(opened, kind) && <section key={kind} className="management-editor" hidden={editor !== kind}>
      <header><h2 ref={editor === kind ? editorTitle : undefined} tabIndex={-1}>{kind === 'connection' ? connectionCopy.register : kind === 'skill' ? m.editSkill : m.editMcp}</h2>
        <button type="button" className="icon-button" aria-label={m.closeDetails} onClick={() => {
          setEditor(null);
          if (editorTrigger.current?.isConnected && !editorTrigger.current.closest('[hidden]')) editorTrigger.current.focus();
          else addControl.current?.focus();
        }}><Icon name="close" /></button></header>
      {kind === 'connection' ? <Connections language={language} connected={connected} registrationOnly onChanged={refresh} />
        : kind === 'skill' ? <ManagedSkills key={opened.skill?.base} language={language} connected={connected} editorOnly initialBaseId={opened.skill?.base}
          editorRevision={opened.skill?.visit} onRegister={() => openEditor('connection')} onChanged={refresh} />
          : <ManagedMcps key={opened.mcp?.base} language={language} connected={connected} editorOnly initialBaseId={opened.mcp?.base}
            editorRevision={opened.mcp?.visit} onRegister={() => openEditor('connection')} onChanged={refresh} />}
    </section>)}
  </section>;
}
