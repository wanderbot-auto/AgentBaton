# Schema 19 · Factory 工具准备交接

认领基准 `375a054e69d3d411f32266c41b03ba2f6c682460`。产品源码 `02fd3cba1218b7f7882015f5fa4af556a9ac35a3`，源码树 `614dc62a5b7cee647cefcac133355221a76969b3`。两个平台和独立复核使用的身份及原始材料收在 [EVIDENCE.json](EVIDENCE.json)。

## 现行门槛

既有 required workflow CLI 继续使用当前 checkout 预构建的普通 `baton` 和 `batond`。prepare 在私有根读取 schema 19 的 SQLite 文件，并核对实际 RPC 的 API、数据库版本和零序列。原 16 张表、新增 Host 注册表及任务和事件必须为空。唯一拥有的 daemon 必须在 SIGTERM 后正常回收，随后才发布冻结清单。verify 核对当前提交及树、完整探针、工具摘要和私有冻结文件身份。

历史 schema 18 的合成测试以明确的历史输入调用。历史真实工具曾观察到 schema 18 正常运行，并拒绝 schema 19，提交保留该独立原件及校验值。现行 CLI 的候选必须通过 schema 19 门槛。

## 验证范围

Apple Silicon 和 Linux ARM64 本地禁网容器各用精确同源码构建默认 workspace 二进制。各自的 prepare 与 verify 都返回 0，输出逐字一致，25 项定向测试均通过。产品范围只包含准备及校验阶段，GitHub 托管 required check 和业务运行时测试仍由集成窗口处理。

Apple Silicon 的 workspace 命令在成功准备后自然到达 Core，467/468 后退出 101。唯一 Core 失败为 `task_workspace_binding::tests::controlled_v15_shape_migration_retains_rows_backup_and_refuses_future19`，旧夹具期待 18，实际 Store 为 19。Cargo 退出后单独调用原 verify 仍通过，收据逐字一致。
