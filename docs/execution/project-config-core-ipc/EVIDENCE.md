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

## a6a9ba9b 补充验收

- 集成目标 `a6a9ba9b4cb11e9531e9b0b9f7f39ef9be9cb505`。首轮 Linux 候选 `ce179dfd32ad6442921838d1f19e3d1aa049f2ef`，树 `faf1f67d6de2b97fa6e7dd9705990eb213ee28b9`。它与已测 Mac `a135ca3ef77c5122fb10b534cf59fab34343aa43` 的 Cargo、Rust、App 和准备脚本输入逐字一致。
- Mac 首轮按原时限、串行执行完整 `factory_runtime_ipc`，35/36，退出 101。失败是 `actual_data_copy_owner_scope_and_scoped_mcp_parent_child_parent_use_fresh_private_witnesses` 的第二个 attempt。它已持久绑定并已发送，3150.614 ms 后报告 `provider_session_creation_failed`，无 `session.ready`，随后回收。生成的 fixture stderr 为 0 字节。原因未能进一步确定。
- 基线 `a6a9ba9b4cb11e9531e9b0b9f7f39ef9be9cb505` 与候选 `a135ca3ef77c5122fb10b534cf59fab34343aa43` 使用相同 Runtime fixture、参数及原时限，各自工具身份前后相同。初次独立基线执行因缺少诊断输出目录退出 101，原日志保留。两方补齐同等证据目录后各通过 1/1（3.14s、3.06s）。子会话 runtime plan SHA-256 相同，首次失败未在单测对照复现。保留完整组 35/36 的结论。
- Linux ARM64 首轮 `ce179dfd32ad6442921838d1f19e3d1aa049f2ef` 为 30/32。两个新 Factory 用例从测试进程的 Cargo 布局查找 companion，真实安全守卫拒绝该文件。对照方案采用冻结工具启动普通 batond，batond 按产品逻辑选择同目录 baton。
- 后续测试提交 `c079dbe2360e9bd5efd3268a7ee77115c7303b73` 的精确候选 `99300ad8b481fc99eacba110c0b206876107a193`，树 `5bc88dde4344d01b94f374ac33afd69718101da1`。Mac 修复后项目配置测试 6/6，fmt 和 Clippy 通过。Mac 使用原已核验工具，生产源码相同。Linux 修复候选从其精确源重新构建，断网 ARM64 容器中项目配置 6、项目 IPC 3、登记 10、策略 13 全部通过，容器已回收。
- Linux 冻结前后一致。普通 Cargo 文件布局实测为 baton 0o755 nlink=2 / batond 0o755 nlink=2。冻结工具均为私有 0500、单链接的真实二进制。第一轮没有记录普通路径文件的逐项元数据。修复运行的工具哈希为 `baton` `fbdf5735673584d6a42f67eb19d9bf474a0c339b16694e0e86dee73e05429309`，`batond` `d1ef6ae0d43710388a069278e975a0f60046205e30e05518be5d9bbcac339c45`。
- 原全量 Mac 组的后续准入仍交独立审查。B 的 `wrong_start` 红属于另一份实测记录。未将它归入 A 的 `data_copy` 错误。

原始日志位于本任务 `artifacts/project-config-core-ipc/supplement/`。

| 文件 | SHA-256 |
| --- | --- |
| `identity.txt` | `b33fbeaba2d9284311f18828f982265678c1166a4199716acb2fbb3ddc50cb1a` |
| `fixed-identity.txt` | `99a2128d8b10df734042bffe930d22e8e31702c21ac6fbe500a0e3880e588144` |
| `mac-input-tree.sha256` | `df2c7afdf851b7148f28469c3a4b3dcd5b702ccdceab7c0628b5c0545a3bed25` |
| `final-input-tree.sha256` | `df2c7afdf851b7148f28469c3a4b3dcd5b702ccdceab7c0628b5c0545a3bed25` |
| `test-fixture-only.diff` | `7e018fc07d7e6a421ee275c433ff72df7f22eb3ac6358e162745136a21290f7a` |
| `mac/factory.log` | `001a63b06f0f82b664faff356c92aceae5db9ba23cad2c9cd6377f05660bc164` |
| `mac/first-failure-events.json` | `3a5f5cf0c5ab384bff3e79625c11bc3c80d7087ecb283e3ef08d2b57d7b494e2` |
| `mac/before-verify.json` | `bb481b85750afb0a6e7109a6eb56d897230c8dd913182e31bda0aafe282cba94` |
| `mac/after-verify.json` | `bb481b85750afb0a6e7109a6eb56d897230c8dd913182e31bda0aafe282cba94` |
| `mac-compare/baseline/case.log` | `acf0cede697e384d894785564089a4cb317949469306d5ef84c2a9b2566b2e06` |
| `mac-compare/candidate/case.log` | `13eb9277403cda40ce0d38d3d2c760832cccbdfeed009b4fa18fb6ac6f6907ea` |
| `mac-compare/fixed-ipc/test.log` | `7e9fefb833229809ae748088e2641c48b458ceabf7ba4a6a016706fc50514528` |
| `mac-compare/fixed-ipc/clippy.log` | `12f2a833c3653cd8fcec54aee0ecacf81ba09ac1547463d311296edec7234dfe` |
| `mac-compare/fixed-ipc/fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `mac-compare/replay-baseline/case.log` | `ea2c144671d6ca894d8918bf6e17269ab45c3b825a943e956aa25f081c861c9c` |
| `mac-compare/replay-candidate/case.log` | `793b527725f4c052bea371917c018cdf7b0f1cea62709ba5cb82bc1712ef46e1` |
| `mac-compare/session-timing.json` | `e06b3ccd8bd3bb6b6e07fbb24cee34206f4520407522bd74833c2c2efa35a01f` |
| `linux/targeted.log` | `53d79ac46ac76c05273a748852cb0bee3327f1d35fdf2ba527ed31ab894e335a` |
| `linux/prepare.json` | `0ba9e0f35a858cd43346131f475e05b661e6d863ac3add2ca5b4e03ea02faf60` |
| `linux-retry/platform` | `52d9994dfe0f2703159d02524605323f6137d32dd30f4856b4623d1f0f74a846` |
| `linux-retry/build.log` | `fd37e9a59d14cd7a679dc0d86eb983ff6528f48a848d032b911d630c2b87528b` |
| `linux-retry/targeted.log` | `c901f69fc76072f5c53971f2be8598b026127fe5b0a6aae89230c41fc840276a` |
| `linux-retry/prepare.json` | `39aa14eec03829a58e34203fbd6b1918fa70e94131e0e4f05195fc7a1def6a92` |
| `linux-retry/before-verify.json` | `39aa14eec03829a58e34203fbd6b1918fa70e94131e0e4f05195fc7a1def6a92` |
| `linux-retry/after-verify.json` | `39aa14eec03829a58e34203fbd6b1918fa70e94131e0e4f05195fc7a1def6a92` |
| `linux-retry/companion-layout.json` | `032af86147ad2d0163beefc278cd4ad7c34c80e2718677a1fb5907cbf9e75f73` |
| `linux-retry/post-container-inspect.log` | `540662eaf05fd89f7276f0b0cb70015892690956bc79e0f92ca32dd15a353324` |
| `mac-compare/replay-baseline/runtime-diagnostics-5521f19bd9bf20926f7014a2ab8bfc25a6f321aeb499fc73acce82d6c9029405.json` | `5521f19bd9bf20926f7014a2ab8bfc25a6f321aeb499fc73acce82d6c9029405` |
| `mac-compare/replay-candidate/runtime-diagnostics-8d7af251c076eee0f908f41283f9ab22d8f97d634dca47bbfab970117f13fac5.json` | `8d7af251c076eee0f908f41283f9ab22d8f97d634dca47bbfab970117f13fac5` |
