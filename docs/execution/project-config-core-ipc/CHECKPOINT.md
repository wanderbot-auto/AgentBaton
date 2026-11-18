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

## 补充验收中

- 补充目标 `a6a9ba9b4cb11e9531e9b0b9f7f39ef9be9cb505`。首轮 Mac 35/36，Linux 30/32，均已回收。原件保留在 `artifacts/project-config-core-ipc/supplement`。
- 候选测试版本 `c079dbe2360e9bd5efd3268a7ee77115c7303b73`，仅更改 `crates/core/tests/project_config_ipc.rs` 和进度记录。重新组合 Linux 候选 `99300ad8b481fc99eacba110c0b206876107a193`，树 `5bc88dde4344d01b94f374ac33afd69718101da1`。
- B 完整 workspace 尚运行。A 已完成只读核对、冻结 daemon 测试夹具修复和隔离 checkout 准备。尚未开始新构建或重负载。当前未提交文件为本检查点；候选 stdout、工具清单及原失败事件文件均保留。

## 补充验收交回审查

- status=review。最新功能实现仍为 `db6cd5f3e77f88b326374aa9ea02cfe7ffb295ea`，测试修复 `c079dbe2360e9bd5efd3268a7ee77115c7303b73`，Linux 实测候选 `99300ad8b481fc99eacba110c0b206876107a193`，树 `5bc88dde4344d01b94f374ac33afd69718101da1`。
- 首轮 Mac 全组 35/36。原失败未在同用例的基线与候选单独复测中复现，双方 1/1。修复后 Mac IPC 6/6，Linux ARM64 定向 32/32。
- 已结束所有 A 测试与容器并释放本机时限槽。补充未提交文件与最终交付 HEAD 在 `supplement/HANDOFF.json`。Mac 全组历史失败待 leader 独立判断，Linux 全量 Factory 和桌面/真实 Provider 不属本次实测。
