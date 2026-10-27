# Evidence

Base7ba60e0822129d474be1ade845640d1d72e79b21; local Linux x86_64, umask022. Read AGENTS, gate, trusted OWNER001/002, orchestration/security contracts and existing descriptor-based artifact APIs. All fixture data and paths were generated locally; no private credential/auth file was copied or read. No external program is invoked by the production data module. Test-only FIFO uses POSIX libc mkfifo and an owned killable Rust test child.

Every Cargo command used CARGO_HOME=/workspace/.toolchains/cargo, RUSTUP_HOME=/workspace/.toolchains/rustup, CARGO_TARGET_DIR=/workspace/.worktrees/validation-integration/target, CARGO_BUILD_JOBS=2, CARGO_INCREMENTAL=0. Shared-target ownership was coordinated with parent; lib.rs mtime only was touched before compilation to avoid stale cross-worktree rmeta; its content stayed as listed. No new target/dependency or lock modifications.

Actual commands/results:

- `cargo test --locked -p agentbaton-workspaces --test data_workspace`:17/17 pass,0.02s, first compile2.86s. FIFO child repeats the same case under3s owned parent bound.
- `cargo fmt --all -- --check`:pass; `cargo fmt --all` applied only allowed module/test formatting.
- First `cargo clippy --locked -p agentbaton-workspaces --all-targets -- -D warnings`:FAIL101 (four same-type conversion warnings on Linux for Darwin-compatible mode_t conversion). Original log retained. Corrected u32 Unix file-kind mask checks; no lint suppression/guard reduction. Same final command:PASS1.69s.
- `cargo test --locked -p agentbaton-workspaces`:107 unique unit/integration(2 library+27 command+17 data+21 JSON+5 Gitguard+18 safety+17 validation),7 compile-fail doctests allPASS. Base90+4 doctests retained; added17+3. Nested FIFO invocation is not counted twice.
- `cargo check --locked -p agentbaton-workspaces --all-targets --target aarch64-apple-darwin`:PASS1.73s.
- `cargo check --locked -p agentbaton-workspaces --all-targets --target x86_64-apple-darwin`:PASS1.32s. These only compile source/tests for Darwin; no Mac execution/link/package validation.

The17 new cases cover selected-byte copy/explicit unselected exclusion/source permissions+mtime+inode preservation, no implicit Git init, output0600/dir0700, source-independent historical Ready restore after change/move/delete, revision path/ID/bytes binding, changed second input retaining first copy+Failed marker, no automatic retry/overwrite/cleanup, file/root replacement, private/nested destination guard, traversal/auth paths/duplicate IDs/all-pair prefix/case-component aliases, canonical roots/wholeHOME, symlink/hardlink/nonregular/FIFO rejection, exact size/hash/count/total boundaries, same-ID concurrent reservation, unknown owner contents preservation, byte/mode/inode/parent tampering, root/metadata tamper+oversize, disk Ready label nonauthority.

Source/unchanged manifest hashes:

- `crates/workspaces/src/data_workspace.rs`: `ec6cd894ac9410418b324c2b3e3c66d2dd728454c80bc68729deef600f1c97bc`
- `crates/workspaces/src/lib.rs`: `438827e89aa1bbcb2cf74d118e76569eb18e6be94fe1b6253da98db513defe25`
- `crates/workspaces/tests/data_workspace.rs`: `9009b712a494534af4fe0d55db6d94edba97e727f62d58e078c36bc51ac8a7d1`
- `Cargo.lock`: `b22c148dd432426f70586093394eeb9f04c0aeca938764be503e1145659f7d42`
- `Cargo.toml`: `5dc88c14683dc23feac5ee1d6076ee96fc8f4a8eb149fa62cf35818553522c29`
- `crates/workspaces/Cargo.toml`: `303ae03186afbb4da35f45b954efbb91155f6d241c174f130ad89c36df9aa897`

Raw logs remain ignored local artifacts, not fictitious CI attachments:

- `artifacts/data-workspaces/all-tests.log`: `044e03aff7e908d8180543bbcc48da930a0c3e2301f6452173f24cc8bbbd5c69`
- `artifacts/data-workspaces/clippy-initial.log`: `90d4017a30017f3895dab388051b82898c230a48c0ce6adf25b36ef89b41e9ba`
- `artifacts/data-workspaces/clippy.log`: `db73b1679f61422bd380c46a02e04db3ae0cc8adc0fbe7fd17823ce7bb6b04da`
- `artifacts/data-workspaces/darwin-arm-check.log`: `5929edb872f2da72a734b9d83415bab53a44bea8e7c9ca99dce75203eefd69e2`
- `artifacts/data-workspaces/darwin-intel-check.log`: `56f5f59a307c0e0c567db15f702f7fc87f0f5d026c2e2d04ef52b60f069597c1`
- `artifacts/data-workspaces/fmt-initial.log`: `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`
- `artifacts/data-workspaces/initial-tests.log`: `3a766240d8581e99abf1688aa660bea5c1141f770cbd0aaf18f5fd1506350abd`

Limitations: no Core/registry/IPC/leases/UI connection, no product-wide data execution/adoption closure, no TaskAccepted/Git/scientific-correctness receipt, no actual Mac tests/DMGs or new CI claim. No simultaneous whole-directory snapshot/OS sandbox/input immutability assertion; same-UID/privileged actors can change files after the observation. Private tokens require authenticated host decisions and persisted contracts in the future Core integration. No background continuation promised. Independent review pending when this source checkpoint is written.
