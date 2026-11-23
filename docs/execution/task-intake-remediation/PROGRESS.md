# 进度

- 审计以 `145a5041e93f82778167de28ac8aebbc4b795cd5` 为基准。Apple Silicon 定向桌面组件测试通过 95 项，实际本地 Core IPC 测试通过 6 项。后端使用生成的本地 ACP，覆盖手动会话、丢回执、Goal 候选、共享预算和全局执行槽。
- 已用 `scripts/collab.sh new` 分别登记整改单和首批实现。原生子代理 `ordinary_dispatch` 负责 START-01。其可写路径为 `crates/core/src/ipc.rs`、`crates/core/src/sessions.rs`、`crates/core/tests/sessions_ipc.rs`、`docs/execution/task-start-ordinary-dispatch/`。整改单与前端各自保有文件归属。
- 已核对 OWNER-001 和 OWNER-002。任务启动整改以任务为主入口，调整后端接线与执行语义所需的交互步骤。样式、布局及前端演进由现有前端窗口处理。
- START-01 的后端候选为 `234b43866edbeaeaba0c4438d2c479b47b15ba16`。子代理已交接，工作树干净。父代理复核实现和验证边界，并在 Apple Silicon 独立复跑 9 项真实本地 Core IPC 测试，全部通过。严格 Clippy、格式、scope 和提交合并预演通过。
- 子代理首次执行的一项既有 Graph 测试在批准阶段失败。子代理的短临时目录复跑与父代理的独立复跑均通过，同一故障的根因未确定。原结果保留在首批任务证据中。
- 后继桌面接线可引用 Core `tasks.start`。请求使用既有任务、工作区、连接和授权，加上预计任务修订及幂等键。回执包括任务、会话、尝试和原任务规格修订。后续状态从 `sessions.get` 和事件读取。首批仍需预先准备隔离工作区和适用授权。
