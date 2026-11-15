当前实现树 `/workspace/.worktrees/task-workspace-binding-ipc`、branch `codex/task-workspace-binding-ipc`；base95c693db9b9c88dcb8d03b3ccdc78f980a5eb6f2，合同508 cherry-pick为d43b9a8，源码3362cbad7f62feeb45b7efdf5b03c5c0ccf5eeb9。原后端WSI树与合同文档树保持冻结。本实现仅8允许源码，以下最初只文档阶段说明保留为历史；当前是SOURCE_IMPLEMENTED/NOT_COMPILED/TESTS_NOT_RUN，不是产品验收完成。

# WSI3固定IPC接线（源码已实现，实际测试待运行）

独立tree `/workspace/.worktrees/task-workspace-binding-ipc-contract`，branch `codex/task-workspace-binding-ipc-contract`，文档基线461e250909fec726d421a9367fd70f639a627dfb；原功能树保持其最新修复冻结。本次只写四docs，无IPC/Native/TS/DDL/锁源码修改。需求AB-PRJ-002/003/005；OWNER001/002 ACK，未发现003；完整Designv0.2/130/27与生产/费用/Provider/签名/许可证边界不变。

## 单一推荐合同

新增一个Owner固定写方法 **`workspaces.prepare_task_git`**；不新增generic/reconcile/reset/assign-existing方法。只用本机Owner token的既有IPC认证，Renderer不能指定role、managed_root、workspace_id、destination、Grant或权限。Factory/MCP scoped两个allowlist及工具清单明确不纳入；new-workspace=false不变。

| 输入层 | 固定字段 |
| --- | --- |
| params（deny_unknown） | task_id、expected_spec_revision、expected_project_revision、host_id(local)、expected_source_pin、preparation_key，共6字段 |
| envelope | expected_revision为原Task CAS正整数；idempotency_key为稳定非空<=256B；原api_version/request_id/token维持现协议 |
| expected_source_pin（deny_unknown） | canonical_root、device、inode、common_dir、head；device/inode必须规范十进制字符串且<=u64MAX，其余只用server已登记source比较 |

Core把envelope.expected_revision放入内部TaskGitWorkspacePreparation.expected_task_revision，6params组成余字段，调用已审Store::prepare_task_git_workspace(p,daemon.managed_root,key)。不能先按当前Task CAS拒绝原key恢复，否则会破坏原reservation后Task revision推进；让Store按完整原payload/receipt先处理同key，然后做fresh验证。新key/变payload、source/spec/project变化、foreign/Factory Task都按Store严格拒绝；未知不改key重建。

仅ready且identity_revalidated=true返回成功view：完整original_receipt与当前binding/task_revision分列、observed_managed_pin独立；所有FS身份从Core原u64专用serde直接输出文本，execution_authorized=false。Pending/outcome_unknown/缺关联/部分finalize都拒OUTCOME_UNKNOWN，不能把目录存在或candidate metadata当ready。复用既有Owner Core tasks.get只读持久binding展示pending/unknown，不谎称fresh身份；Native加固定tasks_get(task_id)桥供单Task恢复（不依赖全量snapshot/1000条列表），不新增Core读方法/状态平台。

Native新增固定写桥 **`workspaces_prepare_task_git(selection:TaskGitWorkspacePreparation,idempotency_key:String)`** 及上述固定旧Core读桥tasks_get：typed selection拒未知，固定body/envelope映射、call(false)，不自动daemon、不Owner确认/Grant/finalize；generate_handler登记，capabilities/权限不变。TS拟api.workspacesPrepareTaskGit：输入输出revision safe integer，FS文本只opaque比较/展示，不Number/JSON.stringify授权；输出version/字段/state/receipt关系和false flags严格校验。不能回退旧workspaces.create或无绑定Session。

丢响应时保留**完整原selection+原key**重放；不得用新Task revision或重新resolve出的preparation_key拼进旧key。传输失败保unknown，不把无回包当not_started；Task当前revision可高于receipt.reserved_task_revision，历史receipt不是新执行批准。新准备沿既有setup-busy guard（同workspaces.create）拒活跃worker期新写；不扩租约/Actor并发策略，busy期用现只读Task查持久状态，后续同key恢复。此限制不能冒完整并行默认任务体验验收。

## 后续受限路径/验证

Root确认合同后才分配Core ipc固定arm/owner与scoped拒绝测试、Native lib固定frame/handler测试、TS新binding-wire guard/API与专属测试；Task/storage16/Grant/Factory业务和权限文件不因接线再次扩展。测试必须真实schema16 UnixIPC：初次/同key丢reply/换key冲突、原CAS恢复、stale spec/project/pin/extra fields/envelope、wrong role、pending/unknown拒与没有FS重放；Native精确frame/callfalse；TS大u64/safeint/伪ready/错receipt和false flags；真实Mac/GUI/genuine15迁移另列。所有此接口测试当前NOT_RUN；既有WSI3首测12/1与新kind-drift修复源另记，不能移签为IPC通过。

## 后续实际结果更新（不构成接口验证）

原功能树source2f/docs0d经Root实际13unit与公开Rust1integration通过；strictClippy随后只报两个type_complexity，source4d4631842ac8aa298f7717a89669160f97d9a690以命名alias窄修、docs2026620e0cb65e367e048f564bc8da1e33896570已冻结，未编译复验。合同对外数据/语义未因此改变。原12/1与lint101均保留，不能将内部Store绿色移签为本接口/Native/真实GUI已过。

## 当前实现与测试源

实际Core固定6参数type→envelope TaskCAS组回原Store请求、固定server managed root；setup-busy加入同旧create集合，Factory两个allowlist不动。Native固定workspaces_prepare_task_git/tasks_get与callfalse；TS api新增同名能力及Task可选binding类型、新guard不影响旧methods。原receipt、当前binding、fresh managed head与Task revision分开；Core digest是opaque引用，不在JS JSON重编码作权威。

写4个真实Unix IPC测试函数源码：双Task独立worktree/只读状态；实际write后关闭socket未读取响应→另只读Task观察commit→原payload/key恢复；6字段/类型/CAS/角色/archived/corrupt/Directory拒；缺identity/受控unknown标记重启拒重checkout。新2个Core unit：strict6及exactuint；真实DataPreview I/O线程待reap触发生产dispatcher的busy拒绝、Task只读不改表、正常shutdown实际join及panic路径只回收自有worker。Busy案例不是活跃Provider/模型；未知标记是负面raw控制，非真实SIGKILL/伪成功intent。新1Native frameunit与独立TS API测试全NOT_RUN。

后续Root独审后租约执行Core lib/Unix IPC、Node type/full/build与Native编译/frameunit；真实UI/OSNative/Mac/genuine迁移/完整集成仍另列，不与Backend已测13+1相互移签。
