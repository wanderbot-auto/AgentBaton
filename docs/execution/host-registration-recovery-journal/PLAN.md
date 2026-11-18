# C2 · Host 登记恢复日志

窗口 B 认领 `collab/host-registration-recovery-journal`，固定基线及预期 feature target `origin/collab/host-registration-integration` 为 `c9225714319a91dae60e635c0a34ba4b3a4a670d`。启动时工作树干净。以任务卡和 Host entrypoints 的持久意图合同为准。本切片覆盖 WP14 的本地原请求留存。

ACK OWNER-001，按 AB-START-001 执行获批开发。已检查本地 `origin/main` 缓存中的 OWNER-002，其 UI 工作由集成窗口安排。全局 STATUS 和需求账本由集成窗口维护。

1. 在 `client_recovery_journal.rs` 声明 Host 子模块及导出。子模块复用同一组私有日志 I/O，在固定 Host 文件及锁中保存最多一笔 Host intent。
2. 用真实私有目录验证原键与正文重开读回、跨进程竞争、旧日志隔离、失败关闭、完整日志字节边界及最大合法元数据。
3. 通过 `collab.sh run` 做格式检查。固定源码提交，得到 leader 的重型槽交接后运行新增及既有 journal 测试和 Core lint。保存命令、环境、日志和测试二进制摘要。
4. 运行 scope、diff 检查及 feature target preview。提交任务证据并交 leader 独审。回执校验及确认后清理等待下一阶段的 `host_registry.rs` 路径交接。

## 验证命令

以下命令在窗口 B 的任务工作树内顺序运行。冷编译和全目标 lint 使用同一重型时段。

```sh
bash scripts/collab.sh run -- cargo fmt --all -- --check
bash scripts/collab.sh run -- cargo test --locked -p agentbaton-core --lib client_recovery_journal
bash scripts/collab.sh run -- cargo test --locked -p agentbaton-core --lib ipc::frame_tests
bash scripts/collab.sh run -- cargo test --locked -p agentbaton-core --test sessions_ipc lost_task_message_reply_recovers_original_turn_without_dispatching_again -- --exact
bash scripts/collab.sh run -- cargo test --locked -p agentbaton-core --test sessions_ipc desktop_process_exit_recovers_committed_message_without_resending -- --exact
bash scripts/collab.sh run -- cargo clippy --locked -p agentbaton-core --all-targets -- -D warnings
bash scripts/collab.sh scope
git diff --check c9225714319a91dae60e635c0a34ba4b3a4a670d..HEAD
bash scripts/collab.sh preview origin/collab/host-registration-integration
```
