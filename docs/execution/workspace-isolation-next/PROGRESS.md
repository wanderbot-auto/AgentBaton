# 调查进展

状态：SOURCE_INVESTIGATION_COMPLETE / IMPLEMENTATION_AND_NEW_TESTS_NOT_RUN。固定源8c178088，只有本目录四记录发生改动。

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

本次执行只有读取、固定Git比较和四份记录编辑；不跑任何产品测试。初次两个exec transport disconnected后，一次短git status成功恢复，同一worktree继续，没有重建替代任务。少量初始搜索使用旧文件名TaskComposer/SessionWorkbench，随后定位到App/ManagedSessions；这是定位错误，不是产品失败。所有旧真实失败/CI/当前15运行继续独立保留。

建议Root在当前15稳定、Core公共接口owner释放后先给WSI-1/2窄作者单元，再并行分配UI与独审。当前无需新费用、凭据、真实Provider或负责人决定；阻塞只是公共Task/preparation合同和当前Core writer归属未冻结。后续测试按PLAN完整执行，不能停在resolver或一个页面即宣布总目标完成。
