# 下一工作单元：任务默认隔离接线（调查，不是实现完成）

基线 `8c178088d7213f625695a9d31a5b6d31cc342a42`，tree `b926b5318208fdb33d321173f026e65b83e04329`；分支 `codex/workspace-isolation-next`。范围：Design v0.2 / AB-START-001 / WP05，并与 WP08 的 Task/Router/派发、WP11 桌面连接。保留130项范围、27题决定及 OWNER-001/002；本调查不修改共享需求表，不将 specified 快照解释为未获开发授权。

初始调查提交eb0312f只写本目录四文件。随后Root明确分配WSI-1最小实现：仅新增内部readonly模块、lib登记、专属测试和本四记录。依赖、锁、DDL/schema、CI、全局 STATUS、用户项目、运行中的 Factory15均不修改。不执行 Cargo/npm、产品、真实 Host/Provider、模型或私有账户。费用、生产发布、签名、最终许可证仍单独关闭。计划不能计作产品完成。

## WSI-1已冻结的内部Rust合同（源dedee373，尚未编译/测试）

`Store::resolve_code_workspace_preparation(&self, &CodeWorkspacePreparationRequest) -> Result<CodeWorkspacePreparation>`。模块本身private，DTO仅re-export给Rust；无IPC/Native/TS。request仅task_id/expected_task_revision/expected_project_revision/host_id四字段且拒未知字段；host只local。方法名固定code preparation目的，不从goal文本猜权限。

结果含真实task/spec/project版本、local host、可选source WorkspaceIdentity与当前HEAD、有限Git准备key、固定state/reason与候选metadata。personal项目占位返回NeedsRegisteredProject；Git默认NeedsGitWorkspace；Directory默认NeedsDatasetSelection且因未选dataset不虚构版本/key。非draft仅Blocked/TaskNotDraft，防止把准备建议用于运行中任务迁移。每个结果的execution_authorized/creates_workspace均false；候选assigned_to_task/identity_revalidated均false，没有Ready变体或自动分配。

实现复用get/get_project/inspect，在显式Deferred只读transaction中检查版本/归属，成功rollback释放读快照，失败由RAII回滚。Task/Project文档最多256KiB；candidate<=1000、单项256KiB/总4MiB，关系列/document/root pin一致。非空lease holder不管UTC是否过期均标has_unreconciled_lease，绝不释放。key仅为task/spec/project/source身份及实际Git HEAD的有限hash建议；不把dirty字节计入HEAD版本，不是权限、assignment或持久平台。

新增9个测试函数源码使用真实临时Store/Git/Directory；所有read分支断言持久表/事件/sequence与source/managed文件和空目录前后一致。**它们尚未编译、尚未执行**。后续必须独审、Root获得Cargo窗口实跑及修复，再进入WSI-2/3；这一步不完成AB-PRJ-005。

## 需求与现状

| 需求 | 设计验收 | 已有基础 | 剩余范围 |
| --- | --- | --- | --- |
| AB-PRJ-002 / WP05 | Git和独立数据目录均可追踪，路径/host身份明确 | `WorkspaceKind::{Git,Directory}`、Project registry、显式 Git worktree、批准的选定数据复制及 Directory managed workspace | 自动任务准备尚未联通；Host在现执行路径只允许local，Project/Workspace identity结构未持久化独立host引用；不能把导入Directory等同已获写权限 |
| AB-PRJ-003 / WP05 | 移动后验证身份并保留历史，不仅依赖路径文本 | device/inode + canonical root；同文件系统 reassociate 的CAS、幂等、事件和Project ID保留 | 缺少项目移动后已有Git managed linkage/活动引用的完整流程验收；跨文件系统复制明确拒绝，未来需显式新映射，不得凭同路径或相同内容静默继承权限 |
| AB-PRJ-005 / WP05 / D18 | 单个、并行编码任务默认独立，不能原地写或只提示风险 | 真实 detached worktree、私有Data copy、Session仅接收ready managed workspace、lease/unknown防重用 | `tasks.create`仍只建draft；默认准备、选择、授权与启动的产品流未联通；不保证默认每任务分配独立可写目录；Factory当前只批准一个现有workspace，创建权限保持false |

## 必须复用的现有调用链

1. `projects.import` → `Store::import_project` → `workspaces::inspect`：不执行内容、不初始化Git；身份以实际目录元数据记录。`projects.reassociate` → `reassociate`：原device/inode及kind一致才更新路径、revision和事件。
2. Git：`workspaces.create` 的项目CAS/expected HEAD/稳定key → 私有managed root → 持久化creating意图 → `create_isolated` 的 exact commit detached worktree → ready或outcome_unknown。禁止stash/reset/force覆盖；不能把原项目dirty内容谎称已复制进HEAD快照。
3. 非Git：已有 `DataWorkspaces` UI → bounded preview/选定文件、hash及限制 → 真实Native确认 → `finalize_data_workspace_proposal` → 真实copy worker → `finish_data_copy` 注册Directory managed workspace。随后Grant/Session要求 `DataInputWitness`，不能仅凭Directory标签或disk marker授权。
4. Task：App `createTask` → api `tasks_create` → Core `tasks.create` → `create_registered`，目前没有workspace准备或任务写集分类。执行通过 `ManagedSessions` 手选workspace/connection/Grant，或Goal/Factory已批准binding；`create_session_inner`、policy与supervisor固定实际managed root，不接受模型任意cwd。
5. `routes.rs` 是明确Provider协议选择、禁止fallback，不是完成的项目规则/模板/能力/数据/负载任务Router。不能用它的名称冒充已完成默认隔离调度。

## 下一可执行实现，按依赖拆分但不缩减总目标

### WSI-1：固定只读准备合同，再接默认入口

先由Core owner冻结任务与工作区准备的只读公共合同；不改Grant/Factory权限，不引入新表。可复用现有get/list接口实现内部纯解析器；只有确需跨IPC的一致CAS结果时才增加一个固定只读方法。草案如下，**字段/方法尚未实现，不是现存wire**：

- 请求：task_id、expected_task_revision、expected_project_revision、明确的输入版本/选定数据引用；host只允许现有local。不能把模型goal文本猜成有权写代码，亦不能让Renderer的read_only标签绕过managed workspace要求。
- 保留个人draft/快捷讨论；`Task.project_id == "local"`是既有未注册个人项目占位，不等于执行host_id的local。需要文件执行时先关联真实注册项目，不能凭该字符串回退到daemon cwd、HOME或当前仓库。
- 返回：真实task/project revision；source kind/opaque identity；Git HEAD或已选dataset引用；可用ready managed workspace；`ready / needs_git_workspace / needs_dataset_selection / needs_data_copy_approval / needs_execution_grant / blocked`明确状态及固定原因；记录本次只是准备建议，不是执行授权。
- 每个写任务的默认计划分配独立可写目录。Git沿用server生成目的路径及现有创建意图；stable key必须绑定task/spec、project revision、source version，不能浏览器重挂后随机新建第二份。先复用已有idempotency/creating记录，评估确需的Task→preparation持久关联后再选最小document/事件表达，不能先决定schema16或写SQL迁移。

建议允许路径（下一真正作者需由Root分配，当前未修改）：`crates/core/src/workspace_resolution.rs`（如确需新内部模块）、`registry.rs`受限适配、`lib.rs`模块登记、专用Core测试；如新增只读wire，再单独分配`ipc.rs`、Native固定wrapper、对应TS类型。一个Core合同owner，无并发迁移writer。

### WSI-2：Task主入口连接已有准备、审批和派发

在同一任务页面复用Projects/DataWorkspaces/ManagedSessions的真实操作，展示已解析的source与writable root；默认先准备，再进行现有完整Native Grant确认，再按已批准binding启动。Git根任务可调用已有workspaces.create；非Git必须走已有exact-selected dataset批准，不能默认递归复制整个目录、认证目录或隐式初始化Git。当前未预授权新数据复制时显示需要批准，不自动调用finalize。没有安全路径则blocked，不回退原路径。

建议允许路径：`App.tsx`的小入口、一个`TaskWorkspacePreparation.tsx`与对应测试、必要api/i18n及已有组件的窄复用。不得另建任务/研发管理平台，不重写旧五套harness或大范围UI架构。

### WSI-3：默认分配/恢复与并发验收，再扩展动态子任务

验证单任务、两并行任务得到不同真实可写目录；同一任务同key恢复不重建；unknown创建必须对账，lease过期不是旧writer已停。复用已有lease/Session fencing与budget，而非另设调度账本。用户移动项目时先更新真实身份并使旧批准引用失效/需重新验证，绝不自动改Grant、旧snapshot或修复用户Git元数据。

Factory/childGoal是后续独立权限合同：当前 `selection.workspace_ids.len()==1`、`new_workspace_authority=false`、`new_workspace_authorized=false`必须保留。不能仅在UI或MCP中置true。若要动态派生工作区，先定义完整Native批准的创建policy/host-private witness、实际路径与原Grant范围、输入版本、预算/TTL/祖先归属、持久意图和真实stop；由现Core owner评估与当前15兼容方式。此依赖不阻塞独立的根任务默认准备接线。

## 必需测试（未来执行，当前NOT_RUN）

| 层 | 正常 | 失败/恢复 |
| --- | --- | --- |
| Core解析 | Git/Directory、local、task/project版本、稳定默认准备key | stale/archive/foreign workspace、替换目录、缺输入/授权、未知host、伪read_only；只读解析不创建意图或预算 |
| Git准备 | 保留dirty原树，对明确HEAD创建新detached checkout；两任务不串写 | HEAD漂移、hooks/filters/includes/promisor/替换ref/隐藏index拒绝；creating后持久化失败与同keyunknown不得再checkout |
| Data准备 | 精确选定hash/限制、Native批准、copy→Ready/witness；原数据与mtime不变 | symlink/hardlink/认证目录/越界/漂移/空间不足/cancel/timeout；不完整copy或无join不Ready、不原地写；旧可用copy不依赖原目录继续存在 |
| 身份/并发 | 同FS rename后同Project ID/history，新任务重新解析；双任务目录不同、真实worker停止 | 相同路径被替换、跨FS复制不能假旧身份；旧managed Git linkage/活动引用需fresh验证；unknown旧writer/lease到期不复用、不reset预算 |
| UI/真实IPC | 默认入口无需手工拼workspace；准确可读根、Native批准、真实生成ACP修改隔离目录 | 取消准备/拒绝审批、响应丢失explicit same-key恢复、刷新重挂；HTTP不开放Owner finalize/generic、新测试原件私有/公开有限摘要 |
| macOS交付 | Apple Silicon/Intel真实本机完整流程和历史兼容 | Git路径/文件身份/私有文件/重启、默认包实际运行；与Linux/browser/Mock证据分列 |

## 可逆ADR建议（草案，嵌本记录而非另添文件）

采用“复用现有隔离原语，准备先于执行批准，Task级默认分配，现有权限不自动继承”。先冻结readonly resolver及任务版本绑定，再接既有方法；无证明不宣称OS沙箱、全部后代隔离或原生账户隔离。host目前显式local，远程项目/移动跨host另按D05合同接入，不创建主机。保持FS u64在JS为opaque、Native/Core做精确比较。与存储owner评估旧document兼容后才决定是否需要新字段/迁移；不预定schema版本。无需负责人重答27题，局部可逆选择由Root/ADR记录，权限扩展仍走真实产品批准边界。

## WSI-1 F1定向修复：关系身份必须先一致

源修复 `7a04d67a8a12bb3cc74088470560679346230ddc`（parent e87fa8c）取代dedee作为当前待测源。原调查草案/初版源冻结描述保留为历史，最终内部合同仍以上文四字段、无Ready、全部授权/创建false为准。

独审F1发现同ID/revision的Project.document可单独被替换为另一个真实Directory身份，而projects.root/device/inode仍是旧pin；只检查document与FS会返回错误准备引用。这是只读准备引用一致性缺口，不是已证明的执行授权绕过。修复在同一Deferred snapshot、inspect之前精确交叉检查关系root与document canonical_root、关系device/inode与原u64十进制TEXT。root读取上限262144字节，device/inode各20字节，超限或不一致返回INPUT_CHANGED/not_started及固定code-workspace-project-pin，不修复数据库。

新增1个真实Store测试函数源含4case：document-only转向另一个实际Directory、root列、device列、inode列漂移；各断言该固定guardref、所有表/源/managed内容与foreign目录不变。原9函数字节保留，合计10函数仍NOT_COMPILED/NOT_RUN。无DDL、IPC、Native、Factory权限或默认任务入口改变。独审仅source-ready后由Root受控运行临时Git/SQLite夹具，不需要bins、Native、Provider或真实用户项目。

## WSI-1受控实际测试完成（Root执行）

Root在精确docs/source head `5c70c314d22f55396c6d5905a89742b0f54bec50` 运行原targeted lib命令，10函数PASS/0failed/0ignored，test3.54s，wrapper25.261956s；这包括F1新1fn/4cases。源码时间点NOT_RUN记录保持历史，不回写dedee/e87。只读resolver的该Store/Git/Directory测试分片已实际通过；strict Clippy/完整回归、IPC/Native/TS、WSI-2/3默认Task链与Mac仍待后续独立工作单元。下一只读wire合同先提出给Root确认，不在本leaf写IPC或UI。
