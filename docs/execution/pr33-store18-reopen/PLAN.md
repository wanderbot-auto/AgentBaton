# AB-REL-007 旧版本恢复验收计划

窗口 C，分支 collab/pr33-store18-reopen，登记基线 ae844ca33406aabd62f44ea73818f9127f211748。ACK OWNER-001。只写本任务文档，使用串行 collab native 运行。

从交付 1026b0df 的真实升级演练恢复文件开始，逐字核对冻结收据、恢复文件和备份的 SHA-256。复用经独立归档的历史提交 a4e03303 构建原件，核对完整构建清单、Git 归档、Store 18 旧 Core/CLI 原码来源及 Mach-O 架构。

在任务短路径和私有 HOME/config/runtime 中，启动旧 Core，走旧 CLI 的只读 snapshot、projects list、tasks list 和 events 0。对照实际 SQLite 的数据和水位，等待 SIGTERM 后正常回收。另用同一演练的 Store 19 中间文件确认旧 Core 拒绝未来 Store。原件留在私有位置，文档只保留摘要及闭合回执。
