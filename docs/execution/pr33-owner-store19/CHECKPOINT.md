# PR #33 Owner Store19 交接

- 基准 `ae844ca33406aabd62f44ea73818f9127f211748`，校验过的源码 `73f729d88369f02f07ea30b6b98e2d674e01521c`。
- 真实工具为基准的普通 `batond` 和 `baton`，Apple Silicon macOS 26.6.1，Rust/Cargo 1.91.1，直接对照与候选均使用 Python 3.12.12。前后工具 SHA-256 一致。
- 基准直接套件 47 项，复现 3 失败和 1 错误，Linux 专属控制跳过 1 项。候选直接套件 48 项，47 通过和 1 跳过。历史反演与源守卫 17 项通过，基础设施全套 1,062 项中 16 项跳过、0 失败。
- 本次实测覆盖私有真实 Core IPC、只读 SQLite、Grant 测试权限回执和未确认的 Factory 准备路径。Linux 本地复跑、macOS GUI、Factory 实际 OS 确认及候选托管 CI 未运行。PR #33 交给集成窗口。

复现时通过 `scripts/collab.sh run --native --` 串行运行。对基准的私有 checkout 执行 `cargo build --offline --locked --bin batond --bin baton`，设置 `AGENTBATON_OWNER_FIXTURE_DAEMON` 指向同一对构建工具中的 `batond`。分别从基准与候选运行 `python3 apps/desktop/src-tauri/tests/test_native_owner_dialog.py -v`。基础设施验收命令为 `python3 -m unittest discover -s tests/infrastructure -p 'test_*.py' -v`。
