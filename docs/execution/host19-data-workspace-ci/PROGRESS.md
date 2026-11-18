# 执行记录

- 已从 `3d19a89f10756966da9f494f928d6962ad63b20b` 使用锁定依赖构建 ARM64 `batond`、`baton` 和现行 React 资源。两个 Core 工具使用 `CARGO_PROFILE_DEV_DEBUG=0` 和非默认 `agentbaton-core/preview-evidence`。原驱动配同一 Core 和已缓存的本机 Chromium 启动后，在 `database()` 的 `==18` 断言复现退出 1。
- 新驱动在写入生成数据前校验 `system.snapshot` 和只读 SQLite 均为整数 19。数据库继续使用 `mode=ro`、`query_only=ON` 和读事务。可选清单逐个对照实际 `batond`、`baton` 摘要并检查来源字段。现行无清单分支保持可运行。
- 本地依赖缓存缺少 Playwright Python 1.62.0，原 Chromium 缓存是 1228。已在任务虚拟环境安装 workflow 固定的包版本，Chromium 1234 装入任务自己的浏览器目录。首次缺浏览器的失败、修正后较早成功执行和原驱动红均保留。
- 独审提出清理通过条件后，驱动将浏览器关闭、HTTP 线程 join、daemon wait 0 且无强杀并入 `passed`，原异常不被清理失败覆盖。浏览器路径在唯一 Playwright 会话内解析和核验。最终同一构建的两次私有运行各 8 项通过、0 未运行，且四个不合法清单均在启动 daemon 前拒绝。
- 最终 Data 82 条及基础设施发现 978 条（16 跳过）通过。额外桌面全量 171 条中有 3 个失败与 1 个错误，均在 Notifications/Integration 探针。用固定基线的 `git archive` 原字节复跑该模块 13 条，复现同一失败集合。合同路径内未修改该模块或共享 workflow。
