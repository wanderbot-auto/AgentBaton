# 交付检查点

- 固定 base `ec2ca48b12241451905157240238987c821e2b2a`，实现和验证 SHA `7e504de5e080db98cfa091760cb0576e01566a4e`。证据追加在独立提交，保持验证的 Rust 文件不变。
- 写入 `crates/remote-host/src/lib.rs` 的模块声明、`crates/remote-host/src/routing.rs`、`crates/remote-host/tests/routing.rs` 与本目录。两个 lockfile 均未改动。未推送。
- `Attempt::New` 只选已核验授权内的完整绑定。`Attempt::Active` 和 `Attempt::OutcomeUnknown` 返回原绑定。`Decision` 带来选择和解释，dispatch 调用者仍负责核验授权、当前能力与可用槽。
- 仅能使用已核验的、属于该绑定的限时观测。排序采取数据在 Host、已批准的共享引用、已批准传输的优先级。同一数据位置按占用槽比例升序，再按完整绑定键排序。缺失负载或容量用尽均等待，其余合格位置可继续参加评估。
- 固定基线与本地目标 `f3ee057c874582427e77129e21014574c57484ae` 对实现提交均无合并冲突。组合的 `crates/remote-host` 子树与已验证子树 `dbaa123680fa3a688d0f1dbf591f821df8b83a72` 完全相同；实际 handoff 后若目标变化，由集成窗口核对组合。 [源码组合日志](logs/source-preview.log)
- 调度器、执行授权签发、Host 在线探测、数据传输和真机远程执行留待后续接线和验收。当前仅由独立 Rust crate 编译、离线测试和格式/lint 核验。独立审阅与集成由 leader 负责。[可读证据索引](EVIDENCE.json)
