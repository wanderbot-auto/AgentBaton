# 06 · 任务接力、管道与恢复

## 1. 基本概念

Task 表达用户要的结果；Attempt 是一次实际执行；Session 是 Agent 的交互上下文；Handoff 是一个任务向另一个任务转交的合同与快照；Workflow 是任务依赖关系。不能把这五种对象都叫 session。

只调度通过Baton创建和连接的受管会话，不构建外部Agent观察入口。Baton 是唯一派发入口，不能让每个 Provider adapter 自建递归调度循环。

## 2. 三个入口，一套调度器

自然语言目标由Baton规划器组织、用户手动调整、Agent调用Baton MCP，均转成版本化TaskRequest。首次授权目标/验收与项目策略后，范围内计划变化和接力可自主执行；计划器是可替换编排组件，复用批准Agent/连接，不意味着自造底层Harness或隐式增加付费模型。按身份、project/workspace、授权范围、输入快照、Provider 能力、资源预算校验后排队。

默认路由优先确定性：用户显式选择 > 项目规则 > 模板映射 > 能力匹配候选。不要无依据地固定“Qoder 更擅长统计 / Codex 更擅长代码”；这是用户经验或待测策略，不是产品事实。模型建议的路由可在批准范围内自动采用，先经确定性权限/能力/数据/预算校验；不能自授连接、成本或数据去向。新任务选择主机，活动尝试不得无声迁移。

## 3. 可执行交接合同

必须包含目标、输入版本、相关上下文摘要、排除内容、输出契约、workspace、requested policy、enforcement requirements、验收方法、预算、parent/lineage、idempotency key、回传位置。

只读原始数据与可写输出目录分别授权；`filesystem=read_only`同时要求写报告是自相矛盾的合同，应在提交前拒绝。大输入使用内容引用，产物路径相对于已授权 artifact root；拒绝路径穿越和符号链接逃逸。

schema 只验证结构。是否有权限、输入是否变化、跨 host 是否可读等必须由语义校验器验证。

## 4. 编码接力示例

用户任务：改进数据解析模块。Codex 实现 → Qoder 对固定 patch/revision 审查 → Codex 按审查项修订 → 确定性验证 → 规则自动验收 → 隔离集成候选验证 → 合入授权目标分支 → 按项目授权推送/PR/测试部署；生产发布等待单独确认。

每次修订产生新的 Attempt 和新的代码版本；旧审查/测试不可覆盖新代码。合入前检查目标分支head和工作树，创建隔离集成候选并验证；冲突在预算内自主修复，不可解/会改变授权语义才问。以expected-head比较后更新目标，若变化则重新集成验证。禁止force覆盖、自动stash/reset或绕过分支保护；候选测试不代表变更后目标仍合格。

## 5. 数据分析管道示例

Qoder 读 immutable dataset snapshot 产出 analysis.json、report.md、复现脚本 → Validator 检查 schema/数值范围/脚本退出码 → Codex 使用已验收分析结果实现处理逻辑 → Validator 对固定数据与版本验证。

输入记录 SHA-256 或不可变对象版本、schema、单位、缺失值策略、行数、时区、种子与环境。大文件可用外部不可变版本，不强制每次完整 hash；但必须写出保证等级。抽样分析标 sample，不输出全量结论。算法有效性不能仅由另一个模型赞同来验收。

## 6. 工作流模型

支持串行、parallel fan-out/fan-in、条件分支、审查返工、显式人类批准和汇总节点。依赖图默认无环；返工作为有界 attempt policy 表达，禁止环形无限委派。

节点声明 required_inputs、success/failure outputs、validator、retry class、side_effect class 与 write set。只允许互不冲突的工作集并行；read-only 标签不等于安全证明。资源约束含 host/provider/workspace/global 维度，防止三个不同队列同时超出一个账户的限制。

模板是版本化配置，执行时固定版本，不受后续编辑影响。可视化图是同一模型的编辑/查看界面，不设计第二种不兼容 DSL。

## 7. 动态委派与父任务恢复

Baton MCP 建议提供 `delegate_task`、`get_task_status`、`get_task_result`、`cancel_task`，作为本产品自定义工具，不冒充 ACP 标准方法。

委派请求不能自带可信的 caller_id 或更大 policy；caller 由受管 session 的授权句柄绑定。结果包含 task_id/attempt_id、状态、artifact refs 与 validation refs，而不是任意长度的全对话。

长子任务可以立即返回 task handle，父 Agent 通过 status/result 查询；短任务可在有上限等待内返回。父session等待子任务后，由同一受管连接在原生支持时接续下一turn；无法恢复时创建带明确输入包的新attempt，不能伪装完整上下文续接，也不发送终端按键。

父任务等待子任务时释放相应执行槽，预留子任务容量，检测循环等待，避免所有并发槽被等待者占满。委派深度和总节点数由 root policy 设定，不允许子任务上调。

## 8. 生命周期与验收

Task：`draft → ready → queued → running → validating → accepted`为预批准规则通过时的主路径；仅缺少必需决策时进入`awaiting_acceptance`。Integration与Release另设记录，分别表示合入/推送/测试部署/生产发布，不把accepted直接等同上线。

可以进入 `blocked / failed / cancelled / outcome_unknown`；返工创建新 Attempt 并返回适当执行状态。Attempt 单独记录 created、starting、running、waiting_child/approval、succeeded、failed、cancel_requested、cancelled、outcome_unknown。

`blocked`记录可解决原因；`failed`保留失败证据；`cancel_requested`不是 stopped；`outcome_unknown`不能自动重试外部副作用。验收者可以是用户或事先批准的确定性规则，不能默认 Agent 自己宣布。

## 9. 失败分类与恢复

| 类型 | 处理 |
|---|---|
| 启动前临时失败 | 根据 bounded retry policy 重试 |
| 认证/权限/不支持能力 | 阻塞，等待用户处理，不跳过 |
| 验证失败 | 创建修订任务，保留原结果，次数上限 |
| 输入版本改变 | 使原验证过期，重新计划或批准新快照 |
| 进程崩溃但无副作用 | 查询 native 状态，支持时恢复；否则新 attempt |
| 网络断开且可能已执行外部写入 | outcome_unknown，先查证和对账 |
| 用户取消 | 停止新子任务、发送原生 cancel、超时后按批准策略终止 |
| daemon 崩溃 | 读取 journal/outbox、核对进程身份、回收 lease，禁止重复派发 |

建议实现 at-least-once 可检测派发和幂等键，不宣传 exactly-once。对不支持幂等的工具，Baton 只能做到谨慎对账、人工确认和限制重放。

## 10. 预算与权限

时间、并发、最大尝试、最大子任务、输出空间可以在 Baton 控制范围内限制。token/金额只有 Provider 可靠提供才做准确计量；无法计量时用已知预算维度并明确未知。

请求权限与真实执行权限分离。无法约束网络的本地 CLI 不能显示“网络已禁止”；严格任务因缺少 enforcement 而拒绝启动。允许用户选择 advisory 模式，但必须说明信任前提，不把它当安全等价替代。

## 11. 产物采纳

结果先登记为候选，校验哈希/大小/路径、schema 和确定性检查，再产生 validation。用户打开报告/HTML/链接时按不可信内容处理。代码变更采纳前需权限和冲突检查；数据产物不会自动覆盖原始数据。

外部数据库写入、发布、发信等不因代码回滚而撤销；每类副作用应显式记录可补偿/不可补偿。恢复报告必须写清尚未确认的动作。

## 12. 授权自动化执行矩阵

| 操作 | 允许自动执行的前提 | 必须停下来询问的情形 |
|---|---|---|
| 拆任务、选择已批准连接/主机、并行与有界返工 | 根目标授权、项目策略与剩余预算覆盖，依赖和实际能力满足 | 扩大目标/权限/数据接收方，新增付费或未知成本方式 |
| 验收与合入 | 固定验收/验证器通过，目标分支授权、head未变化、保护规则满足 | 无法解决冲突、需要削弱测试或覆盖用户改动 |
| git push、PR、测试部署 | 具体仓库/分支/环境在项目许可内；动作可对账 | 未授权目的地、未知副作用、实际为生产环境 |
| 生产发布 | 人对固定版本/目标/证据另行确认 | 未确认或确认后产物/目标改变 |
| 超预算 | 保存状态、停止新派发、按策略终止仍执行步骤并对账 | 必须追加预算才能继续 |
| 经验进入生效规则 | 候选经用户或明确批准的审查者确认 | 不得把自动总结直接应用 |

项目预算由任务覆盖但不扩大根任务授权，子任务共享根预算，不能通过子任务数量重置计数。计划修订固定revision，验收变更不能由被验收Agent自批准。全程持续记录可检查事件，不用循环自述替代执行证据。
