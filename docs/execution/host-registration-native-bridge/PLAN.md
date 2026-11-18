# C3 · Host 登记桌面桥接

窗口 B 认领 `collab/host-registration-native-bridge`，基线与 feature target `origin/collab/host-registration-integration` 固定为 `80eddd5997072203f3593becef47a2bf09ac9e16`。启动时工作树干净。按任务卡及 C3 wire/intent 合同实现。ACK OWNER-001，已检查本地 `origin/main` 的 OWNER-002，设置 UI 由集成窗口另行安排。

1. 添加七个固定 Tauri handler。仅使用现有 Runtime 目录和 Core 认证单请求直连。完整调用参数为闭式 JSON，转发前使用 Core DTO 和字节界限。
2. 持久预留原请求，在同一个私有日志实现 pending、只读核对、显式原键重试和精确回执确认。保存未知结果、有效回执与可恢复条目。
3. 用真实私有日志、SQLite 和同进程 Core daemon Unix socket 验证路径。通过受控下一跳覆盖截断、协议异常、未知结果与并发变更。增加生产 handler 和边界源检查。
4. 在任务工作树用 collab run/jobs2 生成已锁定的真实本机 sidecars，执行 Tauri crate fmt、targeted Rust、strict Clippy 和 Python 源测试。保存原始日志、版本、源码与二进制。所有 owned 进程 join 后执行 scope/diff/feature target preview 并交 leader 独审。

## 验证命令

在 B 工作树内使用 `bash scripts/collab.sh run --`，Cargo 串行、jobs2。

```sh
cargo build --release --locked --bin batond --bin baton
bash scripts/prepare-desktop-sidecars.sh
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
rustfmt --edition 2021 --check apps/desktop/src-tauri/src/hosts.rs
python3 apps/desktop/src-tauri/tests/test_hosts_native_source.py
cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --lib hosts::tests
cargo clippy --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
```
