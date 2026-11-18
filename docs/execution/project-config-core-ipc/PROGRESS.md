# 进度

1. 在准确基线 ARM64 上执行真实项目 IPC、登记和策略回归，共 26 项通过。日志和命令见 `artifacts/project-config-core-ipc/baseline.json`。
2. Leader 追加认领 `storage.rs`、`sessions.rs`、`runtime_config.rs`。Factory 后代由事务固定的根 Task 派生配置版本；实现采用该现有关系，未更改 Factory 写路径。
3. 项目配置入口与原 Owner 确认、版本发布、Task 固定和运行验证已接线。第一次编译发现 WorkspaceIdentity 字段名和 SQLite 参数类型错误，原始日志已保留，修正后继续验证。
4. 初轮真实普通 Session 已验证旧无配置 Task、运行中 v1 及随后 v2 Task。重启回执测试保留连接断开方式与目标目录数量假设的两次失败，改为半关闭写端并比较实际文件 inode、字节与完整目录集合。
5. 首轮回归的 Core 单元 468、Session IPC 63、项目 IPC 3、登记 10、策略 13 均通过。旧 Factory 目标缺匹配的工具/夹具前置，原日志保存。后续采用同一精确组合自行构建并冻结工具。
6. Factory 仅准入 Custom ACP，项目指令仅归批准的 Codex/Claude 路由。已把项目版本证据加入验证的运行计划。真实 Factory Session 创建先完成原 lineage/预留验证，再校验版本，Custom ACP 仍使用其批准的连接能力。仅 `Store::create` 和 Factory 委派写入 Task 表。新增两项真实 Factory 根/子回归待跑。

7. 在精确集成候选用本次构建的普通 baton/batond 完成准备、前置校验、真实 IPC 和后置校验。6 项专项、26 项项目/登记/Owner 既有回归以及原 Factory、原 Codex Session 各 1 项通过。专项涵盖两条真实 Factory 父子会话和重启 inode/目录不变断言。
8. 候选格式、Clippy、范围和合并冲突检查完成。全量 Factory 与完整 Core/Session 回归的初轮日志单独保留。本次最终候选未重跑全量组，交 leader 独立审查。

9. 补充候选 `ce179dfd32ad6442921838d1f19e3d1aa049f2ef`（树 `faf1f67d6de2b97fa6e7dd9705990eb213ee28b9`）相对上轮仅改变工作流、Python 检查和文档，完整 Rust 和准备脚本输入逐字一致。Mac Factory 串行全组 35/36，原诊断保留。Linux 第一轮 30/32，两条新 Factory 用例在真实 companion 文件检查前终止。
10. 两条新 factory fixture 改为启动本次同源、已冻结的独立 batond，并使用其相邻的 baton。安全断言、时限和生产源码保持原状。B 正运行完整 workspace，先准备 Mac 同用例的基线/候选隔离对照和 Linux 冻结工具重跑。
