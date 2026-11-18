# Data Workspace 浏览器证据适配 schema 19

固定源码基线 `3d19a89f10756966da9f494f928d6962ad63b20b`，分支 `collab/host19-data-workspace-ci`。已核对 AB-START-001、OWNER_INBOX、并行认领和当前实际只读 DB 阻断。

1. 适配现行测试驱动的实测 IPC 与只读 SQLite schema 19 检查，保留数据库事务和业务断言。可选候选清单严格绑定两个现用 Core 工具和已声明来源，旧证据文件保持原样。
2. 保留现行前置 gate、取消和重复键、回执序列、源文件和派生元数据边界。报告记录实际工具、源码、前端、schema 和进程清理。相关定向、桌面 Python 与基础设施全量复测。
3. 使用固定基线的 ARM64 preview-evidence Core、锁定前端构建和已核对 Chromium 运行现行私有流程。另验证可选清单。用实际日志标出无托管 Linux CI 或 native WebView 验收。
4. 只提交认领文件和证据，核对当前目标合并，交 leader 审查。共享 workflow 与后续消费者由集成窗口协调。
