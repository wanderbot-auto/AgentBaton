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

## Final independent-review corrections and actual controls

The source hashes/results above are the historical0fba checkpoint. Final source now closes the independently identified marker publication observation and resource-validation-order boundaries. A saved old source contains only0fba semantics+private cfg(test) fault hook and two assertions; it was compiled and actually executed with `cargo test --locked -p agentbaton-workspaces --lib publication_tests -- --nocapture`. Both cases returned a Ready token despite{}-overwritten metadata and failed assertions; cargo101 was expected, not a compile failure. The same production flow with the fix passes both, retaining failure evidence/no verified token; the hook is absent from production builds.

- Old source: `artifacts/data-workspaces/marker-old-source-control.rs` SHA7b606f17fd548b02a6f3ab6ae4dd330e352700719d8b936774c51f446b96baf7.
- Saved actual Linux test binary: `artifacts/data-workspaces/marker-old-source-control` SHA352096ee70d9add96e8c7a7224aa2275e713bc2457b9b5289ea8dbdbcb3cbf9a. It can be independently replayed with `publication_tests --nocapture`, without Cargo or shared target writes.
- Raw old two-assertion FAIL: `artifacts/data-workspaces/marker-old-source-control.log` SHAe7e7115b860fc54c838e9ea12309c05cdf364e8051fb77bf1e3896d080975107.

Final same toolchain/umask022 commands:

- `cargo test --locked -p agentbaton-workspaces`:110 unique unit/integration PASS (4 unit+27 command+18 data+21 JSON+5 Gitguard+18 safety+17 validation),7 compile-fail doctests PASS. Base90+4doc retained; new data20+3doc. The FIFO child is a re-execution of one case, not a111th test. Compile4.20s; test batches0.15/2.22/0.02/0.03/0.53/0.73/4.67s, docs0.08s.
- `cargo clippy --locked -p agentbaton-workspaces --all-targets -- -D warnings`:PASS1.66s.
- `cargo fmt --all -- --check`:PASS.
- `cargo check --locked -p agentbaton-workspaces --all-targets --target aarch64-apple-darwin`:PASS1.36s.
- `cargo check --locked -p agentbaton-workspaces --all-targets --target x86_64-apple-darwin`:PASS1.48s. No native execution or packaging implied.

Final exact source and unchanged manifests:

- `crates/workspaces/src/data_workspace.rs`: `3b248f692a3ae34efcb03dfc4d57601be98aac9a22a812a7e64f007b392b94c6`
- `crates/workspaces/src/lib.rs`: `438827e89aa1bbcb2cf74d118e76569eb18e6be94fe1b6253da98db513defe25`
- `crates/workspaces/tests/data_workspace.rs`: `9286c45c5e739beb5f903fb82e790fcd46de48a3ea22286bfadb15e60d7ec977`
- `Cargo.lock`: `b22c148dd432426f70586093394eeb9f04c0aeca938764be503e1145659f7d42`
- `Cargo.toml`: `5dc88c14683dc23feac5ee1d6076ee96fc8f4a8eb149fa62cf35818553522c29`
- `crates/workspaces/Cargo.toml`: `303ae03186afbb4da35f45b954efbb91155f6d241c174f130ad89c36df9aa897`

Final actual log hashes (earlier evidence/logs retained):

- `artifacts/data-workspaces/final-all-tests.log`: `c9853925c8b937e42e3d2694a9b7f98f9a23380d90dc41098de70500b33a02da`
- `artifacts/data-workspaces/final-clippy.log`: `d741e1f5d2340e18099d346d5fc65ed847d23abcd1c8f67d800e1c1f69c82fe9`
- `artifacts/data-workspaces/final-darwin-arm-check.log`: `9d7a68dbf0006a5a7dc3d1a46a00c32a21aba47cb8f08a3ebf64075aeb1787e0`
- `artifacts/data-workspaces/final-darwin-intel-check.log`: `b92416f02a94dc11d8d1b4f44c4281174d638c09c6155d232387af32109213c0`
- `artifacts/data-workspaces/final-fmt.log`: `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`

Final machine-readable local verification: `artifacts/data-workspaces/verification.json` SHA93422e5cc863f035e60537df22cfb4ba6ef7bbf777c25e48bd62c4a90a0e7602. Historical107+7 verification preserved separately as `verification-0fba.json`. Final independent review is pending; parent controls target access. Existing limits and unimplemented Core/registry/UI integration remain unchanged. No new deps/locks/paid resources/accounts or real Provider calls.
