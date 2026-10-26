# 14 · 内部 API 与自定义委派工具

以下是 AgentBaton 拟议的内部 API，不是已有服务，不是 ACP/MCP 标准自带业务方法。入口为受认证的本地 IPC；桌面端通过受限 Tauri command 转接。远程入口需要独立Host认证，不默认开放本地API。

## 1. 统一请求/响应

请求字段：api_version、request_id、method、params、expected_revision（写操作）、idempotency_key（可能重试的操作）。caller由连接认证确定，不接受params中的自报身份。

响应：request_id、result或error、revision、warnings。错误至少含code、message、retryable、details_ref和side_effect_state。超时不表示操作未执行，查询用同一request/idempotency key确认。

## 2. 方法目录

| 方法 | 主要参数 | 返回/语义 |
|---|---|---|
| projects.import | path/host、授权范围 | project/workspace，默认只读发现 |
| providers.discover | 授权范围、timeout | 候选安装，不自动执行未知脚本 |
| providers.probe | installation_id | 版本、认证状态、支持矩阵 |
| connections.create | installation/environment/identity/host refs | 具名连接、配置归属与支持清单 |
| environments.import | native source refs、approved selection | 托管副本与依赖诊断，不改外部原件 |
| installations.prepare | approved channel/provider/host | 验证来源与兼容的新安装，不影响活跃任务 |
| sessions.create | connection/workspace/config snapshot/policy | 受管session与支持能力 |
| sessions.send | session、typed content、turn key | 接受/拒绝，异步事件，不默认表示已完成 |
| sessions.cancel | session/attempt、expected_revision | cancel_requested与后续确认 |
| sessions.resume | session/attempt | 原生恢复或明确不支持 |
| capabilities.snapshot | provider/workspace/session | 来源链和证据等级 |
| capabilities.compare | snapshot IDs | 差异，不擅自修复 |
| capabilities.diagnose | server/skill、审批引用 | 有副作用的主动检查结果，非运行态冒充 |
| configs.plan | desired fields、baseline hashes | ChangeSet、影响、未知字段 |
| configs.apply | changeset、approval、expected hashes | 每文件结果和重载要求 |
| configs.rollback | change ID、current hashes | 条件补偿，冲突时停止 |
| tasks.create | TaskRequest | Task ID、语义校验结果 |
| tasks.delegate | parent/root绑定、TaskRequest | 子任务句柄、继承后的policy |
| tasks.cancel | task、cascade范围 | 请求取消，不谎报停止 |
| tasks.retry | task、base attempt、批准策略 | 新Attempt，保留历史 |
| tasks.result | task/attempt、访问范围 | result、产物与验证引用 |
| artifacts.open | artifact ID、mode | 受控文件读取/安全预览 |
| validations.run | validator ID、subject refs | 异步验证记录 |
| acceptance.evaluate | task/spec revision、validator refs、preapproval | 规则验收或需决策记录，不自降验收 |
| integration.apply | candidate SHA、expected target head、grant ref | 合入或冲突，不覆盖dirty工作树 |
| releases.request | fixed artifact/commit、target environment、grant | 开发预批准可执行，生产必须单独确认 |
| hosts.register | host identity、directory/capability mapping | 用户Host注册，不创建云资源 |
| routing.resolve | task/inputs/policy revision | 固定Host/Connection及选择依据 |
| remote.reconcile | host/attempt/lease/cursor | 已知状态、事件补发及未知副作用 |
| retention.apply | rule revision、data classes、pins | 日志/索引清理记录，不删除源项目 |
| events.subscribe | cursor、filter、batch limit | 可恢复序列，缺口显式 |
| diagnostics.export | 范围、脱敏选项 | 待用户预览的本地包 |

## 3. 错误分类

`INVALID_REQUEST`、`VERSION_UNSUPPORTED`、`CAPABILITY_UNSUPPORTED`、`AUTH_REQUIRED`、`PERMISSION_DENIED`、`ENFORCEMENT_UNAVAILABLE`、`WORKSPACE_BUSY`、`STALE_REVISION`、`INPUT_CHANGED`、`CONFIG_CONFLICT`、`PROVIDER_DISCONNECTED`、`RATE_LIMITED`、`RESOURCE_LIMIT`、`VALIDATION_FAILED`、`OUTCOME_UNKNOWN`、`INTERNAL_ERROR`。

错误码可程序化处理；message不含秘密。retryable=true仅表示可按策略尝试，不准跳过side_effect_state。OUTCOME_UNKNOWN默认先对账。

## 4. Baton MCP bridge

MCP只是包装同一Task API，不能成为独立高权限后门。使用会话范围授权入口，只开放delegate/status/result/cancel等必要工具，输出大小有限且以产物引用为主。结果/schema变更版本化。

stdio bridge由受管Agent启动时绑定授权；不把长期全局token写入所有CLI配置。HTTP形式若启用，采用明确认证、origin/传输保护和范围校验；本包不指定任何未核验的第三方鉴权实现。

工具调用中提出的 `workspace_id`、`provider`、`network` 等是请求，不是授权。服务端根据parent context校验并返回effective constraints与无法落实项。

## 5. 事件交付

客户端先取得snapshot及sequence，再订阅之后的事件；重复event_id去重；聚合对象revision防乱序覆盖。断线后按cursor补发，超出保留窗口返回需重新snapshot而不是悄悄忽略。

UI控制事件有稳定类型；原始厂商事件保留opaque引用。用户界面只能展示API报告的权限和证据，不从前端按钮状态推断后端已授权。
