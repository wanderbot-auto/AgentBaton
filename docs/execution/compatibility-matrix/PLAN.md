# AB-EXT-004 / AB-REL-009 兼容证据矩阵

在 collab/compatibility-matrix 基于 51b9d762b2acc6ffa0e1c31411bb154ed4326b99 实现只读报告。认领脚本、同名基础设施测试和本目录。Leader 管理集成和全局台账。

- 从产品规格读取 Provider 目标，从 Tauri 配置读取版本与打包声明。当前 macOS 范围遵照 AGENTS 的 ARM64 规则。
- 在本任务目录登记少量实际证据的路径、SHA-256 和确认字段，保留精确源码、版本、架构、测试层级及已知限制。
- 提供 JSON 和 Markdown。显式候选以源码、版本、安装包 SHA-256、系统版本与构建号及 Provider 版本组合查询最低系统待验项。
- 用真实存档和合成正负控验证状态与范围，经 collab run 执行定向与适用的全量基础设施验证，再固定代码与证据交 leader 独审。
