# Store19 · v2 历史夹具进度

- 固定 `97803b16276903ffe4b6945966f21adeaad11eeb` 在 Apple Silicon 复现原 `migration_v3` 两例，Cargo 101。两例均在原 `remove_empty_current_factory_increment` 返回 `InvalidQuery`，v2 构造尚未执行。上轮完整测试中的同名两红与原日志 SHA-256 也已核对。
- 源码 `4e7e70b04c2ec8150c0b17e9131842c28e02159b` 接入已验证的严格 19→18 逆向，保留原两个 v2 SQL 和测试名。成功路径观察真实 Store19、空 Host，仍验证私有 pre-v7 备份与重开，并比较 pre-v19 原像。碰撞路径核对关闭态主库/WAL、逻辑镜像和旧数据，并在单独副本只移除冲突表后验证真正升级。
- 同一候选原目标 2/2（0.34s），fmt 和指定目标严格 Clippy 通过。
- 普通工具和默认 workspace 预编均通过。真实准备器确认 API 1、Store 19、空 Host 与正常停止。准备回执和预检、workspace 前后校验逐字相同。
- 与集成窗口对齐后，默认 workspace 于 2026-11-19 00:30:04Z–00:45:31Z 实际运行并自然退出 0。完整运行中的 migration_v3 再次 2/2。其后运行到全部 Doc-tests。摘要区分 Cargo 直接启动的测试目标和两个测试启动的子进程。
