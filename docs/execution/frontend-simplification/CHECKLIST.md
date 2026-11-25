# 前端精简审计验收

逐项复核 `artifacts/ui-audit/agentbaton-ui-audit.html`「问题清单」的删并、默认首屏、高级与详细内容。原始截图基线为 `c87a3e83`。离线 Chromium 逐项打开 41 项，优先 29、局部 11、基本克制 1。

取景与组件覆盖见 [EVIDENCE.json](EVIDENCE.json)。组件测试证明交互与传输契约，截图记录实际构建连接隔离 Core 的显示，历史原生取景单列。

| # | 清单界面 | 处理 | 首屏与细节 | 代码 | 验证 |
|---:|---|---|---|---|---|
| 01 | 空间首页 · 双列任务 · P1 | 已修 | 保留两列。卡片仅保留标题、状态和已记录的 Agent 进展。删去空泛备注及未派发副标题。 | [App.tsx:435](../../../apps/desktop/src/App.tsx#L435) | home |
| 02 | 紧凑窗口 · 900 px · P2 | 已修 | 去掉常驻快捷键和重复项目栏。保留按钮提示。900 px 双列，窄窗口切换单列。 | [style.css:95](../../../apps/desktop/src/style.css#L95) | compact-zh、compact-en、narrow |
| 03 | 项目筛选 · 空项目 · P2 | 已修 | 空项目只有一句原因和任务输入区。筛选无结果给清除动作。 | [App.tsx:441](../../../apps/desktop/src/App.tsx#L441) | empty |
| 04 | 任务搜索 · P3 | 已复验并微调 | 保留单行搜索结果，补短项目名和回车打开。Dialog 焦点和 Escape 由组件回归覆盖。 | [App.tsx:578](../../../apps/desktop/src/App.tsx#L578) | 组件测试 |
| 05 | 后台断开 · P1 | 已修 | 断开只显示一个状态和重连。缓存标注，错误原文收起；首次断开不显示空任务或创建表单。 | [App.tsx:419](../../../apps/desktop/src/App.tsx#L419) | disconnected、cold-disconnected |
| 06 | 任务详情 · 草稿 · P1 | 已修 | 标题即目标。只保留配置执行入口，未派发时隐藏输入框。 | [App.tsx:452](../../../apps/desktop/src/App.tsx#L452) | draft |
| 07 | 任务对话 · 历史消息缺失 · P1 | 已修 | 任务关联会话按序号分页，旧消息独立于全局事件窗口。连续回复合并，排程参数原文收起。 | [task-thread-history.ts:33](../../../apps/desktop/src/task-thread-history.ts#L33) | task-thread、scoped IPC、Core 回归 |
| 08 | 结果未知 · 重启恢复 · P1 | 已修 | 重启后显示结果未知、原因及核对会话动作，暂停发送。证据层保留原始状态和记录。 | [App.tsx:459](../../../apps/desktop/src/App.tsx#L459) | unknown |
| 09 | 任务详情 · 元数据与工具 · P1 | 已修 | 任务详情先显示项目、Agent、工作区和计划。身份标识、修订及交付工具放入折叠。 | [App.tsx:507](../../../apps/desktop/src/App.tsx#L507) | 组件测试 |
| 10 | 执行配置与授权 · P1 | 已修 | 具名项目和任务只读显示。工作区和 Agent 优先，限制折叠；审阅保留完整接收方、写入根、主机和权限确认。 | [ManagedSessions.tsx:286](../../../apps/desktop/src/ManagedSessions.tsx#L286) | execution、授权合同测试 |
| 11 | 快捷对话 · P1 | 已修 | 先选会话并读取消息。精简会话标识和默认说明。选摘和手写摘要从明确操作进入。 | [QuickConversation.tsx:64](../../../apps/desktop/src/QuickConversation.tsx#L64) | quick-chat、组件测试 |
| 12 | 独立 Agent 对话 · P2 | 已修 | 独立对话入口区分新建动作，默认显示已存会话。展开新建后才选择范围。 | [StandaloneConversations.tsx:101](../../../apps/desktop/src/StandaloneConversations.tsx#L101) | standalone |
| 13 | 独立对话 · 长历史 · P1 | 已修 | 按回合显示角色、当地时间和消息。保留较早页入口，复选框进入选摘后出现。完整事件标识和原文折叠。 | [ConversationHistory.tsx:60](../../../apps/desktop/src/ConversationHistory.tsx#L60) | long-history、long-history-earlier |
| 14 | 对话转为任务 · P2 | 已修 | 在确认页编辑选摘和目标项目，按钮明确创建任务草稿。跨项目时提示来源和保存位置。 | [App.tsx:587](../../../apps/desktop/src/App.tsx#L587) | 组件测试 |
| 15 | 任务工作区 · 绑定 · P2 | 已修 | 工作区绑定状态、短提交、打开目录优先。完整路径和绑定证据按需查看。 | [TaskCodeWorkspace.tsx:97](../../../apps/desktop/src/TaskCodeWorkspace.tsx#L97) | 组件测试 |
| 16 | 项目管理与导入 · P1 | 已修 | 项目列表与状态前置。导入、重关联、归档说明及完整路径只在对应操作内呈现。 | [Projects.tsx:76](../../../apps/desktop/src/Projects.tsx#L76) | projects |
| 17 | 数据工作区 · 预览与副本 · P1 | 已修 | 优先展示文件、观察结果和副本状态；预览选项与逐项证据按需打开，原生复制范围保持。 | [DataWorkspaces.tsx:29](../../../apps/desktop/src/DataWorkspaces.tsx#L29) | 组件测试 |
| 18 | Goal 范围与计划 · P1 | 已修 | 保存的步骤与依赖前置，配置范围、预算和 JSON 编辑折叠。历史授权过期时明确标注。 | [GoalPlanning.tsx:159](../../../apps/desktop/src/GoalPlanning.tsx#L159) | goal-review、组件测试 |
| 19 | Goal 步骤编辑器 · P1 | 已修 | 当前步骤优先显示，逐节点展开参数，保留图和审批范围编辑。原历史取景无需用作本轮通过证据。 | [GoalPlanEditor.tsx:134](../../../apps/desktop/src/GoalPlanEditor.tsx#L134) | 组件测试 |
| 20 | Goal · 等待验收 · P1 | 已修 | 待验收首屏显示候选产物或审阅原因，点击定位到该运行。过期范围仍可查看证据。 | [App.tsx:453](../../../apps/desktop/src/App.tsx#L453) | goal-waiting、goal-review |
| 21 | Goal · 已验收与执行节点 · P2 | 已修 | 验收结论与产物列表优先，审查人、完整 SHA 和收据后置。 | [GoalGraphExecutionHistory.tsx:29](../../../apps/desktop/src/GoalGraphExecutionHistory.tsx#L29) | 原生结构化结果组件测试 |
| 22 | Goal · 阻塞 · P1 | 已修 | 显示可核实的阻塞原因。旧版记录没有结构化原因时给证据入口，并保留 Agent 关于缺文件的原消息。 | [App.tsx:457](../../../apps/desktop/src/App.tsx#L457) | goal-blocked |
| 23 | 产物 · 完整性通过 · P1 | 已修，文件打开受限 | 先显示文件名和检查结果，可展开证据。现有 IPC 无单个产物的原生打开命令，文件级直接打开未验收。 | [ArtifactValidation.tsx:104](../../../apps/desktop/src/ArtifactValidation.tsx#L104) | artifacts-pass |
| 24 | 产物 · 完整性失败 · P1 | 已修 | 失败首屏仅列未通过文件、原因、哈希短对照及可用的重查动作。 | [ArtifactValidation.tsx:107](../../../apps/desktop/src/ArtifactValidation.tsx#L107) | artifacts-fail |
| 25 | 失败记录 · 展开证据 · P2 | 已修 | 展开结果后核对预期、实测、策略、输入修订及完整哈希。 | [ArtifactValidation.tsx:145](../../../apps/desktop/src/ArtifactValidation.tsx#L145) | validation-failure |
| 26 | 数据合同 · 规则表单 · P1 | 已修 | 已有规则与结果在前，创建策略、列限制和复现参数折叠。 | [DataContractValidation.tsx:113](../../../apps/desktop/src/DataContractValidation.tsx#L113) | 组件测试 |
| 27 | 固定命令 · 配置与结果 · P1 | 已修 | 命令策略显示实际 argv，历史摘要显示退出码。环境、时限、工作目录及输出证据按需查看。 | [CommandValidation.tsx:116](../../../apps/desktop/src/CommandValidation.tsx#L116) | 组件测试 |
| 28 | 本地分支集成 · P1 | 已修 | 来源与目标、准备结果和当前动作先于预算与完整收据。上线前沿用原生合入范围审阅。 | [TaskIntegration.tsx:107](../../../apps/desktop/src/TaskIntegration.tsx#L107) | 组件测试 |
| 29 | Factory · 委派范围 · P1 | 已修 | 先显示已有运行和委派，再展开配置预算、递归、接力和返工。 | [FactoryRuntime.tsx:137](../../../apps/desktop/src/FactoryRuntime.tsx#L137) | 组件测试 |
| 30 | 经验候选 · 草稿 · P2 | 已修 | 从已验收产物页开启经验候选。编辑、预览和来源证据分阶段呈现。 | [TaskExperienceProposal.tsx:35](../../../apps/desktop/src/TaskExperienceProposal.tsx#L35) | 组件测试 |
| 31 | 能力总览 · P1 | 已修 | 连接、Skill、MCP 分类展示，添加入口可见，创建表默认收起。 | [App.tsx:522](../../../apps/desktop/src/App.tsx#L522) | capabilities、skills、mcp |
| 32 | 连接 · 失败诊断 · P1 | 已修 | 显示具名连接与探测状态。完整程序路径、身份和诊断记录折叠，需诊断时展开操作。 | [Connections.tsx:99](../../../apps/desktop/src/Connections.tsx#L99) | capabilities、组件测试 |
| 33 | 托管 Skill · P1 | 已修 | 显示已管理 Skill 和添加入口。修改来源和生效证据按需打开。 | [ManagedSkills.tsx:38](../../../apps/desktop/src/ManagedSkills.tsx#L38) | skills、组件测试 |
| 34 | 托管 MCP · P1 | 已修 | 显示已管理 Server、工具数量和添加入口。授权审阅继续列出完整变更。 | [ManagedMcps.tsx:40](../../../apps/desktop/src/ManagedMcps.tsx#L40) | mcp、组件测试 |
| 35 | 全局历史 · 原始事件 · P1 | 已修 | 用人类可读的事件摘要、对象和当地时间替代原始行，合并连续片段。支持按事件类型筛选，保留分页和通知定位，输出原文折叠。 | [App.tsx:609](../../../apps/desktop/src/App.tsx#L609) | 组件测试、真实 IPC |
| 36 | 通知与待办 · P2 | 已修 | 保留待处理排序和打开任务主操作，免打扰和来源细节分层。 | [TaskNotifications.tsx:54](../../../apps/desktop/src/TaskNotifications.tsx#L54) | 组件测试 |
| 37 | 设置 · 后台与恢复 · P1 | 已修 | 划分常规、后台、恢复和主机。常规先显示语言和连接状态，实例回执按需查看。 | [App.tsx:565](../../../apps/desktop/src/App.tsx#L565) | settings |
| 38 | 远程 Host · 设置入口 · P1 | 已修 | 列表和状态优先。新建 Host 表单和长登记说明默认折叠。 | [Hosts.tsx:455](../../../apps/desktop/src/Hosts.tsx#L455) | hosts、组件测试 |
| 39 | 远程 Host · 原生登记回执 · P1 | 已修，原生回执待实测 | 恢复入口突出当前记录及查询动作。实际登记、读回执的原生端到端未在本轮拟真库复测。 | [Hosts.tsx:479](../../../apps/desktop/src/Hosts.tsx#L479) | Host 组件测试 |
| 40 | 无顶栏候选 · 任务首页 · P2 | 已复验 | 在主线外壳上实现同样的双列内容精简。无顶栏 Mantine 外壳为独立 PR #37 候选。 | [App.tsx:435](../../../apps/desktop/src/App.tsx#L435) | home、compact-zh |
| 41 | 分类设置候选 · P2 | 已复验并修订 | 分类设置已在主线产品实现，并去掉常规分类的解释文字。Mantine 候选单独进行集成核验。 | [App.tsx:568](../../../apps/desktop/src/App.tsx#L568) | settings |
