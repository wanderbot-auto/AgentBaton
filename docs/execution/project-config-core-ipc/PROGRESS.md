# 进度

1. 在准确基线 ARM64 上执行真实项目 IPC、登记和策略回归，共 26 项通过。日志和命令见 `artifacts/project-config-core-ipc/baseline.json`。
2. Leader 追加认领 `storage.rs`、`sessions.rs`、`runtime_config.rs`。Factory 后代由事务固定的根 Task 派生配置版本；实现采用该现有关系，未更改 Factory 写路径。
3. 项目配置入口与原 Owner 确认、版本发布、Task 固定和运行验证已接线。第一次编译发现 WorkspaceIdentity 字段名和 SQLite 参数类型错误，原始日志已保留，修正后继续验证。
4. 初轮真实普通 Session 已验证旧无配置 Task、运行中 v1 及随后 v2 Task。重启回执测试保留连接断开方式与目标目录数量假设的两次失败，改为半关闭写端并比较实际文件 inode、字节与完整目录集合。
5. 首轮回归的 Core 单元 468、Session IPC 63、项目 IPC 3、登记 10、策略 13 均通过。旧 Factory 目标缺匹配的工具/夹具前置，原日志保存。后续采用同一精确组合自行构建并冻结工具。
6. Factory 仅准入 Custom ACP，项目指令仅归批准的 Codex/Claude 路由。已把项目版本证据加入验证的运行计划。真实 Factory Session 创建先完成原 lineage/预留验证，再校验版本，Custom ACP 仍使用其批准的连接能力。仅 `Store::create` 和 Factory 委派写入 Task 表。新增两项真实 Factory 根/子回归待跑。
