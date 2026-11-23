# 普通任务启动与首条目标派发

整改单 P1 首批。基准为 origin/main 的 145a5041e93f82778167de28ac8aebbc4b795cd5，目标为受保护的 main。需求范围涉及 AB-TSK-001、AB-TSK-004 和 AB-SES-002 的本地执行切片。已核对 OWNER-001、OWNER-002 和 AB-START-001。负责人为子代理 ordinary_dispatch，独立复核由父代理负责；值班集成窗口管理公共台账和最终合入。

在受限本地 IPC 中新增显式启动入口，引用现成的任务绑定 Git 工作区、具名连接和授权。使用现有会话创建事务固化回执和首轮目标指纹，由监督器在收到受管进程就绪观察后复验当前权限及绑定，并使用既有提交在先的会话消息路径。启动保留独立幂等范围。同键回读启动回执，其他键再次启动同一任务时拒绝。退出、撤销、取消及不明结果沿用已有停止和对账语义。

只认领 crates/core/src/ipc.rs、crates/core/src/sessions.rs、crates/core/tests/sessions_ipc.rs 和本目录。项目默认连接、预算配置、自动准备工作区、桌面入口和 Goal 编排由对应任务接续。前端重构窗口保有自己的路径。

验收使用任务专属 runtime 和构建目录。通过 cargo fmt、针对新入口及旧会话的 Rust 测试，以及 collab scope 和 preview。Unix IPC、SQLite 与本机生成 ACP 进程作为本阶段验证。
