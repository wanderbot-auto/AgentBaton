# 交付检查点

- 固定源码基线 `46e38016256dfe3ff2646eb546cad5b8179e27ef`，验证实现提交 `a15b4990a1e9837039990619a8affeed17bfa21f`，crate 子树 `2bd5102b43d3038d9990d2f827a29cd0483884c3`。
- 认领只包含 `crates/remote-host/src/lib.rs` 的模块声明、`crates/remote-host/src/transfer.rs`、`crates/remote-host/tests/transfer.rs` 和本目录。现有 lockfile 保持原字节。
- `plan_transfer` 绑定返回的 Host、项目、连接、attempt、输入版本、配置、身份、映射、接收方和敏感级别。政策、审批及资源范围引用必须由调用者核验。类型化数据分类拒绝秘密和账号资料。计划包含逻辑 ID、版本、字节数与摘要，调用者需在实际交付前重新核验当时的批准和绑定。
- 产物级别沿用 `task-result.v1` 的 `public`、`internal`、`sensitive`，作为绑定允许的最高等级。单项等级还须与批准项精确相同。`unknown` 或其他绑定标签、未知数据类型均拒绝。校验器依赖来源分类，摘要证明的是字节相等。
- 合成字节只在内存中核验。留待后续的是实际文件读取、传输、调度接线、真机远程与 Linux/CI 验收。无生产凭据、外部 Host、Provider 或网络活动。由 leader 独审及集成。[证据索引](EVIDENCE.json) · [源码合并预览](logs/source-preview.log)
