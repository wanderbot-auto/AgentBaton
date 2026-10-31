export const workspaceOpenMessages = {
  en: {
    title: 'Open a workspace in Terminal', source: 'Project source', isolated: 'Isolated workspace', target: 'Directory to open', terminal: 'External terminal', terminalName: 'macOS Terminal',
    refresh: 'Refresh directory choices', open: 'Open selected directory', loading: 'Reading registered directories…', pending: 'Directory handoff pending. Do not repeat it.',
    unknown: 'The handoff outcome is unknown. Terminal may have opened. Check it, then refresh before another explicit action.',
    accepted: 'OS handoff accepted. The terminal’s window and working directory have not been observed.', refused: 'The directory handoff was refused.',
    disconnected: 'Connect to the local daemon before opening a directory.', unavailable: 'Refresh the current workspace view before opening a directory.', noProject: 'A registered project is required.', readFailed: 'Directory choices could not be verified. Refresh before opening.',
    boundary: 'Opens only this directory. It does not start an Agent, transfer a managed session or carry approvals. External terminal startup follows your own terminal settings.',
    recorded: 'Recorded directory; Native checks the current filesystem identity at the click.', unsupported_platform: 'This terminal handoff is available on macOS.', invalid_selection: 'The selection is invalid.', stale_context: 'The project or workspace changed. Refresh its records.', directory_changed: 'The directory identity changed.', native_window_invalid: 'The calling app window is no longer valid.', opener_unavailable: 'The system opener is unavailable.', opener_failed: 'The system opener did not confirm the handoff.', opener_timeout: 'The system opener did not confirm within five seconds.', transport_unknown: 'The handoff reply could not be verified.',
  },
  zh: {
    title: '在终端中打开工作区', source: '项目源目录', isolated: '隔离工作区', target: '要打开的目录', terminal: '外部终端', terminalName: 'macOS 终端',
    refresh: '刷新目录选项', open: '打开所选目录', loading: '正在读取已登记目录…', pending: '目录交接待定，请勿重复操作。',
    unknown: '交接结果未知，终端可能已经打开。请先检查终端，再刷新后明确操作。',
    accepted: '操作系统已接受交接；尚未观察终端窗口或工作目录。', refused: '目录交接被拒绝。',
    disconnected: '请先连接本地守护进程，再打开目录。', unavailable: '请先刷新当前工作区视图，再打开目录。', noProject: '需要已登记的项目。', readFailed: '无法核验目录选项，请先刷新。',
    boundary: '只打开此目录，不启动 Agent、不转移受管会话，也不携带审批。外部终端的启动行为遵循你自己的终端设置。',
    recorded: '这是已登记目录；点击时由 Native 核验当前文件系统身份。', unsupported_platform: '此终端交接仅支持 macOS。', invalid_selection: '所选记录无效。', stale_context: '项目或工作区已变化，请刷新记录。', directory_changed: '目录身份已变化。', native_window_invalid: '调用的应用窗口已失效。', opener_unavailable: '系统打开程序不可用。', opener_failed: '系统打开程序未确认交接。', opener_timeout: '系统打开程序未在五秒内确认。', transport_unknown: '无法核验交接回执。',
  },
} as const;
