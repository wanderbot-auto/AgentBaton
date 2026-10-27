# Fixed-command validation evidence

Environment: managed Linux x86_64 Cloud, current toolchain from `/workspace/.toolchains`, no network/product-provider/account call in command fixtures. Local generated C fixture is compiled by `/usr/bin/cc` under an empty explicitly configured environment. Script fixtures use the approved local system sh; no repository instruction is automatically executed. Actual macOS execution is not available on this host.

Commands used the existing shared target and limited jobs:

```sh
export CARGO_HOME=/workspace/.toolchains/cargo
export RUSTUP_HOME=/workspace/.toolchains/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_BUILD_JOBS=2
export CARGO_TARGET_DIR=/workspace/.worktrees/validation-integration/target
umask 022
cargo test --locked -p agentbaton-workspaces
cargo clippy --locked -p agentbaton-workspaces --all-targets -- -D warnings
cargo clippy --locked -p agentbaton-workspaces --all-targets --target aarch64-apple-darwin -- -D warnings
cargo clippy --locked -p agentbaton-workspaces --all-targets --target x86_64-apple-darwin -- -D warnings
cargo fmt --all -- --check
```

Initial freeze eec524c Linux tests: **89 unit/integration + 4 compile-fail doctests**, zero failed/ignored. Breakdown: 2 existing unit + 26 new command + 21 existing deterministic-data + 5 existing Git-security + 18 existing workspace + 17 existing validation/adoption. Two new compile-fail tests prove the command rule and command-only acceptance are not Deserialize. Both Apple target all-targets Clippy checks compile library and platform tests but do not link/run native programs. `rustup target add aarch64-apple-darwin x86_64-apple-darwin` installed standard libraries without any Cargo dependency/lock modification.

Actual new Linux paths include: separate captured streams; expected/unexpected nonzero and signal exits; program/root/input/cwd/script drift; sealed-image execution during original-path overwrite; changed Git candidate/target; input change-and-restore limitation; literal non-evaluated script arguments; pre-cancel and in-flight cancel; original monotonic timeout; zero/128-byte capture limits on both streams; inherited same-group pipe cleanup; an explicitly self-terminating generated setsid fixture with escaped pipe reported unknown; env_clear and denied loader environment; safe FIFO/hardlink/symlink/oversize reads; missing/oversized output rejection despite exit zero; exact rule/current output acceptance binding. Escaped descendants are not discovered or killed; the test waits only for its fixture's bounded self-completion.

Mac-specific positive tests execute fixed `/usr/bin/touch`/`false` and the shared script cases. Negative tests reject writable program chains and unsupported sealing. They compile under both targets and **have not run on macOS here**. Existing approved native CI must establish their real results.

Logs are retained locally under ignored `artifacts/command-validators/`:

| Log | Actual outcome | SHA-256 |
| --- | --- | --- |
| library-final-freeze.log | 89 unit/integration + 4 doctests PASS | `1156053fcfc33fc600c35a68a8f920bec9210681aa3aca1dab851714d32a7c7e` |
| clippy-linux-final-freeze.log | all-targets PASS | `1d38b899ce2116bc09940ef8c4ba45cf13ca81d04a23ddf926aa4aae43c39090` |
| clippy-arm-final-freeze.log | cross-target all-targets PASS | `f5686259a33c622bfcfd06663407c95f490170c45994fdb8d6cc0edb25dc7eae` |
| clippy-intel-final-freeze.log | cross-target all-targets PASS | `a080dcd97a8b3c5eb5d2d75f46d4adc86d4424687b8b5e2d4ecdb830bf6c7c5b` |
| fmt-final-freeze.log | PASS, empty output | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| clippy-initial.log | FAIL: redundant unit expression, subsequently removed | `553fc5548a066b2f17c19a8891eb52e296440a2f8bad785a556ada9534643076` |
| rustup-macos.log | two cross-target standard libraries installed | `4ae2551ea6202e5136e5d7344c5cd841fb1b981ec6798b8e0c525a228d335c72` |

Earlier passing logs (`linux-initial`, `linux-platform-refactor`, `linux-scripts`, `linux-git-bindings`, `library-initial`, `library-final`) are retained rather than replacing the history. The final run includes API documentation comments and the escape-fixture release-file handshake; the checkpoint fingerprints specify the exact tested source for independent review.

Scope limits: no Core tables/policies/IPC/native-owner dialogs or UI wiring; no full-task/code/scientific acceptance; no adoption/ref mutation/deployment; no OS filesystem/network/input/descendant isolation; no proof of auxiliary code/native configuration/dynamic-library integrity. Capture limits bound retained pipe bytes, not all filesystem writes. The process timer covers spawn/process observation and separate bounded cleanup, not synchronous artifact/Git preflight or arbitrary kernel stalls. The trusted host must retain unknown Child ownership and must not use a global SIGCHLD reaper. Input and output observations do not form a multi-file transaction or execution-time freeze. No exact-head native CI, signing, artifact delivery or product-complete claim is made by this library unit.

## Independent review F1 and exact-source follow-up

The reviewer generated `/tmp/ab-command-deadline-review/waitid_delay.c` and `.so` to intercept only the explicitly injected parent test-harness WNOWAIT|WNOHANG observation. The actual validator and C compilation use env_clear, so they do not load the shim. On original eec524c, the existing normal fixture with unchanged wall_time_ms1000 ran under a1500ms first-observation delay and still passed both run and accept_command. Original raw log is preserved at `/tmp/ab-command-deadline-review/baseline-delay.log`, SHA `7a90bc7ae2b2456399ffdec2cff5f0d284f8d46761ce759295419a549a3d9e14`; reviewer C source SHA `7d056f0d0975bd0a9cb0de510bae2ea9845983dd689f6215ffed7d7177c56eaf`, compiled local shim SHA `0eff0e99b4f304ce3095c1e5ea511a8d46415c0f233290633332bdbe24e1c814`. This demonstrates late-observation misclassification, not a historical/macOS root cause or a guarantee that syscalls can be interrupted.

The new integration case is `linux::delayed_exit_cleanup_and_cancel_observations_cannot_be_accepted`. It compiles its own harmless fixture shim and runs itself in separate generated test-harness child processes. Before the source repair, this same test failed with `Exited passed=true elapsed_ms=1501 exit=Some(0)` under the unchanged1000ms budget. After repair, actual controls show:

- 1500ms exit observation/1000ms command budget: DeadlineExceeded, passed=false, acceptance rejected.
- 150ms cleanup ownership observation/100ms cleanup budget: CleanupUnknown, passed=false, original Child retained until explicit nonblocking reap; later reap still cannot revive acceptance.
- 150ms observation while cancellation is requested: CancelRequested, passed=false, acceptance rejected.

No command/cleanup budget or original assertion was increased or removed. An initial cleanup fixture selected the second waitid call, which could still be an ordinary in-budget exit poll; its expected-negative assertion failed correctly. That fixture was corrected to delay the next ownership observation after the first positive exit observation. The intermediate failure is preserved rather than described as an independently reproduced production cleanup failure.

Final same-source commands are the commands above with `cargo test --locked -p agentbaton-workspaces`, plus the targeted regression with `--test command_validation delayed_exit_cleanup_and_cancel_observations_cannot_be_accepted -- --nocapture`. Actual final results: **90 unit/integration +4 compile-fail doctests**, zero failed/ignored; Linux +Apple ARM +Apple Intel all-targets Clippy and fmt PASS. Mac checks remain cross compilation only. These source hashes and current counts supersede the initial eec524c freeze; its original logs remain unchanged.

| Local ignored log | Result | SHA-256 |
| --- | --- | --- |
| f1-new-regression-old-source.log | Expected regression FAIL on unchanged old source: late exit PASS | `03419d21cf622a88f16f3e8e5da4ab6e63af6ff323ab47b81128b4cd64ae9336` |
| f1-new-regression-fixed-source.log | Intermediate FAIL: cleanup fixture selected an ordinary in-budget poll | `222860f99b8cea0e1960400c7d96471898cd9b88d51825b5e27d3bd0f6663263` |
| f1-regression-fixed-phase.log | Actual delayed-exit, cleanup, cancel controls all reject PASS/acceptance | `f329581d12548e59c7f050a919fb0a59aeafb58daf2bd4d099da5f0c424e0a93` |
| f1-library-final.log | 90 unit/integration +4 doctests PASS | `e9ae80345406515031d46a095c91215876fd23b4ce684ddf3905a9088d51f664` |
| f1-clippy-linux.log | all-targets PASS | `b75058ad5c90af482d7462b98ed9baac33d777aeaac809d11a7720ce0fc8edd3` |
| f1-clippy-arm.log | cross-target all-targets PASS | `ecd1823da7b6d68e72b50cb2db606e691a52df7c91703c910fa29a8db88fd265` |
| f1-clippy-intel.log | cross-target all-targets PASS | `f104e7132791ab52b696704aa5ecaf7f7d6b155a009a7ef19f0bad98f3321df0` |
| f1-fmt.log | PASS empty output | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

Independent F1 closure is pending reviewer execution of this exact fixed source. No additional Core/native/UI/dependency/lock changes or product/provider operations were made.
