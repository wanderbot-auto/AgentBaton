# 集成交接

- 目标为受保护的 `origin/main`，基线 `c87a3e83207f2a651d0cbd9da2e3e16608272ad4`。产品源码候选 `576015d7ca99f51d53b3889cd888185a435f18b7`，分支 `collab/frontend-simplification`。后续交付提交只包含验证驱动和证据。
- 移交前按 [CHECKLIST.md](CHECKLIST.md) 检查 41 项。取景在任务 worktree 的 `artifacts/ui-simplification/screenshots/comparison.html`，原始 PNG 分 `before/` 和 `after/`。本地拟真库为独立 SQLite 副本。截图为实际前端和真实 ARM64 daemon 的 Chromium 读写边界测试，断连是本地测试桥断开。
- 已得到职责分开的独立审阅。修复了审查发现的过滤状态口径、历史新旧窗口并发合并、过量增量可继续读取以及待验收导航指向准确范围和运行的问题。无会话任务跳过日志扫描；已有会话且日志很长的查询耗时仍需性能取证。未引入 schema 迁移、新增可写 IPC 或测试控制到交付产品。
- 当前任务停留在独立工作树。全局 `STATUS.md` 由 `leader-control` 持有，未覆盖其工作。PR #37 有书面交接并保留在另一分支，前端改动重叠，整合顺序需由集成窗口确认并重验。
- 已更新 Goal Graph、Factory、Factory 子 Goal、Integration、集成历史和工作区真实 IPC 验收入口。严格测试桥仅增加绑定到夹具任务的只读历史查询。受保护的旧证据源码通过精确版本逆变换复验，继续拒绝未审阅的变动。
- 使用候选 `7dffcd83ec1d9bed24c740fc9f85f5253a508608` 跑通 Apple Silicon 实际 WebView 启动、建任务、限定任务范围的只读历史、非法游标拒绝、进程重启后的持久化和自有子进程清理。独立的一轮 macOS Chromium 工作区验证 6 项均通过，覆盖结果丢失、原键恢复、任务关联切换与权限边界。Linux 浏览器驱动在草稿 PR #39 运行 GitHub 托管验收。
- 只使用本机合成 ACP、隔离数据及现有测试权限。真实 Provider、生产发布和远程主机登记未纳入本轮验证。
- PR #39 的 Linux UI 第二次尝试已通过。macOS 原生后台登记烟测已按设置的新导航修正并在本机复验。托管原生与汇总状态以最新提交的检查为准。
