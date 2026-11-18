# 验证证据

ARM64 基准 `7b04514caaf7807b46018547a8b10f2a5e7fc3d6`。通过 `scripts/collab.sh run --` 执行 `cargo test --locked --offline -p agentbaton-core --no-fail-fast --test project_ipc --test registry --test policy -- --test-threads=1`，26 项通过。日志 SHA-256 `f7437202d1e74abdecd271f3c1c5b62c1c590a12eee0e2bba9d7f85761637e55`。

首次编译诊断保存在任务产物 `artifacts/project-config-core-ipc/cargo-check.log`。
