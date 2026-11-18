# Provider 认证状态第一阶段计划

- 任务分支为 collab/provider-auth-status，基线 a6a9ba9b4cb11e9531e9b0b9f7f39ef9be9cb505，基线树 3247f8a012a27a902eeac2c27da7cb7eb69d6ba4。
- 改动范围为 crates/providers/src/installation.rs、crates/providers/tests/auth_status.rs 和本目录。由集成窗口负责 Core、IPC、全局台账与后续授权。
- 静态核对固定 Codex 源码和候选接口，区分本机缓存线索与当前身份验证，同时核对启动、网络及持久化副作用。
- 在 Provider 复用有界进程与摘要检查，限定 argv 和允许投影的状态。生产调用缺少可验证能力时返回未知并跳过启动。生成夹具独立测试公开拒绝路径和进程路径。
- 在 Apple Silicon 用锁定依赖运行 Provider 全套、严格 Clippy 和工作区 fmt。保留失败对照，记录准确代码提交和合并预演。
