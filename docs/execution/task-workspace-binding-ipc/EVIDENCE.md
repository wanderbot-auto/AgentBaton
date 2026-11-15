# 有限来源与未运行范围

固定文档tree base461e250909fec726d421a9367fd70f639a627dfb；仅Git读取/rg/sed与四docs编辑，未运行产品。此次本地origin/main653679db60d5c68f7e3c5b5e146ef58aed10de23的OWNER_INBOX实际读到001/002无003；不是新远程fetch证明。原task-workspace-binding树Root授权修复后最新source2f0b9b3ff50cf8414526989b867e120d7d9fc831/docs0d6a034785ffd3d47ca781b2ec41a5036567bb40，未将其改入本docs分支。

现有代码锚点：Core ipc.rs Request的owner token+expected_revision/idempotency_key、workspaces.create/resolve、setup busy集合、两个Factory scoped allowlist；Native lib.rs fixedframe/call(false)/generate_handler与capabilities/main.json；WSI3 TaskGitWorkspacePreparation/TaskWorkspaceBindingView/prepare_task_git_workspace、scope幂等与fresh_view、storage16 future guard。新方法只是本PLAN建议，不是现有wire。

WSI3原2b独审source-ready REVIEWba77b1c5736a9a6a3b879d02c2edc19bb969749cb967060f2f218be45ecc529c；Root首实际13units @461=12PASS1FAIL/test8.17s、log44adcdc01a7fbe0d6d85ff3b5bd59226b0264f2f211cf7aed6be9d8319a3dec9，失败分类已由2f修复，原件保留。2f定向源码审REVIEWa0315c1acc32baeb233ab1f0a7cf0a827787d2828c061607fc474eb04e828059/MANIFESTb483e45b8cd6591f0065960448287e2209eff5b7fe6397cfa4c9d414dba2fca3，11静态控不是实际Rust重验。本接口没有测试或运行证据，不与这些源审/内部Store结果互移。

本分支有限检查只有git diff范围/空白/源与base全同；没有Actor/Provider/新Host/账户/费用、权限/lease/预算扩展，没有后台executor。公共接口需Root确认后另阶段实施与独审/真实测试。

后续Root原件 `.onboarding/task-workspace-binding-core-units2`：0d source实际13PASS/test9.33s/wrapper26.774188971s/log9c6e3bd0719315e9e29c5ba5d69da2226c4e2fd48ce16ae382891b3b64113b57；public-contract1同源1PASS/test0.82s/wrapper16.540144127s/logda05de309401f7950071f087b753fe6511fb1d94eeca8fab71d423f2efb3d59b；strict-clippy1同源101/2type_complexity/wrapper8.621165110s/log4d1af9d1ce1c860767b4a0383691a20eddfde5f701e5386bf2a74f433414c0a3。作者已在原功能任务实际只读核三log SHA，不重跑。source4d aliases与0d全文件逆改等、原SQL/测试保持；原树当前docs2026620 clean。4d限定review seal8058b423807424b102a9fa0613da3ec72b560dd4b305d30daffb71141749015f/d070cc4e5ce58879b3fbe328f9d166af5e60337dc03d6758235a8b7030ab7be8 source-only；新lint未运行。

本接口方案最小额外读取仅补固定Native tasks_get桥到已存在Core tasks.get，不扩大Core角色/方法，不依赖超1000Task全量列表才能恢复某Task。不在该历史读桥伪造fresh身份或批准；新prepare才返回fresh view且execution_authorized=false。

## 实现源3362cbad（所有新测试尚未运行）

基线95c693db9b9c88dcb8d03b3ccdc78f980a5eb6f2，纯合同508导入为d43b9a8；固定source `3362cbad7f62feeb45b7efdf5b03c5c0ccf5eeb9`。

| 文件 | SHA-256 |
| --- | --- |
| crates/core/src/ipc.rs | b980abd68b7ace66b8b56f8fd8000e139aaae6690f3dc59ecb839dc861f96222 |
| crates/core/src/task_workspace_binding_ipc.rs | 7d77584167011f9c96e939060fc753991cb7b043286e71cdb5d41ffacfd933ea |
| crates/core/src/task_workspace_binding_ipc_tests.rs | f2620fb45faef9e1c1dda2070982012ab5526f3d3b07dc9c2058b94c40ac42e0 |
| crates/core/tests/task_workspace_binding_ipc.rs | c82aadc56a9071e92147389742e4d277659c95ae940e1668c18012958ee4f9c8 |
| apps/desktop/src-tauri/src/lib.rs | 726c1ecf73f642a95510db6fcd9d02ae143ac188538e6198bfc79e2a6406f15a |
| apps/desktop/src/api.ts | 9754c337e90ed8900a43ce60afc7ed302ea2a0f3c0a26ee809538ec98fb1f32b |
| apps/desktop/src/task-workspace-binding-wire.ts | 5f505bbabb69b7389af4573615fd1ffc707a2eb1090113ac50e8d9e1e4ab6d62 |
| apps/desktop/src/task-workspace-binding-api.test.ts | 6d6b74ba432b91868d995026ca65760d8aa680abbc1986ab025a494968293dd7 |

### 作者实际有限检查

选择性rustfmt --edition2021 --config skip_children=true applied/check，限定ipc/strictwire/Core unit/newUnixIPC/Native lib五文件exit0；git diff/cached --check0。Python实核：去Core的module声明/newarm/一个busy method string/test声明后整个ipc等95c（两Factory allowlist及所有旧方法不变）；去Native新frame/wrappers/两handler entries/newunit后整file等95c；去api新import/Task可选类型/两个methods后整file等95c。后端task_workspace_binding/storage/registry/session/lib/Factory ctor、旧两组tests、旧api.test与WSI2guard、三相关锁/权限均逐Git byte相同。

静态声明数4新Unix IPC+2Core+1Native，不是执行数。TS guard未使用JSON.stringify/Number()/parseInt/parseFloat作FS转换或权威比较；FS string同值检查与revision safeint分开。Core/store负责原raw request_digest实验证明，JS只保留其opaque格式并检查两份receipt完全对应，不谎称浏览器已重算Core原字节。

### 新测试SOURCE的证据边界

四Unix函数计划真实本机Daemon/Unix socket、自己的Git/Directory/SQLite/TempDir，与旧helper模式复用而不导入旧tests重复计数。首准备确实调用新write，两个Task得到不同UUID目录；拒绝/只读/同key恢复全表含sequence、source/managed文件和空目录前后比对，准备不产生Grant/budget/Session/attempt/Factoryrun。正常Git worktree登记元数据沿既有原语，用户源内容和index保持，不谎称无任何Git bookkeeping写。

真实丢响应测试源码发送完整已认证生成fixture帧后直接drop socket，不读取回复；通过独立tasks.get观察真实commit后原body/key恢复。它不是普通samekey replay冒丢包。4th测试的outcome_unknown标记是显式raw负控，不伪造成功/审批/候选/停机，也不是SIGKILL证据；Backend真正pending两阶段恢复已有13unit另维度，不能移签本wire已运行。缺identity的stored Task仍可只读显示原ready标记，没有fresh flags；新prepare必须拒OUTCOME_UNKNOWN，不能授予执行。

Busy unit确实启动并持有一个真实DataPreview I/O worker，调用原生产dispatcher（不是Unix eventloop）在owner未tick/reap时命中同setup guard；线程可已完成但还未owned join，均属于真实未对账busy。normal shutdown完成实际join、Drop仅reap自有worker，不用伪Supervisor/活模型/全局signal。unit输入是Directory preview context中的固定Git-prepare body，证明全局busy拒先于capability检查，不宣完整活跃Provider场景已验。所有这些Rust与TS/Native测试尚未编译/执行。

### 再次撤回的JS正则静态归因

reviewer提出无m的$可接受尾行字符；作者内建functions.exec V8以本source exact decimal/hex63/component/acceptance四patterns×4终止符（10/13/8232/8233）实际16项全部false；reviewer独立16项同结果并明确撤回。没有旧Node测试FAIL，不改生产来迎合误报。内建VM有限pattern实验不等于Node/TS suite/typecheck/native/browser，正式it.each负控仍NOT_RUN。

当前只可记录SOURCE_IMPLEMENTED，review与Root真实lease验证待完成；不能称genuine15迁移、新Mac、UI全链或完整130项已验。作者未运行Cargo/Node/Actor/真实Git/Provider/新工具/远程操作，未购买资源/新增身份。
