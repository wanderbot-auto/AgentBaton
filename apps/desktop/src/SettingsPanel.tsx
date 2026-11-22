import { useId, useState, type KeyboardEvent } from 'react';
import { BackgroundService } from './BackgroundService';
import { BackgroundServiceSettings } from './BackgroundServiceSettings';
import { canvasMessages } from './canvas-i18n';
import { Hosts, useHostRegistrationSession } from './Hosts';
import { messages, type Language } from './i18n';
import { managementMessages } from './management-i18n';
import { RecoveryCenter, useRecoveryCenter } from './RecoveryCenter';

export type SettingsSection = 'general' | 'background' | 'hosts' | 'recovery';
const sections: readonly SettingsSection[] = ['general', 'background', 'hosts', 'recovery'];

type Props = {
  language: Language;
  onLanguage: (language: Language) => void;
  status: 'connecting' | 'connected' | 'disconnected';
  initialSection: SettingsSection;
  recovery: ReturnType<typeof useRecoveryCenter>;
  hostRegistration: ReturnType<typeof useHostRegistrationSession>;
};

export function SettingsPanel({ language, onLanguage, status, initialSection, recovery, hostRegistration }: Props) {
  const [active, setActive] = useState(initialSection);
  const [visited, setVisited] = useState(() => new Set([initialSection]));
  const id = useId();
  const m = managementMessages[language], t = messages[language], c = canvasMessages[language];
  const connected = status === 'connected';

  function select(section: SettingsSection) {
    setActive(section);
    if (!visited.has(section)) setVisited(new Set([...visited, section]));
  }

  function handleKey(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    const next = event.key === 'ArrowRight' ? sections[(index + 1) % sections.length]
      : event.key === 'ArrowLeft' ? sections[(index + sections.length - 1) % sections.length]
      : event.key === 'Home' ? sections[0] : event.key === 'End' ? sections.at(-1) : undefined;
    if (!next) return;
    event.preventDefault();
    select(next);
    document.getElementById(`${id}-tab-${next}`)?.focus();
  }

  return <div className="management-settings">
    <div className="management-tabs" role="tablist" aria-label={m.settingsSections}>
      {sections.map((section, index) => <button key={section} type="button" role="tab"
        id={`${id}-tab-${section}`} aria-controls={`${id}-panel-${section}`} aria-selected={active === section}
        tabIndex={active === section ? 0 : -1} onKeyDown={event => handleKey(event, index)} onClick={() => select(section)}>
        {m[section]}
      </button>)}
    </div>
    {sections.map(section => <div key={section} role="tabpanel" className="management-settings-pane"
      id={`${id}-panel-${section}`} aria-labelledby={`${id}-tab-${section}`} hidden={active !== section}>
      {visited.has(section) && (section === 'general' ? <>
        <div className="management-setting settings-group"><label htmlFor={`${id}-language`}>{t.language}</label>
          <select id={`${id}-language`} value={language} onChange={event => onLanguage(event.target.value as Language)}>
            <option value="en">English</option><option value="zh">简体中文</option>
          </select></div>
      </> : section === 'background' ? <>
        <div className="settings-group">
          <div className="management-setting management-service-heading">
            <strong>{m.backgroundStatus}</strong><span className={`management-status ${connected ? 'good' : ''}`}>{t[status]}</span>
          </div>
          <BackgroundServiceSettings language={language} embedded />
        </div>
        <BackgroundService language={language} connected={connected} embedded />
      </> : section === 'hosts' ? <Hosts language={language} daemonConnected={connected} session={hostRegistration} embedded />
        : <>
          <RecoveryCenter language={language} controller={recovery} embedded />
          <details className="management-disclosure"><summary>{c.connectedHelp}</summary><p>{t[status]}</p></details>
        </>)}
    </div>)}
  </div>;
}
