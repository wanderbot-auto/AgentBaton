export type IconName = 'grid' | 'task' | 'capabilities' | 'history' | 'search' | 'plus' | 'settings' | 'send' | 'back' | 'close' | 'panel' | 'chat' | 'folder' | 'arrow' | 'refresh' | 'file';
const paths: Record<IconName, string> = {
  grid: 'M3 3h7v7H3zM14 3h7v7h-7zM3 14h7v7H3zM14 14h7v7h-7z',
  task: 'M5 3h14v18H5zM9 8h6M9 12h6M9 16h4',
  capabilities: 'M8 3v5m8-5v5M5 8h14M7 8v5a5 5 0 0 0 10 0V8m-5 10v3',
  history: 'M3 10a9 9 0 1 1 1 7M3 4v6h6m3-4v6l4 2',
  search: 'M16 16l5 5M17 10a7 7 0 1 1-14 0 7 7 0 0 1 14 0',
  plus: 'M12 5v14M5 12h14', settings: 'M4 6h16M4 12h16M4 18h16M8 4v4m8 2v4m-6 2v4',
  send: 'M12 20V4m-6 6 6-6 6 6', back: 'M14 6l-6 6 6 6M8 12h12', close: 'M6 6l12 12M18 6 6 18',
  panel: 'M3 4h18v16H3zM9 4v16', chat: 'M21 11a8 8 0 0 1-8 8H8l-5 3V9a7 7 0 0 1 7-7h3a8 8 0 0 1 8 9',
  folder: 'M3 7V5a2 2 0 0 1 2-2h5l3 4h6a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z',
  arrow: 'M5 12h14m-6-6 6 6-6 6', refresh: 'M20 5v5h-5M4 19v-5h5M5 8a8 8 0 0 1 13-3l2 3M4 16l2 3a8 8 0 0 0 13-3',
  file: 'M13 3H5v18h14V9l-6-6zm0 0v6h6M8 13h8M8 17h5',
};
export function Icon({ name }: { name: IconName }) {
  return <svg className="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d={paths[name]} /></svg>;
}
