# 集成交接

- 目标为受保护的 `origin/main`，基线 `c87a3e83207f2a651d0cbd9da2e3e16608272ad4`。产品源码候选 `576015d7ca99f51d53b3889cd888185a435f18b7`，分支 `collab/frontend-simplification`。后续交付提交只包含验证驱动和证据。
- 移交前按 [CHECKLIST.md](CHECKLIST.md) 检查 41 项。取景在任务 worktree 的 `artifacts/ui-simplification/screenshots/comparison.html`，原始 PNG 分 `before/` 和 `after/`。本地拟真库为独立 SQLite 副本。截图为实际前端和真实 ARM64 daemon 的 Chromium 读写边界测试，断连是本地测试桥断开。
- 已得到职责分开的独立审阅。修复了审查发现的过滤状态口径、历史新旧窗口并发合并、过量增量可继续读取以及待验收导航指向准确范围和运行的问题。无会话任务跳过日志扫描；已有会话且日志很长的查询耗时仍需性能取证。未引入 schema 迁移、新增可写 IPC 或测试控制到交付产品。
- 当前任务停留在独立工作树。全局 `STATUS.md` 由 `leader-control` 持有，未覆盖其工作。PR #37 有书面交接并保留在另一分支，前端改动重叠，整合顺序需由集成窗口确认并重验。
- 已知集成阻塞。受历史哈希保护的 `goal-graph-real-ipc.py`（旧范围选择与审阅披露）和 `factory-real-ipc.py`（旧详情入口）仍需分别更新测试驱动与对应的历史逆变换。相关保护分别为 `test_host19_goal_graph_source_fixture.py`、`test_factory_bound_provider_exit.py`。Factory 子工作、集成和原生全套托管 CI 未执行。集成浏览器严格适配器需允许新增只读 `tasks_history` 并更新当前源码的证据逆变换，原有保护与通过标准保留。
- 只使用本机合成 ACP、隔离数据及现有测试权限。真实 Provider、生产发布和远程主机登记未纳入本轮验证。
