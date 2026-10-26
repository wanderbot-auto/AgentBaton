# 07 · 数据、状态与事件契约

## 1. 实体关系

Host 包含 ProviderInstallation 和 Workspace。Project 关联一个或多个 Workspace。Connection绑定ProviderInstallation、RuntimeEnvironment、IdentityRef和Host；Session绑定Connection、Workspace、执行形态与能力快照。程序与配置来源独立。Task 含多个 Attempt，Attempt 使用 Session 并产出 Artifact。Handoff 连接父子 Task；Validation 引用精确的 Attempt、Artifact 与代码/数据版本。Approval 绑定 actor、action、scope、expiry，不只绑定一个 UI 按钮。

## 2. 核心数据表（逻辑模型，不是已执行迁移）

| 表 | 关键字段与约束 |
|---|---|
| projects | id、name、trust_policy、archived_at |
| workspaces | id、project_id、host_id、canonical_root、git_repo_id、base_revision |
| provider_installations | id、provider、host_id、source managed/external、path、version、binary_digest |
| runtime_environments | id、host_id、owner baton/external、config_root_refs、state_root_refs、config_revision、inheritance_manifest |
| identity_refs | id、provider、host_id、native_secret_handle、account_label，不存明文token |
| connections | id、name、installation_id、environment_id、identity_ref、host_id、support_manifest |
| provider_support | installation_id、capability、support_state、evidence、tested_at |
| sessions | id、native_id、connection_id、workspace_id、execution_mode、config_snapshot_id、lifecycle、control_flags |
| capability_snapshots | id、session/workspace、provider_version、digest、source/evidence |
| config_changes | id、baseline hashes、patch manifest、owner、approval、per-file state |
| tasks | id、goal、root/parent、state、active_attempt、spec_revision |
| attempts | id、task_id、session_id、attempt_number、status、idempotency_key |
| workflow_nodes/edges | workflow_version、task/validator refs、dependency rule |
| artifacts | id、attempt_id、locator、digest/version、media_type、sensitivity、size |
| validations | id、subject refs、validator_version、command_digest、result、evidence |
| approvals | id、actor、scope、policy_revision、expires_at、consumed_at |
| domain_events | sequence、event_id、aggregate/version、kind、payload_ref、source |
| dispatch_intents | command_id、attempt_id、state、idempotency_key、reconcile_state |
| leases | resource、holder_attempt、fencing_token、expiry |
| attention_items | kind、source、task/session、resolved_by、resolved_at |
| integration_records | task_id、candidate_sha、expected_target_head、target_ref、validator_refs、status |
| release_records | artifact/commit、environment_class、target、approval_ref、idempotency_key、status |
| retention_rules | project_id、data_class、ttl、pinned、derived_index_policy |
| host_attempt_journal | host_id、attempt_id、lease_epoch、sequence、ack_cursor、state |

事件与状态更新必须同事务写入；大原始事件可外存但引用完整性需验证。关键字段使用外键/唯一约束。所有时间存 UTC，duration 用单调时钟测量；个人内容可删除，不承诺不可删的合规审计。

## 3. 任务与尝试分离

一个 Task 的目标与验收条件可修订，每次变更 spec_revision。已经执行的 Attempt 固定引用旧 revision，不能修改历史合同。UI 的当前状态是投影，历史事件不得因新 attempt 重置而消失。

重复的客户端提交通过 idempotency key 映射到同一请求结果；相同 key 不同 payload 返回 conflict。去重范围必须包含 caller 和 project，不能全局覆盖别人的请求。

## 4. 状态转移规则

正式状态机应具备穷举测试。`accepted`必须有 acceptance record；`awaiting_acceptance`必须已有必要验证；`validating`不能在缺少产物时假进入成功；`cancelled`需执行器确认停止或明确的进程终止记录；无法确认时是 outcome_unknown。

状态机与权限不混用：某个任务为 running 不代表用户有 cancel 权限；某个会话 idle 不代表有能力恢复已丢失的上下文。

## 5. 事件 envelope

事件带 schema_version、event_id、aggregate_id、sequence、kind、occurred_at、recorded_at、source、attempt_id、payload 与 raw_ref。source 区分 provider runtime、Baton validator、config resolver、remote host、user action和推断。

不能用 self-reported provider 名称做认证；连接身份来自本地授权映射。sequence 是本地持久序，不代表来自多个系统的真实全局因果序。需要 parent/causation id 关联因果。未知原生事件以 opaque 形式存储并标注能力缺口。

## 6. Artifact 与数据一致性

逻辑 artifact_id 与本地路径分离。输入不可变性由 content hash 或受信不可变版本保证；外部可变引用只提供较弱保证，要在验证前重新检查。artifact 指向尚不存在文件时是 planned，不是 delivered。

读取时限定到授权 root、处理 symlink、规范化路径、大小上限与权限。关键检查尽量基于已打开的文件描述符，减少检查后替换的 TOCTOU 风险。媒体预览不执行脚本；二进制、压缩包和 HTML 需不同策略。

## 7. Schema 策略

`specs/schemas/`给出 TaskRequest、TaskResult 和 EventEnvelope 的初稿；`specs/examples/`给出可验证示例。它们是 Baton 自己的领域协议，不是原生 CLI 参数。

schema_version 显式存在。破坏性变更提升主版本；新增字段通过 extension 容器或新版本，严格模型不静默吞掉误拼字段。schema 验证不能代替语义授权、文件存在性、运行态能力或数值正确性检查。

## 8. 配置存储与文件布局

建议本机应用数据目录下存数据库、日志分片、产物索引和私有配置；项目中仅保留用户明确同意的 `.agentbaton/`项目意图文件。不得将账户 token、全局路径或完整私密对话自动提交 Git。

数据导出生成 manifest、版本、选择范围、脱敏报告、缺失引用与校验值。导入先验证 schema 和路径，不自动运行恢复脚本或连接 MCP。

## 9. 迁移、备份与删除

数据库迁移可重复检测、可预览风险、先备份；不对不认识的新版本写库。应用回滚需 schema 兼容策略，不能直接恢复旧二进制后写新数据库。备份/导出包含敏感数据的可能性需清楚提示。

任务删除按 retention policy 清理对话、日志与产物；原仓库/数据默认不删除。保留最小 tombstone 仅在明确策略允许下，否则删除关联索引。磁盘空间统计区分用户项目与 Baton 生成数据。

## 10. v0.2新增设计契约

保留v1任务/结果/事件/分析schema作为固定子执行合同示例；它们不覆盖全部API。新增`connection-profile.v1`、`execution-envelope.v1`、`project-policy.v1`，明确运行连接、解析后执行绑定与项目授权/留存。下发前先把自动路由目标解析为固定Host/Connection，再生成ExecutionEnvelope；不能把abstract provider名作为实际身份。

这些schema仅结构约束。项目外部授权引用必须由服务器验证归属与revision；生产批准绑定精确commit/产物/环境且用后可追踪。自动验收、发布允许、数据接收方以及身份是否兼容均不可只靠客户端JSON字段断言。

清理正文需连带处理对应FTS/向量索引、缓存、导出副本标记与备份到期策略；删除生成正文不删除已采纳代码或独立报告。摘要保留也要遵守敏感项目策略，不保证清除外部原生Provider自己拥有的数据。
