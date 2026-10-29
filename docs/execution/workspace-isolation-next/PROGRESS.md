# 调查进展

状态：WSI1_F1_SOURCE_FIXED / SOURCE_READY_FOR_ROOT_CONTROLLED_TESTS / NOT_COMPILED / TESTS_NOT_RUN。调查基线8c178088；调查提交eb0312f只有四记录。Root随后批准的WSI-1源提交`dedee373bd9f587de87ebe545ecaa88916a7d2b9`仅新增workspace_resolution.rs、workspace_resolution_tests.rs及lib五行登记/re-export；本轮记录另行更新，无IPC/Native/TS/DDL/锁改变。

已读AGENTS/gate/APPROVAL、GOAL、RUNBOOK、MASTER_PROMPT对应执行原则及OWNER_INBOX。ACK OWNER-001/002；文件里的历史“待ACK”不是本次授权未生效。未发现OWNER-003；未新增授权。specs中specified/not_granted是设计快照，当前三个需求在实际进度表均partial/implementing，未将该状态改绿。

## 确认已有功能，避免重复建设

- Git和Directory都可导入。`inspect`明确验证目录、Git根与配置，记录canonical path/device/inode；Project重复导入以实际身份或root识别，非Git不会初始化Git。已有Projects表单、归档和重关联。
- `reassociate`保留同一个目录device/inode与kind；Core再检查Project revision/archive，记录同ID事件与新revision。已实现同FS移动与旧路径替换拒绝。跨FS复制不能用同路径或同内容冒认旧身份；当前拒绝是安全边界，不是已有跨FS重定位功能。
- `create_workspace`真正调用 `create_isolated`，有creating持久意图、幂等和outcome_unknown，不是仅UI占位。目的地必须在原项目外、私有同用户managed root，exact HEAD detached worktree；原dirty文件保持不变。不能把未提交内容当已进入隔离HEAD版本。
- Directory编码工作区不能直接借 `workspaces.create`：该方法明确CAPABILITY_UNSUPPORTED。已有DataWorkspace方案才是可复用路径：选定数据版本、真实Native批准、worker复制、Ready后注册managed Directory、fresh witness及Grant。`ready_copy_survives_deleted_original...`已覆盖原数据移除后的现有copy；不需重造通用复制器或全目录快照平台。
- Session创建和supervisor已有ready managed workspace/项目/Grant/identity/lease/输入校验，实际传给运行器的是managed `workspace.root`；这不等于OS filesystem sandbox。当前未发现由Session正常选择原项目路径作为任意cwd的接口。

## 真正剩余缺口

1. Task创建只产生draft和goal/project引用，无默认准备关联、工作区分配或统一任务Router。App随后通过ManagedSessions手工选择workspace/connection/Grant；`routes.rs`只管协议route，不是D05/D18所述任务调度器。
2. 已有安全隔离能力与“单个/并行写任务默认各自隔离”的体验/调度保证不是一回事。现在每次执行要求现成managed workspace，但没有新的任务入口默认生成/绑定自己隔离目录的全链证据；不能据手动worktree测试宣布AB-PRJ-005完成。
3. Project/Workspace identity没有独立host字段，当前grant校验只接受local；本地路径/dev/inode不是跨host全局身份。下一根任务单元应明确local、不猜远程，不因此新增Host平台或买机器。
4. Project移动现成，已有managed Git linkage与仍活跃批准/租约如何重新验证需补证；不应自动改Git commondir指针或把旧Grant跟随目录移动。归档保存历史、不删除源文件的现有语义必须保留。
5. Factory单workspace与false创建权限是当前真实批准合同。它可以在同一已隔离目录中按owned-stop串行handoff，但不能据此宣称所有并行子任务已各自新建隔离；扩展需单独完整Native能力合同，不能借本次PRJ调查放开。

## 证据与本次操作

重新检查已有PR17 c43官方Rust原log（既有once下载，未重新取远端）：project_ipc3、registry10、workspaces/safety23真实通过；五个相关源码/测试文件与本基线逐字相同，见EVIDENCE。另三项既有Data真实测试的同名通过行也已确认；Data/Session/Factory整体源码此后变化，不能跨版本移签整套结果。

调查阶段只有读取、固定Git比较和四份记录编辑。WSI-1阶段编写真实只读实现和测试源码，使用rustfmt解析/格式化并检查这三个限定文件，未跑任何产品测试/编译。初次两个exec transport disconnected后，一次短git status成功恢复，同一worktree继续，没有重建替代任务。少量初始搜索使用旧文件名TaskComposer/SessionWorkbench，随后定位到App/ManagedSessions；这是定位错误，不是产品失败。所有旧真实失败/CI/当前15运行继续独立保留。

Root已确认四字段和Rust可见性合同，WSI-1源现供独审；当前Core共享target仍Root独占，作者没有Cargo lease。无新增费用、凭据、真实Provider或负责人决定。WSI-2仍需任务准备关联/既有创建审批默认入口，不能停在resolver或一个页面即宣布总目标完成。

## WSI-1源码与测试覆盖（不是实际PASS）

已写9函数：personal项目占位与host区分；现成ready Git仅metadata；Directory不复制/不Git init；stale/archive/foreign-host/unknown引用；源目录替换与Git linkage变化；foreign/root/identity/oversize candidate；过期非空lease不释放；稳定key在task/project/HEAD变化而dirty字节不混入；cancelled task阻塞/请求不能注入授权字段。公共方法返回false旗标、无ready/assigned状态。每个解析调用前后比对全表SQLite值（含sequence）、用户source及managed文件/目录；没有伪Provider或假worker-join。

作者检查仅rustfmt applied/check及有限源码范围/marker核对；测试未运行，Rust类型兼容、Clippy、真实只读断言均待Root实证。非draft阻塞和Directory key为空是保守准备语义，不修改既有Session/Factory权限或工作区行为。

## F1修复与当前待测源（7a04d67a）

初版dedee独审结论BLOCKED，原记录未改绿。F1是Project.document与关系root/device/inode可能不一致；作者已窄修为inspect前同读快照交叉检查，源 `7a04d67a8a12bb3cc74088470560679346230ddc`。只改resolver和专属tests，lib与公共四字段/false旗标完全不变。新增1fn/4腐坏case，原9函数原字节保留；没有执行Rust、Git夹具或产品。

作者实际检查：选择性rustfmt applied/check exit0；git diff --cached --check exit0；有限Python逆改恢复整个旧resolver字节且旧9test为完整前缀，exit0。它们不是编译或产品测试。

GoalLegacy独立定向复审已给SOURCE_READY_FOR_ROOT_CONTROLLED_TESTS，16 standalone SQLite内存精确SELECT/比较控exit0，明确不是Store/Rust。新REVIEW/MANIFEST见EVIDENCE；旧dedee BLOCKED seal不变。当前10个Rust测试函数仍NOT_COMPILED/NOT_RUN，等待Root受控targeted与后续必要Clippy。WSI-2/3/defaultTask UI、真实IPC、Mac均未实现/未验，不将源准备或源审通过计作AB-PRJ需求完成。

## Root实际targeted结果（不覆盖历史NOT_RUN）

Root在 `5c70c314d22f55396c6d5905a89742b0f54bec50` 实际运行 `cargo test --offline --locked -p agentbaton-core --lib workspace_resolution::tests -- --nocapture --test-threads=1`：10PASS/0failed/0ignored/128filtered，test3.54s，wrapper25.261956s，exit0。执行者是Root，作者本轮只读核RESULTS、原log SHA、source before/after和工具hash清单，未另跑Cargo。四个normal-debug/frozen15工具SHA前后相同，证明该次lib测试未替换它们。

当前最新状态为WSI1_TARGETED_STORE_TESTS_PASS；此前NOT_COMPILED/NOT_RUN和dedee源码阻塞保留为各自时点事实。不能扩为完整Core/Clippy/Actor/Native/真实IPC/默认任务UI/Mac通过。下一只读resolve wire建议准备中，尚未实现。
