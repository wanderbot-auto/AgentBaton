# Store19 · 默认 Rust workspace 交接

Apple Silicon macOS 26.6.1，Rust 1.91.1。认领基线 `84540f293a362c1098c6d6309df9cb1e12cb0dc8`，树 `b3ea94d0b077d6f2b26a7e49e4613a7997f8c935`。实际测试源码 `31295af9469511ffe74872d9b1fb5f76e8a8552b`，树 `09270a218c6ce269aa348921e44dfbc3b19ad10f`。

- 未改基线按原入口复现七红。最终完整五组 33/33，分别为 `admission_performance` 1、`data_workspaces` 21、`migration_v5` 2、`migration_v6` 2、`workspace_open_ipc` 7。现行三个入口观测实际 Store19 和空 Host，Unix IPC 全表只读比较包括 Host 表。
- 两条历史迁移各保留原逆向 SQL、精确 v4/v5 输入、原任务字节和 pre-v7 备份。正向验证实际 Store19、空 Host 与只读复核的 pre-v19 备份。负向验证关闭态主库与 WAL、完整逻辑镜像、原哨兵和 pre-v19 备份。相同旧库的独立副本仅移除冲突表后成功迁移。前两轮新增断言失败、实际 rusqlite 诊断与空侧车证据均留存。
- 普通 `baton`、`batond` 构建，真实 prepare 和前中后 verify 均退出 0，回执逐字一致。全仓 fmt、workspace 严格 Clippy `--all-targets -- -D warnings`、默认 workspace 预编译均退出 0。
- 默认 workspace 执行原 Mac 入口，687 项通过、1 项失败后自然退出 101，含 Core 单元 468/468。首红 `factory_runtime_ipc::actual_data_factory_wrong_start_revision_is_stale_before_read_worker_or_intent`，该目标 35/36。原错修订请求返回 `STALE_REVISION/not_started`。随后合法启动进入 `provider_session_creation_failed`，真实进程已回收，结算 3220ms，最终 `outcome_unknown` 对预期 `completed`。底层错误未观测到。Cargo 在 Factory 后续目标前停止，五个认领目标的独立结果已完整记录。

[运行命令、回执和 SHA-256 索引](EVIDENCE.json)
