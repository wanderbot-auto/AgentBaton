# AB-REL-007 交付检查点

基线为 `53da4ce20fdfedae4a745e704394072ad6b742ae`，源码为 `aa040e8977f6c48d26678dad50e3d4d8bcfb3cc4`，交付分支为 `collab/update-backup-issuer`。写入范围仅为 `crates/core/src/execution/update_readiness.rs`、`crates/core/tests/update_readiness.rs` 及本目录。由集成窗口负责独立评审与状态台账。

调用方须由可信 actor 选择私有目的地，并按原流程提供批准、候选制品、迁移和恢复证据。更改准入和实际替换制品仍取决于既有维护边界与最终重新检查。Apple Silicon 验证使用生成的本地子进程和同 schema 试做副本。实际跨版本安装、Linux、签名、真实 Provider、CI 和生产发布留待各自的验收。

精确命令和结果见 [EVIDENCE.json](EVIDENCE.json)。
