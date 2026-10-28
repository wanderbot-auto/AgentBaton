# Data long-path fixture independent review

**PASS for the narrow fixture correction and independent Linux verification.** No blocking code finding. macOS after-fix remains **NOT_RUN** and must be established on a new exact CI subject before acceptance; this review does not authorize merge of the failed37bd subject.

Baseline `37bd032f0e1b2dfa75fc9ff768e85801f1693a99`; frozen source `893efe5aa1dd89844de6d4df00d90d736d2b5e8a`; author docs `e3949be42ad991f8a2a042540257757fbeebc4d1`. The only source commit file is `crates/workspaces/tests/data_workspace.rs`, SHA-256 `f1aa9b30b601becab1a9b52e50c090a585f6becfbe4c5a80e1c963ed75b8d77b`. Exact before/after source is retained. Author manifest SHA-256 `2a1228346d7c6c8b8189e848779162a5157606b89e3385d0f74c97f111f5caa1` plus every source/log/ELF it references was independently rehashed successfully.

## Semantic and mechanical review

All24 original test names remain. The other23 complete top-level test functions and every byte outside the changed target are identical. The original16-component length calculation, component cap250, source path target4090, creation ID80 bytes, full target `>4096`, exact `Some(DataWorkspaceError::ResourceLimit)` and final empty managed-directory assertions are unchanged. No production source, threshold, timeout, skip, dependency, Cargo.lock or CI file changes. `static-proof.json` is independently computed from exact Git objects, not copied author conclusions.

The local helper opens the existing source root with O_DIRECTORY/O_NOFOLLOW/O_NONBLOCK/O_CLOEXEC, then performs mkdirat/openat one normal component at a time. It uses O_CREAT|O_EXCL and mode0600 for the new leaf, mode0700 for directories; each successful descriptor becomes one RAII-owned File and prior directory handles close on replacement. This avoids passing the complete approximately4KiB fixture path to the OS during construction. It still writes real fixed bytes, calls the original product snapshot traversal, and evaluates the original target-path refusal. Added assertions require16 components, one snapshot file, exact relative path, expected hash and actual11byte total. This preserves the safety acceptance contract and strengthens fixture evidence rather than reducing its boundary.

37bd's actual ARM and Intel Os63 failures occurred in the former `Fixture::put` create_dir_all source setup before the product refusal assertion. That history and raw hashes remain immutable in `data-native-37bd`; the corrected fixture has not yet been observed on Mac. No old Linux success is called a Mac reproduction.

## Actual independent commands

Root explicitly granted sole use of the released Native target for this review; Main remained with another writer. I recompiled against `/workspace/.worktrees/data-path-fixture` using existing toolchain homes, CARGO_TARGET_DIR `/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target`, CARGO_INCREMENTAL=0, CARGO_BUILD_JOBS=2 and RUST_TEST_THREADS=1. Offline locked Cargo calls were:

- `cargo test --locked --offline -p agentbaton-workspaces --test data_workspace -- --show-output`: **24 passed, zero failed/ignored**, actual0.05s test body. A child-process invocation overlaps one original test and is not a25th unique Data test. Raw SHA-256 `4be92281421940676ae7cb5acd28342eed2742d7b3c3ff78ed3bbdf8ff05361c`.
- `cargo test --locked --offline -p agentbaton-workspaces --lib validation::root_binding_tests::opened_root_descriptor_is_checked_instead_of_trusting_an_earlier_path_stat -- --exact --show-output`: **1 passed**,6 unrelated library tests filtered by the explicit scope. Raw SHA-256 `612a81b2c52b98031b873621abcb347749e6a5eafc271dad7d32b9cf93abb165`.
- `cargo clippy --locked --offline -p agentbaton-workspaces --all-targets -- -D warnings`: **exit0**, raw SHA-256 `4ef80863e5d4f223b1b277692f3c2668bc5ab27c8bf0935f57a280c913a49e4b`.
- `cargo fmt --all -- --check`: **exit0**.
- `git diff --check`: **exit0**; final working tree and index clean.

Independent emitted observation:16 components, relative4015 bytes, full source4038, full target4132, snapshot11 bytes, snapshot SHA-256 `01cadfa37f70dd32b86cbdadd173ff1b0bf744b79ae6f12c1a56745c26bcb1f1`, `creation=directory_fds`. The product ResourceLimit and no-reservation assertions then passed. The independently rebuilt test ELF SHA-256 is `9f46cc199ff63c2019dc68a69aea3f82d89ed8e8debd439c6e628e131e292934`; rebuild logs show this exact worktree, not another cached source path.

The initial inherited PATH lacked `cargo`; that process never started. `initial-command-unavailable.json` preserves this environment failure. Retry used the existing authorized `/workspace/.toolchains/cargo` and rustup directories, without dependency download or changing tests. Entry-point mtime touches on lib.rs and the test file were byte-checked before/after in `mtime-byte-invariance.json`. No source or index bytes changed.

## Limits and handoff

All Cargo/check processes exited and the Native target is explicitly **RELEASED** to Root; no further Cargo writer is active from this reviewer. No production-source or permission change, CI trigger/rerun/cancel, push/merge, external credentials/account, paid API or subagent was used. This proves the fixture and existing guard on Linux; it does not prove the Mac fix, native22/12, real Owner dialog, new screenshots, default release audit or development packages. Require a fresh exact seven-check CI run and current dual Mac native/audit/package evidence; never borrow37bd or prior green artifacts.
