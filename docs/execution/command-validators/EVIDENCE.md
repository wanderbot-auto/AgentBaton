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

Final Linux tests: **89 unit/integration + 4 compile-fail doctests**, zero failed/ignored. Breakdown: 2 existing unit + 26 new command + 21 existing deterministic-data + 5 existing Git-security + 18 existing workspace + 17 existing validation/adoption. Two new compile-fail tests prove the command rule and command-only acceptance are not Deserialize. Both Apple target all-targets Clippy checks compile library and platform tests but do not link/run native programs. `rustup target add aarch64-apple-darwin x86_64-apple-darwin` installed standard libraries without any Cargo dependency/lock modification.

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
