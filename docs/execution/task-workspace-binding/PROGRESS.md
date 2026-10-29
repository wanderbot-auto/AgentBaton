# 进展

已建立独立分支，核对storage15入口、Task deny_unknown_fields、registry两阶段创建/旧ready快捷返回及租约语义。源码尚未编译/运行，未改Factory当前实现/CI/原WSI2a树。

固定方向：schema16兼容保护、Task可选typed binding、原receipt与fresh view分离、原子reservation/finalize、unknown不重试FS、现成workspace不借用；源内容不覆盖。只有现registry Git worktree原语，无新复制平台。

必要额外调用点已向Root报告：旧sessions admission若不读取Task binding，不能宣称执行已强制绑定。当前未擅改sessions.rs；等待Root明确有限路径。无新用户/账户/费用/签名权限。

## 最终source-only实现

当前源2b9080a6（前954首实现）；已实际写storage16备份/未来17拒、Task optional typed binding与None ctor、registry受限reserve/finalize/关联恢复、lib类型登记、新模块以及Root许可的Session单guard。只改允许Core路径，没有IPC/Native/TS/DDL表/锁/权限/CI或Factory现执行policy变化。

专属13个unit和1个公开Rust integration函数源已写，未编译/未运行；作者只有选择性rustfmt与有限静态逆改/路径核验、SQLite内存事务原语控制。原WSI1/2a实际通过属于各自源，不能移签schema16。

自审在首954源后发现recovery若只信doc.id、而不重读实际SQL主键，可在故意错键复制文档时写错Task或留下反复pending；最终2b将恢复输入改为有界SQL id→task_from校验，并加raw-row故障测试源码。首954从未运行，不称旧Rust FAIL或使用虚构复现。另补两commit前联读与FS前fresh gate、Box内部两种reservation结果（不改变公共语义）；此后功能冻结。

实际选择性rustfmt applied/check、diffcheck与静态检查exit0，SQLite :memory: BEGIN/pragma/FK/rollback/行保留原语控制exit0。这不是Rust Store迁移/真实15旧应用/类型/lint证据。所有新增Rust、Actor、真实Git、Native/Mac仍NOT_COMPILED/NOT_RUN。无真实Provider/新账户/新费用/后台executor。

## 实际首轮：12PASS/1FAIL，修复待重验

执行者Root，精确source461e250：cargo test --offline --locked -p agentbaton-core --lib task_workspace_binding -- --nocapture --test-threads=1，实际12PASS1FAIL/0ignored/139filtered/test8.17s/wrapper32.405701685s/exit101。不再把此原源称未编译，源码时间点NOT_RUN保留历史。尚未运行公开integration、genuine15、严格Clippy、Actor/Native/Mac。

唯一失败在replaced_workspace_and_cleared_binding_history_cannot_reassign_or_repeat_creation首replaced变体；fresh_view把当前普通Directory转Git pin时用了首次capability错误。源2f0b9b3只修已有绑定重验四个类型漂移错误分类为INPUT_CHANGED；初次Directory拒绝路径未改，所有测试/原期待码保持。作者只实核原log SHA、四表达式逆改和格式，不重复Cargo/运行失败ELF。

原13unit中的12个通过是实际Store/Git/SQLite测试，包括受控v15形状；它仍不是genuine旧15应用迁移证明。未通过的函数不能算完成其后两个variant。新源复审与Root重验待完成，原12/1失败和失败ELF保留。

## Root真实重验通过与独立lint失败

精确0d6a034：13unit全部通过/0ignored/test9.33s/wrapper26.774188971s；公开Rust integration1通过/0ignored/test0.82s/wrapper16.540144127s。failed函数三个循环variant此次已完整返回成功，原first-run12/1与ELF保留；仍不能将controlled-v15-shape当genuine旧15生产者。

同源strictClippy实际101/wrapper8.621165110s，唯一两处type_complexity在旧模块202与384行的Option tuple。作者source4d46318仅两命名alias+两局部类型替换，原SQL/params/所有guard/error与13+1测试不动，禁止allow或降lint。类型展开全文件逆改等0d6/格式检查0，作者未Cargo。最初inverse脚本按未格式化单行alias移除而失败，修checker的多行block边界后0；不是产品失败或放宽验收。当前独审/新lint待执行，其它重型验证不冒通过。

## 2026-10-29T19:55:06.250426+00:00 — Root actual backend evidence

At0d6a034 actual13 binding units passed (9.33s tests/26.774s wrapper) and separate public backend restart/source-dirty preservation contract1 passed (.82s tests/16.540s wrapper). At95c693d fullworkspace alltargets strictClippy passed (12.553s). Actual old46112/1 failure,0d two-type-complexity lint failure, and older branch-base constantassert lint failure remain; reviewed bound-kind fix2f/rowalias4d and two trivial Root test adapters are separately attributed. See [exact commands/heads/log hashes](LOCAL_RUST_RESULTS.json).

This is not a schema16 full runtime pass. Genuine15 fixture migration, all version consumers, fixed IPC/Native/UI and bothMac remain pending. No missingGrant/Unknown or default Directory behavior was relaxed. No real Provider/identity/spend/production/sign/license/formalApprove authority added.
