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

## 源实现冻结与Root允许的准入接线

首功能源95438f49d9df04238e8c574c32f81aadac0a3002；最终功能/测试源 `2b9080a6ce03080fd7a890246065934a799b11b5`。原合同比较source pin等路径都保留。Root已明确许可sessions.rs唯一内部binding-admission guard：在原Session幂等历史回执分支之后、原Task CAS/state之后、新Grant校验之前；只有Task binding Some才查，None原行为不变。Factory Task ctor仅补None，无创建policy/预算扩展。

当前状态严格pending→ready/outcome_unknown；唯一生产create_isolated在独立checkout helper中，reservation提交后立即再验Task/source/pin/root/空lease，未知不调用它。两个commit边界均联读核对Task/workspace/原幂等receipt后提交。recovery按实际Task主键重新有界读doc并验证id，不能按被复制文档的id写另一个Task。相同Task绑定被清空但receipt历史还在也不能换key再创建。

Task当前revision只需不倒退到reserved之前，不能永久等于历史reserved_revision；新Session自身expectedTaskCAS仍严格。准备重放/准入重查原registered source HEAD/pin，但managed root只强制物理身份/commondir，不把合法输出提交或dirty当目录替换。原Grant精确输入校验、candidate/rework验证及lease仍独立严格；guard通过不等于新增执行批准。

## 专属测试源（13unit+1integration，NOT_RUN）

涵盖None字节/decimal大数、双Task独立目录/同key重启与冲突、拒绝零写、pending重启未知和真实目录碰撞、最终event1811回滚、cancel/spec/source漂移不ready、输出commit与Task revision推进不永久锁绑定、原Grant仍必需、过期非空lease、根替换/清空binding拒新key、受控v15形状备份/17拒/隐藏新字段拒、reservation迟故障回滚、错Task主键复制doc恢复拒绝、reservation后FS前source漂移拒绝，以及独立公开Rust合同。没有fake Worker join/TaskAccepted/Provider。

其中v15形状测试明确由本源16初始化后受控降pragma，仅为格式/迁移primitive control，不充当genuine旧15应用证据。Root后续必须实际旧15生产者夹具及旧consumer/CI适配。现已知非本路径Task ctor `crates/core/tests/goal_plan.rs:40`需集成补None，作者未越权修改；旧schema15版本断言亦不批量替换。

## 首次Root实际13unit与上下文错误码修复

Root在461e250909fec726d421a9367fd70f639a627dfb实际编译并运行13 unit：12PASS/1FAIL/0ignored/test8.17s，exit101。失败是已绑定Git工作区被真实rename保留、原路径变普通Directory后的错误码：应INPUT_CHANGED却返回首次Directory准备才适用的CAPABILITY_UNSUPPORTED；拒绝本身未绕过，但错误分类有误。原测试未改允许码/断言，后两个循环variant不能据此次中断声称已运行。

修复源 `2f0b9b3ff50cf8414526989b867e120d7d9fc831` 仅在已有binding的source_from/fresh_view四个GitKind转换将类型漂移映射INPUT_CHANGED；首次Directory准备仍CAPABILITY_UNSUPPORTED。lease/source/物理pin/unknown/FS规则与全部13+1测试源不变。新源等待独审及Root重验，不将旧12PASS直接移签。

## 实际重验13+1通过，严格lint待alias修复重验

Root精确0d6a034实际13unit PASS/test9.33s/wrapper26.774189s，公开Rust contract1 PASS/test0.82s/wrapper16.540144s；原12/1不覆盖。之后同源strictClippy --lib --test task_workspace_binding exit101，只报两个Option多元tuple类型complexity。新源 `4d4631842ac8aa298f7717a89669160f97d9a690` 命名ProjectPinRow/ManagedBindingRow，展开后整模块等0d6；无SQL/错误/守卫/门限/测试变化，等待独审与Root精确lint。没有Native/Mac/genuine旧15或公共IPC新通过。
