# 交付检查点

- 固定源码基线 `4be7d4493a3e82624e2eb45329ec5c358503486a`，验证实现 `b84c56765883a9c1f3fdf119562793a97a56cb43`，crate 子树 `a3155a6ca9287c83c3dfd3a1107f364f5a9779a8`。证据在独立提交保留原生产和测试字节。
- 范围是 `crates/remote-host/src/lib.rs` 的模块声明、`crates/remote-host/src/cache_retention.rs`、`crates/remote-host/tests/cache_retention.rs` 和本目录。两个锁文件保持原字节。
- 调用者提供可信当前时间、最近的原绑定／fence、经过验证的 `Snapshot` 和同一连续游标的本机主库确认。只接受最后一页和有限有效期的证据。每条缓存详情还需精确匹配已经持久化的 ID、序列和摘要。其他 epoch 的证据不豁免验证。
- 预览以缓存时间起算项目提供的 TTL。年龄严格超过 TTL 才可成为候选。负数 TTL 或无效时间保留，零只在调用者明确提供的项目策略下使用。保留原因和本机确认状态各自可见。按序列和稳定逻辑 ID 排序，非法 ID 不回显。
- `CleanupCandidate` 描述当前证据下的只读预览。实际使用需要重新验证主库持久性、项目策略、pin、引用、绑定与生命周期，以及单独的清理执行授权。主历史接线、删除和真实 Host/native 兼容性未验收。由 leader 独审与集成。[证据索引](EVIDENCE.json) · [源码合并预览](logs/source-preview.log)
