import { Tabs, useMantineColorScheme, type MantineColorScheme } from '@mantine/core';
import { Disclosure, DisclosureLabel, Select, SelectOption } from './ui';
import { useId, useState } from 'react';
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
  const { colorScheme, setColorScheme } = useMantineColorScheme();
  const m = managementMessages[language], t = messages[language], c = canvasMessages[language];
  const connected = status === 'connected';

  function select(section: SettingsSection) {
    setActive(section);
    if (!visited.has(section)) setVisited(new Set([...visited, section]));
  }

  return <Tabs className="management-settings" value={active} keepMountedMode="display-none"
    onChange={section => { if (section && sections.includes(section as SettingsSection)) select(section as SettingsSection); }}>
    <Tabs.List className="management-tabs" aria-label={m.settingsSections}>
      {sections.map(section => <Tabs.Tab key={section} value={section}>{m[section]}</Tabs.Tab>)}
    </Tabs.List>
    {sections.map(section => <Tabs.Panel key={section} value={section} className="management-settings-pane" hidden={active !== section}>
      {visited.has(section) && (section === 'general' ? <>
        <div className="settings-group">
          <div className="management-setting">
            <label htmlFor={id + '-language'}>{t.language}</label>
            <Select id={id + '-language'} value={language} onChange={selectedValue => onLanguage(selectedValue as Language)}>
              <SelectOption value="en">English</SelectOption>
              <SelectOption value="zh">简体中文</SelectOption>
            </Select>
          </div>
          <div className="management-setting">
            <label htmlFor={id + '-appearance'}>{m.appearance}</label>
            <Select id={id + '-appearance'} value={colorScheme}
              onChange={selectedValue => setColorScheme(selectedValue as MantineColorScheme)}>
              <SelectOption value="light">{m.light}</SelectOption>
              <SelectOption value="dark">{m.dark}</SelectOption>
              <SelectOption value="auto">{m.system}</SelectOption>
            </Select>
          </div>
        </div>
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
          <Disclosure className="management-disclosure"><DisclosureLabel>{c.connectedHelp}</DisclosureLabel><p>{t[status]}</p></Disclosure>
        </>)}
    </Tabs.Panel>)}
  </Tabs>;
}
