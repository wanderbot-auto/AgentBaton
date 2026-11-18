# 验证证据

## 精确候选实测

- Base `7b04514caaf7807b46018547a8b10f2a5e7fc3d6`，实现 `db6cd5f3e77f88b326374aa9ea02cfe7ffb295ea`，集成目标 `84540f293a362c1098c6d6309df9cb1e12cb0dc8`。
- 实测合成提交 `a135ca3ef77c5122fb10b534cf59fab34343aa43`，树 `3e030d7d853a909b2a8de3c534b74b70599bc677`。Apple Silicon ARM64。用该候选自行构建 `baton`、`batond`，独立测试工具目录经过 prepare 与测试前、后和最终 verify，四份报告逐字一致。
- `cargo test --locked --offline -p agentbaton-core --no-fail-fast --test project_config_ipc --test project_ipc --test registry --test policy -- --test-threads=1`，6 + 3 + 10 + 13，全部通过。
- 原 Factory `actual_scoped_mcp_parent_child_new_parent_attempt_preserves_grant_root_and_owned_stop` 和原 Session `native_codex_fixture_is_bound_to_verified_runtime_and_real_turn_terminal` 各 1 项通过。
- `cargo fmt --all --check` 与 `cargo clippy --locked --offline -p agentbaton-core --lib --test project_config_ipc -- -D warnings` 通过。
- `baton` SHA-256 `3e2aacfe9e65282230a7383f7701e4dd9746b1a6d66ee4844385ecf1d91882f0`。
- `batond` SHA-256 `df0836a0921e9606d5e551f06c416e91a8d1f958487eb63bacf2f4bdf20cb129`。

## 原件和边界

- 精确基线测试为项目 IPC 3、登记 10、策略 13，全部通过。
- 第一版工具检查、测试编译与重启 fixture 两轮失败保留。最终重启回归通过实际 SQLite 注入保存前的不确定状态，经真实 Unix IPC 掉线并重启后核对快照字节、inode 与目录集合。
- 中间版本完整 Core 单元 468 和 Session IPC 63 通过。原 Factory 目标 11 通过、25 失败，包含尚未准备匹配 CLI 与旧 fixture 的前置错误。最终候选的两项新增真实 Factory 专项与一项原 Factory 回归通过。全量 Factory 未在最终候选重跑。
- 本任务的原生维度为实际 Apple Silicon Core、SQLite、Unix IPC 与本地受控进程。真实商业 Provider 及桌面交互留给下阶段。记录中的 Custom ACP 子代验证只涉及版本证据，指令由已批准的匹配 Provider 使用。

原始日志位于任务 `artifacts/project-config-core-ipc/`，哈希如下。

| 文件 | SHA-256 |
| --- | --- |
| `baseline.log` | `f7437202d1e74abdecd271f3c1c5b62c1c590a12eee0e2bba9d7f85761637e55` |
| `cargo-check.log` | `e26b19ed1c9b30d837964aea793dd4c4684c0dbe62b5d5a7285569d1257ba623` |
| `project-config-ipc-first.log` | `4d65ed744c9d22d3a90cec9eb118c12829e967661b600a5f43c78b2b7ae2fd1a` |
| `project-config-ipc-run2.log` | `ba12681fffc823e81ebd5b6e0ecd70a57e48bb4bb50186025c69bc0d01f68a53` |
| `project-config-ipc-run3.log` | `2f13e10bb50475a2f89408cb797929592fa828d9d15927aacb5539f9c46f6784` |
| `regressions.log` | `2ee15148776ac4359eb4a41e9aad3360bf5e0c7550b0e1e5010181b9569dc910` |
| `clippy.log` | `65a21a7518fa683661c36daf91fb7a27981e97d91238cfb7b3cb2762939b6e2d` |
| `combined-preview.log` | `6e0bca3c494be26030b69ed4a7b04b975374e3037fb3268463d95adc8705c9dd` |
| `candidate-build.log` | `0fb5ce775e87b789012b100041caecde619dbb4bd8087d0864580e4ce1d1d393` |
| `candidate-prepare.json` | `bb481b85750afb0a6e7109a6eb56d897230c8dd913182e31bda0aafe282cba94` |
| `candidate-verify-before.json` | `bb481b85750afb0a6e7109a6eb56d897230c8dd913182e31bda0aafe282cba94` |
| `candidate-verify-after.json` | `bb481b85750afb0a6e7109a6eb56d897230c8dd913182e31bda0aafe282cba94` |
| `candidate-verify-final.json` | `bb481b85750afb0a6e7109a6eb56d897230c8dd913182e31bda0aafe282cba94` |
| `candidate-ipc.log` | `bd72e0981f50b0c9b843c868ce25310ab886b95f2368b47451ade3156dad5f0e` |
| `candidate-fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `candidate-clippy.log` | `bdc689c75f682aa65194e083e32380dbe8150a70e6331caa59b5d72a75f8e8d0` |
| `candidate-factory-regression.log` | `ea97e463ed1a39d5eb3fed3a9dd0d528f0057ac942a829a0499d647007802c93` |
| `candidate-session-regression.log` | `448f01ae32ce2e42f81b2987edba26f7a5d20cf5e7cb45f38c7cdd110ce294d3` |
