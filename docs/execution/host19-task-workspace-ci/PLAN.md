# Task Workspace 浏览器与 CI 投影 schema 19

固定基线 9e897d6f6bf0b823e187930fbb84f897be51b89e，分支 collab/host19-task-workspace-ci。已核对 AB-START-001、OWNER-001 至 OWNER-005 和路径认领。共享 workflow 由 leader 管理。

- 将候选清单、实际 Core 与只读 SQLite 绑定到整数 schema 19。保持六个现有场景的运行断言、截图和清理要求。
- 对现行表集合执行精确核验。新增 host_registrations 必须是真实空表，driver 只核对其行是否存在。有限投影只读报告与允许的 PNG。
- 用完整文件摘要逆向还原旧 driver 与投影器，并验证 schema 18 的历史观察。
- 跑现行 Python 定向与全量基础设施。固定源码构建侧车和前端，用独立 Core、SQLite、Chromium 验证。记录私有原件、有限结果、scope 与精确目标合并演算。
