# Schema 19 · 默认 Factory 运行时测试

认领基准 `577b922eb6df9f133c14508ffa26aa4b3bbaf290`。白名单为 `crates/core/tests/factory_runtime_ipc.rs`、`crates/core/tests/fixtures/factory_child_goal_runtime.rs` 和此目录。已核对 AB-START-001 与 OWNER-001 / OWNER-002。集成窗口负责共享 workflow、迁移和最终合并。

- 按默认二进制构建与已合入的 freeze prepare/verify 固定真工具，先保留 Data 启动和 Child Goal 退出后的旧 schema 18 失败。
- 让两个现行测试观察真实 SQLite 19 和空 Host 注册，保留旧历史工具、业务断言、Owner/IPC、Linux read gate、结账及原期限。
- 在 Apple Silicon 和 Linux 串行跑适用 Factory 目标。与 C 的时限验收错开，编译低优先级。A 合入后在集成窗口指定的精确候选上复跑默认 workspace，分别记录自然失败、工具回执及 scope/preview。
