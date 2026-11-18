# Store19 · 默认 Rust workspace 剩余五个测试目标

任务基线 `84540f293a362c1098c6d6309df9cb1e12cb0dc8`，树 `b3ea94d0b077d6f2b26a7e49e4613a7997f8c935`。已核对 AB-START-001、OWNER-001 / OWNER-002。认领为 `crates/core/tests/{admission_performance,data_workspaces,migration_v5,migration_v6,workspace_open_ipc}.rs` 的五个文件和本证据目录。集成窗口维护全局状态和合入。

1. 在原基线保留普通工具准备和 1060ff83 指定的七项失败。与 C 错开实际时限运行。
2. 核对准入与数据重启的真实 Store19 和空 Host，保持原准入预算与数据副本断言。保持 Owner UnixIPC 前后的完整 SQLite 镜像并显式确认空 Host 表。
3. 复用已审核的空 Host19 逆向，在原逆向之前得到确切 18。保留 v4/v5 历史结构、原始任务字节及私有 pre-v7 备份，验证 Host 备份和未知表拒绝前后的原始数据。
4. 使用默认特性在 Apple Silicon 跑五个完整目标、普通工具准备和 workspace，在原设置下记录自然结果。严格 Clippy 和 fmt，提交前后验证 scope，检查对确切目标的 Git 合成。
