# 证据

当前仅只读源码调查与小合同，无新产品测试。baseline aa489595，原WSI2a已测结果及限制留原记录，不移签本单元。

实际已读：storage.rs Task/Store::open版本15上限/备份/迁移/恢复顺序；registry.rs create_workspace已原子creating reservation→FS→ready/unknown，但ready replay不fresh检查；workspace lease过期只数据库时钟不等于writer stop；create_isolated实际git worktree add会登记Git元数据但不覆盖用户源内容。新绑定需单独fresh gate，不篡改旧API行为来假称已有Task绑定。

所有source/body/types/SQLite/Git/Mac的新增验证均NOT_RUN；当前没有真实15→16迁移证据，不使用篡改version的库冒genuine旧应用fixture。源码freeze及有限静态命令后追加准确SHA/结果。

## 固定源码清单

基线aa4895955bd2d6d113626c5994323a0fd2aabefc；合同bdb89ac96224bd2df67d0539f6f5d6a9df72fea6；首功能95438f49d9df04238e8c574c32f81aadac0a3002；最终功能/测试 `2b9080a6ce03080fd7a890246065934a799b11b5`。

| 文件 | SHA-256 |
| --- | --- |
| crates/core/src/storage.rs | 5fe4b98b2321ecf26f35a5d825d8bfc5baf3b74e78d96927e90a2ceacd090aec |
| crates/core/src/registry.rs | 62de0f81241627e29807e63764a2efaa55dc1420b46cf9b575acc0c6066828d3 |
| crates/core/src/lib.rs | 2044c7f67d1a314d8feb9f2e26016073710e897dc374c410e520547ad68935ec |
| crates/core/src/factory_runtime.rs | 38d9218eb5dd1e89ed16760be92f45f4fd7fbf88984ddb1b59cf7c195d823b0a |
| crates/core/src/sessions.rs | 7b31c59b254a24a057a8a6995d0a7f1bc37c4145e7f37c7cd5a000e3ba4e947a |
| crates/core/src/task_workspace_binding.rs | 23afb90ccaa8fa315f8ce31cf876732113fcb5a689350dbc075ebfc41b2ab98b |
| crates/core/src/task_workspace_binding_tests.rs | aad5761ac50738fab2c9a8748be996fb843e0a7671090b1ca2e7b0217dc83c2c |
| crates/core/tests/task_workspace_binding.rs | 56aad20e79a5873cc412ac7bbcabb7cd976d252cc84591374b7876b496db6754 |

## 作者实际命令与范围

选择性 `/workspace/.toolchains/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustfmt --edition 2021 --config skip_children=true` applied及--check，限定上述8 Rust文件，exit0；没有递归格式化旧模块。git diff/cached --check exit0。格式器解析不证明类型/Clippy/测试通过。

Python静态实核：registry去新helpers/关联恢复行、将两个helper调用逆展开原SQL/event块后整个文件与aa基线逐字等（旧create行为/payload/ready shortcut保留）；Factory ctor删唯一None后全文件等；Session删新guard/comment后全文件等。IPC、旧resolver与10tests、Native/TS、锁/权限文件逐Git byte未改。生产新模块仅1处create_isolated；两个commit前verify_bound_rows与主键恢复、有界typed反序列化源声明已核；13unit+1integration仅是函数声明数量，不是执行结果。

Python sqlite3仅 `:memory:` 原语控制：自建最小tasks/child表、FK开启、pragma15→16在BEGIN IMMEDIATE内rollback恢复15且row bytes不变，再commit16且row不变，exit0。这不是执行Rust migrate/Store::open，更不是genuine旧15数据；不汇总成Rust测试数。

## 待执行测试的具体边界

新13 unit确实调用本模块真实Store/SQLite/Git原语与阶段helper源码，不另造镜像实现。故障控制用显式raw DB变化/SQLite RAISE(ABORT)1811，不伪造成功candidate或Owner批准：reservation迟event失败全表/FS零写；FS已成功但finalize迟event失败两状态全回滚，重启unknown不重checkout；Task cancel/spec/source更改后不ready且不覆盖其它Task字段；错Task主键复制doc拒绝不跨写。pending阶段通过生产共用private reserve函数建立真实持久意图，非SQL手造成功意图。

合法output提交只在私有worktree，比较物理pin/commondir而不锁初始HEAD；原registered source HEAD仍必须不漂移。绑定guard测试会再次调用create_session验证无Grant不能启动，但没有真实Session/Provider/owned-stop或rework E2E，不能宣该维度已验。lease过期案例是故意准备的未对账DB lease状态，不冒真实后台writer或join证明。

公开integration1通过导出DTO/Store API准备、持久Task binding、重启同key、FS整数文本和无Grant拒绝；未调用Unix IPC/Native/UI。两个schema控制故意用新16初始化库设置pragma15，清楚标为controlled shape，不移签真实旧应用fixture；未来17拒与隐藏新语义字段在旧版本拒绝也仍为未运行源码。

原954源码恢复键一致性缺口仅自审推理，未实际跑旧source或形成旧RustFAIL；最终2b定向修复+新raw-row负控等待独审。所有新增Rust/类型/Clippy、genuine15→16/全旧版本consumer/CI、真实Actor/新Session/rework、Native/Mac均NOT_RUN，真实Provider/spend/sign/license权限未改变。
