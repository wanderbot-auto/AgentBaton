# Task Notification schema 19

固定基线 f4b1e5217085fe20c9122b3fae12bff535d24930，分支 collab/host19-task-notification-ci。已核对 AB-START-001、OWNER-001 至 OWNER-005 和独立路径认领。共享 workflow 与 Integration driver 由 leader 管理。

- 重现现行四项测试失败。通过现有完整字节逆向恢复历史 Integration driver，继续验证原 AST 和权限断言。
- 从实际只读 snapshot 核验整数 schema 19。保持通知来源、排序、可见性、权限、历史、移动布局、固定时限、四张原图和回收守卫。
- 测试现行正负控与全量基础设施，在固定源码上运行真实私有 Core 和本机 Chromium 的正常通知路径，同时验证命令失败路径的隔离。
- 记录固定源码、平台、原件与有限结果。托管 Linux 和原生系统通知分别记录。
