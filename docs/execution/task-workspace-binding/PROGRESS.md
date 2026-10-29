# 进展

已建立独立分支，核对storage15入口、Task deny_unknown_fields、registry两阶段创建/旧ready快捷返回及租约语义。源码尚未编译/运行，未改Factory当前实现/CI/原WSI2a树。

固定方向：schema16兼容保护、Task可选typed binding、原receipt与fresh view分离、原子reservation/finalize、unknown不重试FS、现成workspace不借用；源内容不覆盖。只有现registry Git worktree原语，无新复制平台。

必要额外调用点已向Root报告：旧sessions admission若不读取Task binding，不能宣称执行已强制绑定。当前未擅改sessions.rs；等待Root明确有限路径。无新用户/账户/费用/签名权限。

## 最终source-only实现

当前源2b9080a6（前954首实现）；已实际写storage16备份/未来17拒、Task optional typed binding与None ctor、registry受限reserve/finalize/关联恢复、lib类型登记、新模块以及Root许可的Session单guard。只改允许Core路径，没有IPC/Native/TS/DDL表/锁/权限/CI或Factory现执行policy变化。

专属13个unit和1个公开Rust integration函数源已写，未编译/未运行；作者只有选择性rustfmt与有限静态逆改/路径核验、SQLite内存事务原语控制。原WSI1/2a实际通过属于各自源，不能移签schema16。

自审在首954源后发现recovery若只信doc.id、而不重读实际SQL主键，可在故意错键复制文档时写错Task或留下反复pending；最终2b将恢复输入改为有界SQL id→task_from校验，并加raw-row故障测试源码。首954从未运行，不称旧Rust FAIL或使用虚构复现。另补两commit前联读与FS前fresh gate、Box内部两种reservation结果（不改变公共语义）；此后功能冻结。

实际选择性rustfmt applied/check、diffcheck与静态检查exit0，SQLite :memory: BEGIN/pragma/FK/rollback/行保留原语控制exit0。这不是Rust Store迁移/真实15旧应用/类型/lint证据。所有新增Rust、Actor、真实Git、Native/Mac仍NOT_COMPILED/NOT_RUN。无真实Provider/新账户/新费用/后台executor。
