# PR #33 Linux Clippy 修复

Leader 分配本任务，以 PR #33 的 `c7dffe2ce7f3544e41fb4e2d93f816e536448754` 为基线。登记范围为 `crates/core/src/managed_mcps/diagnostics.rs` 和本目录。OWNER-001 与 OWNER-002 已查阅，本任务仅处理 Linux 诊断分支的 Clippy 失败。

- 保留 [原始 Actions Job](https://github.com/wanderbot-auto/AgentBaton/actions/runs/44432204086/job/135252128889) 的失败摘录与完整日志哈希。
- Linux 直接返回原来的短路检查结果，macOS 保留原来的检查顺序与 provenance 条件。
- 用现成 Linux arm64 镜像和 Rust 1.100.0 复现基线，运行 CI 的严格 Clippy 及现有 MCP 诊断测试，再交 Leader 独立审查。
