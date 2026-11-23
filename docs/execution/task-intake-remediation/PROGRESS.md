# 进度

- 审计以 `145a5041e93f82778167de28ac8aebbc4b795cd5` 为基准。Apple Silicon 定向桌面组件测试通过 95 项，实际本地 Core IPC 测试通过 6 项。后端使用生成的本地 ACP，覆盖手动会话、丢回执、Goal 候选、共享预算和全局执行槽。
- 已用 `scripts/collab.sh new` 分别登记整改单和首批实现。原生子代理 `ordinary_dispatch` 负责 START-01。其可写路径为 `crates/core/src/ipc.rs`、`crates/core/src/sessions.rs`、`crates/core/tests/sessions_ipc.rs`、`docs/execution/task-start-ordinary-dispatch/`。整改单与前端各自保有文件归属。
- 已核对 OWNER-001 和 OWNER-002。任务启动整改以任务为主入口，调整后端接线与执行语义所需的交互步骤。样式、布局及前端演进由现有前端窗口处理。
