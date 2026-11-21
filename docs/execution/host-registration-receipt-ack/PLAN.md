# C2 · Host 登记回执核对与确认

窗口 B 认领 `collab/host-registration-receipt-ack`，基线及预期 feature target `origin/collab/host-registration-integration` 固定在 `a076fc734cffa3f4d90cf5ce45eaf55a2b1c3c28`。启动时工作树干净。以任务卡及 Host entrypoints 持久意图合同为准。

ACK OWNER-001，WP14 在 AB-START-001 获批范围内。已核对本地 `origin/main` 的 OWNER-002，UI 由集成窗口安排。全局 STATUS 及需求进度由集成窗口维护。

1. 复用原回执语义验证。核对只读查询回显键、原登记正文、回执以及预期 Host ID/创建序号。
2. 在日志锁外调用受信任原生 Rust 调用层提供的只读查询，取回锁后比较完整待确认条目。只对完全匹配的已确认条目原子写入合法空日志。
3. 以真实本机私有日志和 SQLite Store 样本覆盖成功、缺失、错误、字段不符、重复确认、有效改写及竞争换代。验证既有日志保持隔离。
4. 通过 collab run 做格式检查。固定源码，重型时段明确移交后运行 Rust targeted、既有日志回归和严格 Core Clippy，收集原始日志/二进制摘要并 join。
5. 本分支提交并核对 scope、diff 和 feature target preview。交 leader 独审、集成。

## 验证命令

在本任务工作树运行，Rust 验证使用 `CARGO_BUILD_JOBS=2` 串行执行。

```sh
bash scripts/collab.sh run -- cargo fmt --all -- --check
bash scripts/collab.sh run -- env CARGO_BUILD_JOBS=2 cargo test --locked -p agentbaton-core --lib client_recovery_journal
bash scripts/collab.sh run -- env CARGO_BUILD_JOBS=2 cargo test --locked -p agentbaton-core --lib registry::host_registry::tests
bash scripts/collab.sh run -- env CARGO_BUILD_JOBS=2 cargo test --locked -p agentbaton-core --lib ipc::host_registry_wire::tests
bash scripts/collab.sh run -- env CARGO_BUILD_JOBS=2 cargo test --locked -p agentbaton-core --lib ipc::frame_tests
bash scripts/collab.sh run -- env CARGO_BUILD_JOBS=2 cargo clippy --locked -p agentbaton-core --all-targets -- -D warnings
bash scripts/collab.sh scope
git diff --check a076fc734cffa3f4d90cf5ce45eaf55a2b1c3c28..HEAD
bash scripts/collab.sh preview origin/collab/host-registration-integration
```
