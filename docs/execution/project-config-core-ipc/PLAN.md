# AB-CFG-009/010 · Core 项目配置

- 基准 `7b04514caaf7807b46018547a8b10f2a5e7fc3d6`，分支 `collab/project-config-core-ipc`。
- 私有 Unix IPC 从登记项目解析配置源。现有 Owner 审批确认首个指令快照，以审阅时固定的文件 SHA 选择范围。
- SQLite 保存发布意图、操作回执和项目当前修订。重启与丢响应时读取原确切快照，任务在创建事务内固定当时已提交版本。
- Session 准入、运行快照登记、临启动验证和恢复均核对该 Task 的固定配置与连接配置。Factory 子代沿现有事务保留的根任务标识读取同一固定配置。
- 后续更新仅接受同一 Owner 批准中明确批准的非秘密指令 SHA。新增范围、未分类字节、MCP、Hook 和无法解消的冲突要求再审。
- 真实 Core/SQLite/Unix IPC、受控本地子进程和原项目回归。桌面入口、真实 Provider 加载和更广秘密分类留给后续阶段。

## Core 接口

- `projects.config.preview` 接受登记 `project_id` 与 `provider`，返回当前修订、只读源清单和候选摘要。Provider 值为 `codex` 或 `claude_code`。
- `projects.config.get` 接受 `project_id`，返回项目配置修订、已发布快照摘要以及待对账操作 ID。
- `projects.config.propose` 接受项目、Provider、`expected_config_revision` 和指令清单。每条选择包含 `key`、`sha256`、`approved_update_sha256`。信封要求登记项目的 `expected_revision` 和幂等键。只有既有可信 `approvals.finalize` 会确认原审阅并开始发布。
- `projects.config.refresh` 接受项目和 `expected_config_revision`，信封同样要求项目修订和幂等键。Core 根据已持久化 Owner 批准与当前实际 SHA 决定是否生成版本，返回 `applied`、`unchanged` 或 `review_required`。
- `projects.config.reconcile` 以项目与 `operation_id` 查回原结果或核验已存在的发布。发生确认记录与文件之间的中断时，重开只检查原文件与准确回执，保留未能确定的状态。
- 每个项目当前保存一个 Provider 的主动配置。Task 固定快照的 SHA 与修订进入已验证运行快照 provenance。Overlay 只用于匹配已批准 Provider，其他独立批准连接沿用自己的权限和配置。现有 Factory 仅接受 Custom ACP，其真实子代继承根 Task 的版本证据。
