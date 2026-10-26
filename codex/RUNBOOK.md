# Codex 长程执行规范 · 已获开发授权

## 权威与恢复

读取 AGENTS、infra/execution-gate.json、docs/execution/launch/APPROVAL.md、GOAL、MASTER_PROMPT、最近 CHECKPOINT。AB-START-001 替代历史准备阶段的等待指令；具体许可证/真实 Provider 测试身份/生产发布仍未授权。不得改变产品语义或把设计中允许的远程功能当作实际远程主机凭据。

读取 Git 状态、remote、base/head、已存在分支/PR、CI/产物与未确认的外部操作，再开始任何写入。保存已有改动；新 Cloud 任务从环境快照产生，不假设继承未提交文件。不要 reset、stash 覆盖、强推或绕过保护。

## 执行与需求追踪

1. 用 130 项 baseline 需求与 14 个工作包建立可执行依赖计划。保持基线文件和 ID；在 docs/execution/requirements-progress.json 记录实际状态、实现文件、测试维度、失败/阻塞、证据提交，不改 specified 基线制造完成率。
2. 初始化状态并执行环境基线检查；随后真正建立应用和测试工程。已有 preparation fixture 不是产品。新增产品 CI 时通过 reviewed PR 演进检查器，不能删除仍有效的检查或变更 required-check 名称。
3. 选择依赖 ready 的任务贯穿契约、核心、接口、界面与验证；新任务不反复重读全部设计。发现接口误差按官方文档与实际版本写 ADR。
4. 每个变更运行针对性测试；集成前运行完整适用回归。没有凭据的真实集成单列 blocked，允许先实现适配器与契约测试，不声称已完成真实联通。
5. reviewer 检查代码、测试断言、需求覆盖、安全与 UI 真实接线。主代理处理 review，运行必要测试，再开/更新 PR、读取 CI、遵守 main 保护后合并。
6. 每个闭环继续下一 ready 工作，不要求用户批准每一步。阶段分解用于验证，不缩小总体目标。

## Subagent 协作合同

检测运行时工具而非猜测名称。默认采用少量独立子任务；主代理保持集成权，避免子代理等父代理、父代理又阻塞等待全部子代理的死锁。无工具时使用串行执行和明确自审，不把虚构身份当多代理审查。

子任务输入：需求 ID、基准 SHA、明确目标、允许修改文件/独立 worktree、接口契约、必需测试、禁止事项、可用预算。返回：更改文件和 commit/patch、命令及结果、证据路径、遗留问题、集成注意。不要返回全部原始日志污染主上下文，不允许修改其他人的未提交内容。共享 schema/lockfile/迁移只有一个 writer。停止任务要核对进程与副作用，而不只发 cancel。

## 测试维度与安装包

区分 design_validated、implemented、mock_verified、integration_verified、native_verified、accepted；N/A 有理由，blocked/not_run 不算 passed。产品完备度由需求证据决定，不由模型自述决定。

为每个适用需求登记：工作目录、命令、工具/依赖版本、基准提交、输入 fixture、起止记录、退出码、测试计数、失败日志、artifact 哈希与路径。重点覆盖取消/超时/重启/重连、重复派发、权限扩大、配置竞态、跨账号污染、符号链接/路径穿越、恶意产物、日志清理和数据库升级恢复。

UI 必须连接真实核心；以 fixture 为后端的浏览器测试另行标注。macOS 应用需在真 macOS 上构建和验证 Tauri/IPC/窗口/后台生命周期；两种架构分别有证据。缺少测试环境不能把浏览器或 Linux 通过冒充桌面通过。测试控制端口与测试权限不能进入给用户的生产构建。

开发安装包可作为 GitHub Actions artifact 交付，不触发正式 Release。记录架构、commit、校验值、签名状态与已知限制。尚无 Developer ID 时明确未正式签名；代码签名、公证及公开正式发布需独立授权。最低 macOS 按依赖和实测决定，不自行放弃 Intel。

## 文件与沟通

- docs/execution/STATUS.md：单一主代理维护的总览、当前 PR/SHA、已验证、阻塞、下一步。
- docs/execution/OWNER_INBOX.md：负责人已批准指令和序号；主代理在安全边界检查增量并登记 ACK。
- docs/execution/<work-id>/PLAN.md、PROGRESS.md、CHECKPOINT.md、EVIDENCE.json：每个工作单元自己的记录。
- docs/execution/requirements-progress.json：实时需求覆盖与证据映射，由集成者维护。

在开始、接口确定、集成完成、失败阻塞及任务停止前更新。不要持续无意义提交日志或对 main 直接推送。状态先在活动分支/PR可见，随已审阅 PR 合入。与负责人通信不需要额外研发平台或常驻轮询服务。文件修改不会自动启动/唤醒 Codex；没有活跃任务时需真实用户或获批调度器启动。

## 资源与停止

遵守既有 Cloud/CI 限额，不自动充值或更换 API 计费。CI 保持托管 runner、只读 PR token、无私人凭据、无 privileged PR 触发器；禁止在公开 fork PR 上使用个人 self-hosted runner。现有并发/超时保持有界。子代理不自行继续递归。

没有环境级 Goal 能力时记录事实，使用连续计划循环，不承诺无限运行。子代理与 Goal 不授予新权限。局部受阻时继续其他 ready 项；全部剩余工作受真实外部约束或达到平台限制时保存可恢复检查点并报告，不标全产品完成。

## 核验来源（平台能力需按当前执行入口再次确认）

- https://developers.openai.com/codex/cloud
- https://developers.openai.com/codex/long-running-work
- https://developers.openai.com/codex/agent-configuration/subagents
- https://developers.openai.com/cookbook/examples/codex/using_goals_in_codex
- https://developers.openai.com/blog/run-long-horizon-tasks-with-codex

这些来源说明平台机制，不是本项目已有测试结果。/goal 在支持它的入口使用；网页 Cloud 不因提示词出现 /goal 就必然创建持久目标。
