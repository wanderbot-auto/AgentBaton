# Store19 · v2 历史迁移验收

Apple Silicon macOS 26.6.1（Rust 1.91.1）。固定基线 `97803b16276903ffe4b6945966f21adeaad11eeb`，树 `c13081f2a5b8725539dc7477c33937a5920a70e2`。实际测试源码 `4e7e70b04c2ec8150c0b17e9131842c28e02159b`，树 `fbb515030a15e0ccb581e04ed681ac99ee765a9f`。

- 基线两例在原逆向助手返回 `InvalidQuery`，Cargo 101。候选原目标 2/2。两条原 SQL、v2 形状、旧任务断言、`0600` pre-v7 备份和重开次数检查保留。真实升级追加观察 Store19、空 Host 和重开时 pre-v19 原像保持不变。
- 碰撞路径保留原 `projects` 表，校验关闭态主库与 WAL、完整逻辑镜像及 pre-v19 备份。从独立 SQLite 副本仅移除冲突表后，真实迁移达到 Store19。
- 普通工具构建、默认预编、fmt 和指定目标严格 Clippy 全通过。实际冻结 probe 观测 API 1、Store 19、空 Host 和正常回收。准备回执与三次校验都逐字一致。
- 完整默认 workspace 自然退出 0。Cargo 直接启动的 77 个目标含 Doc-tests，共 1,349 项通过。日志共打印 79 份通过摘要，包含两个测试各自启动的子进程，因此逐行累加为 1,351 项通过和 490 项子进程过滤。本轮 `migration_v3` 2/2、Factory 36/36、Integration IPC 34/34、Core 单元 468/468。

[原红、默认 workspace 日志与工具 SHA-256](EVIDENCE.json)
