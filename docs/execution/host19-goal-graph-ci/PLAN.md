# Goal Graph schema 19 浏览器验证

固定基线 `6c207575c60e55a6e421df7f043daf9941593698`，分支 `collab/host19-goal-graph-ci`。登记范围含 Graph driver、现行定向测试、历史与现行 SQLite 元数据控制、历史字节逆向及其测试、该证据目录。Factory、Goal Execution 与共享工作流由其他窗口维护。

- 确认工作流的候选清单、owner-only 和五个 validation-case。记录实际 schema 19 候选与实库在旧 driver 的拒绝。
- 现行入口固定 schema 19、当前 Git 源和运行时实际 IPC/只读 SQLite。三个旧 Factory 系列的共享导入保留 schema 18 边界。原 driver 完整字节哈希作可逆历史版本。
- 测试候选类型、版本、来源、混配及证据边界。在固定源码编译私有 Core，验收本机 IPC。若 Linux 工具可用，在任务私有容器无网络运行全部现行场景，保留原 procfs、原时限、原断言和真实 cleanup。
- 全量基础设施、定向合同、范围和实际目标 preview 后交独立审查。记录 Linux 本机容器、macOS、托管 CI 证据的差异。
