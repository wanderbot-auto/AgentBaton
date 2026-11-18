# Task Notification schema 19 交接检查点

- 分支 `collab/host19-task-notification-ci`。固定基线与本次远端核对点均为 `f4b1e5217085fe20c9122b3fae12bff535d24930`。实施和实测固定源码为 `aa9de9a96621cdabfad4097572365ae0693fa682`，交付提交只在此基础上增加本目录记录。
- 完成基线复现、定向历史与现行契约、实际本机 Chromium 私有 Core 19 回放验收、全量基础设施和桌面测试。保留真实通知 4 PNG 和原 Integration 12 PNG 的验证。详见 [EVIDENCE.md](EVIDENCE.md)。
- 保留原 Integration driver、工作流和共享状态路径。Linux 有限投影的本机合成控制通过，真实本机输入被 Linux 维度约束拒绝。托管 Linux 检查与原生系统通知未运行。
- 固定基线及刷新后的 Integration 目标 preview 通过。leader 控制分支核对点 `bd0664adb22cb6799fa2ef4e3952094837298260` 与实现源码 preview 无冲突。由 leader 审查候选并记录共享状态。

## 独立验收与交接

集成负责人已接受 `dd3a7e4b83c57c2871a95d111e25eba6f163a60b`。独立复跑 15 项探针契约和 32 项有限投影及历史契约，逐项核对两件 ARM64 工具、真实 IPC 任务验收与会话停止来源，以及 16 张原始 PNG。历史 AST 固定摘要通过复核。托管 Linux 与原生系统通知仍待验证。

任务分支增加本条验收记录后交回集成负责人。下一集成目标为 `fb8bab79b091c203be4ec09c110a5a1cfc75de8e`，由集成负责人执行合并和记录共享状态。
