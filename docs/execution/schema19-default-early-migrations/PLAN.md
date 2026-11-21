# Store19 · v2 历史迁移夹具

固定基线 `97803b16276903ffe4b6945966f21adeaad11eeb`，树 `c13081f2a5b8725539dc7477c33937a5920a70e2`。已核对 AB-START-001、OWNER-001 / OWNER-002。只认领 `crates/core/tests/migration_v3.rs` 与本证据目录。

- 核对上次完整 workspace 的两项旧失败，先在固定基线按原测试目标复现，保留确切源码、命令和原日志。
- 复用已审核的空 Host19 逆向，再由原严格助手和原 SQL 构造 v2。观察真正 Store19，核对 v2 源、私有备份、碰撞回滚和重开。
- 原目标两例、fmt 和严格 Clippy 通过后，编译同源普通工具及默认 workspace。与集成窗口安排实际时限，核对工具前后并记录完整命令自然出口。校验 scope 与目标预演。
