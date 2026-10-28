# Data path fixture evidence

Source `893efe5aa1dd89844de6d4df00d90d736d2b5e8a`, baseline `37bd032f0e1b2dfa75fc9ff768e85801f1693a99`. Linux only author verification, requirement contributions AB-PRJ-002/005, AB-SEC-005, AB-REL-001 partial. Only `crates/workspaces/tests/data_workspace.rs` changed; SHA-256 `f1aa9b30b601becab1a9b52e50c090a585f6becfbe4c5a80e1c963ed75b8d77b`. No product source,4096 limit,dependency,lock,timeout,skip or permission change.

All evidence is under `/workspace/.worktrees/data-path-fixture/artifacts/data-path-fixture/`. Manifest SHA-256 `2a1228346d7c6c8b8189e848779162a5157606b89e3385d0f74c97f111f5caa1`; workspace source map SHA-256 `91176b9c44b20fa1ecfe0474e23170d867abb20ef6786ceea67488708e32f6fb`. `mechanical-scope-check.json` proves all24 test names unchanged, all bytes outside target test/other23 functions identical, original16-component calculation and original target>4096/ResourceLimit/managed-root0 assertions identical. Production source bytes remained original, including entrypoint before/after mtime touch SHA recorded in baseline.json.

Actual commands use CARGO_HOME=/workspace/.toolchains/cargo, RUSTUP_HOME=/workspace/.toolchains/rustup, CARGO_TARGET_DIR=/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target, CARGO_INCREMENTAL=0, CARGO_BUILD_JOBS=2, RUST_TEST_THREADS=1. Serial commands only; main target untouched.

| Actual command | Log | Result |
| --- | --- | --- |
| cargo test -p agentbaton-workspaces --test data_workspace full_internal_target_path_budget_is_rejected_before_reserving_any_directory -- --test-threads=1 --nocapture (baseline) | linux-old-boundary.log |1 PASS; Linux already supported old fixture, does not reproduce macOS error |
| cargo test -p agentbaton-workspaces --test data_workspace -- --test-threads=1 --nocapture | data24-1.log |24 PASS0.06s; all original names retained |
| cargo test -p agentbaton-workspaces --lib validation::root_binding_tests -- --test-threads=1 | root-fd-control.log |1 PASS; real opened-root descriptor check |
| cargo clippy -p agentbaton-workspaces --all-targets -- -D warnings | clippy1.log |PASS, no suppressions |
| cargo fmt --all -- --check | fmt.log |PASS |
| git diff --check | author tool output |PASS |

Actual fixture observation:16components, relative4015bytes, source-full4038bytes, complete target4132bytes>4096; real verified snapshot11bytes/hash `01cadfa37f70dd32b86cbdadd173ff1b0bf744b79ae6f12c1a56745c26bcb1f1`. Product returned exactResourceLimit before reserving any managed directory (count0). Local helper walks only individual component names through actual directory FDs with mkdirat/openat, NOFOLLOW/DIRECTORY/NONBLOCK/CLOEXEC/private modes/exclusive new leaf. It writes the original fixed input, not a forged snapshot.

Frozen test executable `/workspace/.worktrees/data-path-fixture/artifacts/data-path-fixture/data_workspace-test-elf` SHA-256 `9f46cc199ff63c2019dc68a69aea3f82d89ed8e8debd439c6e628e131e292934` enables independent direct execution without another target writer. This is a test ELF, not a product or installer artifact. Native target has been formally released to Root; independent reviewer notified, author performs no more Cargo writes.

Original two-Mac logs copied unchanged as old-arm-job-122751238994.log and old-intel-job-122751238886.log, with exact hashes in baseline.json/manifest. Both failed at Fixture.put full-path create_dir_all (macOS Os63) before this product boundary assertion; original Data Core21 had already passed. No after-fix macOS result or independent verdict is claimed here. Parent must run new CI and read actual results before signing off Mac support. No remote push/PR/CI/merge, Provider/model/account/credential or extra-cost action was taken by this leaf.
