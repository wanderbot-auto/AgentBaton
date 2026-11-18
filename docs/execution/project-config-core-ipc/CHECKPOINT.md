# 检查点

- status=review
- task base=`7b04514caaf7807b46018547a8b10f2a5e7fc3d6`
- implementation head=`db6cd5f3e77f88b326374aa9ea02cfe7ffb295ea`
- integration target=`84540f293a362c1098c6d6309df9cb1e12cb0dc8`
- tested candidate=`a135ca3ef77c5122fb10b534cf59fab34343aa43`，tree=`3e030d7d853a909b2a8de3c534b74b70599bc677`
- 交付 head 和最终未提交文件清单见本地 `artifacts/project-config-core-ipc/HANDOFF.json`。本检查点之后的交付提交只更新任务证据。
- Apple Silicon 精确候选 34 项通过，覆盖真实 daemon、SQLite、Unix IPC、生成的实际子进程和正式 Factory MCP companion。格式、Clippy、scope 和 Git 冲突预演通过。
- Factory 使用既有 Custom ACP 权限。匹配的原生 Provider 才会加载项目指令。两条真实子任务分别证明旧 v1 和配置前根的绑定语义。
- App/Tauri、真实 Provider 加载、Linux、SHA 以外的自动秘密分类未测或留给后续阶段。项目配置更新由刷新入口触发。
- 测试与准备进程结束，证据目录保留。主工作树和共享入口认领保持原状，等待 leader 审查。
