# 04 · Provider、具名连接与运行环境

## 1. 接入事实与支持承诺

本章的接口路径来自v0.1资料索引，仅是实现候选，当前没有真实集成测试。正式产品目标为Codex/Qoder/Claude Code；按D06分项建立支持矩阵，自定义ACP按实际能力开放。

| Provider | 候选接入路径 | 当前状态 |
|---|---|---|
| Codex | codex-acp转原生App Server，或直接原生App Server | 需锁版核验与实现 [S05][S06] |
| Qoder | 原生ACP；必要时SDK/headless补充 | 需确认配置/认证传递与原生差异 [S09] |
| Claude Code | claude-agent-acp / 原生SDK支持的路径 | 需核验认证、分发与功能覆盖 [S10] |
| 自定义ACP | manifest＋能力协商与合同测试 | 无全能力兜底承诺 [S03] |

未明确发行方的ZCode不列额外必做驱动，不阻塞正式目标。不要用过往对竞品的未实测描述作为本产品事实。

## 2. 配置模式与执行形态

本节以v0.2两组正交维度替代旧接入等级。

**配置模式**：`baton_managed`为默认，Baton拥有配置与状态命名空间；`native_connected`为高级模式，新受控进程显式使用用户原生配置，Baton不写回外部文件。原生进程本身仍可能写历史/缓存，必须说明。

**执行形态**：`interactive`通过ACP/原生接口流式交互；`batch`通过结构化headless执行单次尝试。两者均由Baton管理，批处理无法处理的审批进入阻塞而不是绕过。

不包含外部终端观察或已有TUI接管。程序可来自managed installation或external executable，与配置模式独立。每个Connection固定provider、installation、environment、identity、Host及支持矩阵；新任务自动路由只从获批连接中选择。

## 3. Provider Adapter 契约

接口是 Baton 内部规范，并非直接声称任何 SDK 有这些同名方法。

| 操作 | 输入/输出 | 必须区分的失败 |
|---|---|---|
| discover_installations | 路径范围 → 可执行路径/版本候选 | 无安装、不可执行、架构不匹配 |
| probe | installation + timeout → SupportManifest | 未认证、协议不兼容、诊断超时 |
| create_session | connection + isolated workspace + config snapshot + policy → native_session_id | 配置漂移、认证缺失、策略不能落实 |
| resume_managed | 已知受管会话标识 → ownership/result | 不支持、busy、不存在、版本或配置不匹配 |
| submit | task/turn + scoped inputs → stream handle | 会话忙、无权限、断连、不确定是否已提交 |
| cancel | attempt/request id → acknowledgement | 仅请求已送达、已停止、无法确认 |
| observe_events | cursor → normalized/raw refs | 缺口、乱序、未知事件 |
| inspect_capabilities | cwd/session → evidence-backed snapshot | 不支持运行态查询、独立诊断替代 |
| propose_config_change | expected hashes + desired fields → ChangeSet | 不可安全写入、外部修改 |
| shutdown | mode → process disposition | 活动任务、不可回收外部进程 |

SupportManifest 每字段使用 `supported / unsupported / unknown / untested`，记录 Provider、版本、OS、协议、检测证据与时间；不以一个全局 supported=true 覆盖所有能力。

## 4. ACP 能力协商

initialize 后记录版本和能力，再创建会话、提交 prompt、消费 update、处理权限、取消；恢复能力只有声明支持才调用。部分版本具有不同恢复语义，按锁定 schema 和 contract tests 实现，不靠猜测。[S03]

客户端暴露文件或 terminal 功能时，需要实现真正的 host-side 方法和边界；不能只宣称 terminal=true 却没有实现。Agent 可能还运行自有工具，因此客户端方法受限不等于全部 Agent 行为受限。

协议消息保留原始类型与扩展字段的引用；UI 只消费归一化 domain events。未知扩展记录为 unsupported detail，不崩溃、不擅自执行、不伪装已理解。

## 5. Codex 路径选择

标准 ACP 作为通用接入，原生 App Server 用于更丰富的 Skills/MCP 和会话信息。[S05] 但不能为查询状态另启动一个 App Server，再把它的连接结果标成原 ACP 会话的运行态。

必须解决连接所有权：由同一个 Provider adapter 管理该 runtime；不能访问同一 runtime 时，查询标记为 `independent_diagnostic`，不能冒充 `session_runtime`。是否采用直接原生驱动或扩展 ACP adapter，依据接口暴露和集成测试形成 ADR，而非两条执行链同时控制一个任务。

## 6. 启动准备与隔离边界

本节废止v0.1的终端观察/包装器路线，替换为运行环境准备。

1. 选择已批准具名连接，解析程序来源与固定版本，不无条件source用户shell。
2. 托管模式生成私有配置/状态路径；原生连接绑定已批准外部来源。目录、启动参数、继承env、项目配置和插件来源必须分别核验。
3. 认证使用厂商允许的登录与凭据接口；不复制或软链接个人认证目录。需本机密钥存储的身份不能因此直接复制到远端。
4. 解析Skills/MCP、项目规则、组织强制策略与任务权限；生成实际配置快照，无法排除的外部来源要报告。
5. 主动连接/安装在预批准范围内进行；缺少必需能力则阻塞或选择获批兼容连接，不静默忽略。
6. 启动、核对同一runtime的实际能力，持久记录绑定关系后执行。

程序独立不等于配置独立，配置目录独立不等于项目/组织来源都被排除，更不等于文件网络沙箱。原生连接只可在满足任务真实要求时使用。

## 7. Session 状态模型

生命周期：`created / connecting / ready / active / idle / disconnected / exited / archived`。

注意状态独立表达：`none / approval / input / auth / conflict / failure / stale / unknown`。

控制权限独立表达：`view / prompt / approve / cancel / resume / close`。一项操作不支持时解释原生原因。

任何 runtime event 需要 session_id、attempt_id（可为空但有原因）、provider_event_id（若原生提供）、timestamp、observation source。时间线不能把两个会话的无标识日志混成一条。

## 8. 身份、恢复与降级

同一安装可有多个账户/认证配置，session 固定到认证上下文引用；不存原始 token。恢复前验证 workspace 和输入是否变更、Provider 版本是否兼容。不能恢复时创建新 attempt，传入已选交接包，不声称延续了完整上下文。

仅有文本日志时允许“有限可观察”但不提供自动结构化审批。取消不支持时提供明确提示与用户控制的进程终止选项，未知副作用进入 reconcile。原生配置连接中的会话仍在Baton内受控交互；外部产生的文件可以作为输入导入，但不得伪造它们的执行来源或验收记录。

## 9. Provider 认证测试

每个正式支持组合覆盖握手、认证失败、prompt、工具事件、审批、拒绝、取消、长输出、断连、恢复、未知字段、配置变化、MCP 注入/拒绝、真实产物。测试清单必须说明哪些使用 mock，哪些使用真实 CLI。不得在未经确认的 Qoder/ZCode 版本上承诺命令参数固定不变。
