# WSI3：Task默认Git工作区持久绑定

基线aa4895955bd2d6d113626c5994323a0fd2aabefc；独立worktree `/workspace/.worktrees/task-workspace-binding`，branch `codex/task-workspace-binding`。需求AB-PRJ-002/003/005，Designv0.2/AB-START-001，OWNER001/002，保留完整130/27。原WSI2a冻结；Factory/PR18的schema15修复不混入本分支。

## 固定内部合同（无公共IPC）

`Store::prepare_task_git_workspace(&mut self, &TaskGitWorkspacePreparation, managed_root:&Path, key:&str) -> Result<TaskWorkspaceBindingView>`。

request仅task_id、expected_task_revision、expected_spec_revision、expected_project_revision、host_id(local)、expected_source_pin(canonical_root/device/inode/common_dir/head)、preparation_key。source pin只与server实际注册Project关系/文档/inspect交叉比较，不将输入path当新能力。owner-only内部API，无自动Grant/Provider/Factory/MCP授权。Directory/个人项目明确拒绝该Git准备写路径，继续既有注册/Native Dataset路径。

Task新增可选typed code_workspace_binding（default/skip_none）；持久binding含immutable原reservation receipt、pending/ready/outcome_unknown状态与revision、可选真实managed pin。receipt固定Task/spec/Project/source版本、server生成binding/workspace/创建key、private managed root与原请求digest。原幂等payload/result保留immutable原receipt；返回view分开当前Task revision/binding和fresh identity_revalidated，execution_authorized始终false。FS身份Rust为u64但serde严格规范十进制字符串，禁止旧JSON/JS number损失。

相同Task不因换key另造目录，另Task必需独立server UUID。相同key/payload从原receipt恢复后仍fresh检查，变payload冲突；pending/unknown不重放FS。无现成workspace_id输入，不把WSI1候选或同Project当绑定。

## 两阶段事务与版本边界

首Immediate事务同时写Task pending(+revision)、managed_workspace creating、immutable幂等receipt及事件，commit后才单次既有create_isolated；final Immediate事务同时更新workspace和Task binding，重新验证Task/spec/project/source/原receipt/pin，漂移或partial错误不可ready，不覆盖其它Task变化。不得套外事务再调用旧自开事务的create_workspace。旧create_workspace的payload/行为与无Task路径保持，受限helper只复用其SQL/event，不能重造copy/worktree。

startup将pending/creating关联对账为unknown，不猜FS已完成、不再次checkout；ready恢复检查关系pin、canonical路径、dev/inode、commondir/head及任何非空lease（过UTC仍busy）。Git worktree的必要.git/worktrees登记属于原原语；不写/重置/暂存用户源内容，保留dirty/HEAD/index/用户文件，不能声称完全无Git元数据写入。

新增Task字段/绑定语义必须storage16：最小备份/事务FKcheck/版本提升，无新表；版本17在任何迁移/恢复前拒。旧None序列化逐字保持只证明向后数据兼容；旧15应用必须拒新16，不能宣向前兼容。genuine15 fixture与历史consumer/CI升级由Root后续独占适配，本源阶段不乱改旧标签求绿。

## ADR

采用Task文档保存唯一默认binding、现registry写意图与幂等表保留原receipt；无新表/平台。新状态是准备证据，不是授权；原Grant/Factory new_workspace=false不变。Task绑定对普通Session准入的严格检查需要一个明确调用点；若Root未分配sessions.rs，本单元不得虚称执行路径已强制遵守binding。

只写限定Core/专属测试和四记录，不改IPC/Native/TS/锁/权限。作者不Cargo/Node/真实Git/Actor/Provider/目标工具，所有新增Rust与Mac验证NOT_RUN；源独审后Root实际串行。
