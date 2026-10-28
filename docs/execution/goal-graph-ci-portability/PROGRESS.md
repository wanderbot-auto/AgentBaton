# Graph CI portability

Baseline: `6912fa9b72bc3ab9fbc1ddba18cd76e7c052677c`; PR16 Product40733070149 checked out a11045e6a8f72bb5264c1f4cdbf792f51370bdfa. Original CI logs remain `.onboarding/pr16-6912-ci/product-{rust,intel}.log`. OWNER-001/002 and AB-START-001 acknowledged, no permission change.

Only sessions_ipc test fixture code, goal_validator_io cfg(test) code and this unit's four records may change. Production guards, locks, deadlines, required checks and old frozen source remain unchanged. No real Provider/model/account calls, paid resources, remote PR/merge or signing.

Reproduce missing artifact parent and recovery runtime0755 with a clean new worktree and umask022. Validate Scenario shutdown-before-directory-drop lifecycle, replace speed-dependent recursive SQL with controlled real queue blocking, investigate Intel rm program preflight without assuming var/private alias (workspace is already canonicalized). Preserve exact1200ms, stop/join/zeroCandidate/noRefund checks. Run focused old-failure tests and full appropriate Core regression/Clippy/fmt, then independent review and actual future MacCI; Linux cannot prove Mac behavior. Native cache only, jobs2/INC0/testthreads1; explicit lease release at end.

## Verified checkpoint

Source d4ecdf5 changes only the two authorized test files. The `goal_validator_io.rs` production prefix before `#[cfg(test)]` is byte-identical to6912fa9, SHA-2569f78593e5aca462b6e87ff8d47bd0955f57a30ee8df0a04e696f2e1dd838ec36. All other tracked Rust files and Cargo manifests/locks were mechanically compared and remain identical. No product exemption or deadline changed.

Before fixing, a new worktree with no Graph artifact directory and umask022 reproduced: Integrity export NotFound (1 failed,9.84s); recovery runtime permission rejection (1 failed,3.69s); proposal test on its own daemon shutdown INPUT_CHANGED (1 failed,1.75s). Logs are in `.onboarding/goal-graph-ci-portability/` with suffixes `clean-artifacts-old-fail`, `umask022-recovery-old-fail`, `proposal-drop-old-probe`. Original CI logs are retained separately. Earlier patch compile E0509 and the correctly refused NativeExecutable shell-c experiment are also retained, not called product defects.

Fixes are test fixture changes: create the Graph artifact parent at every exporter; declare Scenario.server before Scenario._dir so normal Rust field destruction joins the daemon before deleting its workspace, while keeping the original shutdown/join errors visible; create the recovery runtime0700 and its newly generated SQLite backup0600; replace the speed-dependent4m recursive-query delay with an integration-test-only SQLite function that actually blocks1500ms in the capture-preparation transaction. A scoped auto-extension registers the function before test connections and unregisters after Scenario cleanup. Actual call count1 and monotonic elapsed>=1500 are checked; approved1200ms/RPC5s, actual producer stop, no IO thread/join/candidate, unknown output and consumed1/no refund assertions stay intact.

Targeted actual Linux results: controlled queue1PASS3.33s (delay1500ms, actual attempt1822ms, no capture started); independent proposal shutdown1PASS1.56s; private recovery1PASS3.97s; all11 private IO tests PASS1.43s; Core all-target Clippy-Dwarnings PASS9.15s. The deletion test uses the library's existing PosixShStdin mode with no argv and a literal `exec /bin/rm report` script whose bytes/root/SHA/pin are bound. It really deletes the report, returns a CommandRun, and confirms its owned leader is reaped. Original300ms insufficient-budget rejection remains. The original system-rm failure is not claimed fixed: actual canonical path/root metadata/dev/inode/nlink/mode/SHA/held-versus-path pins and a program-only FD preflight outcome are logged without launching that original command; next MacCI must establish its host-specific result.

All earlier local artifact files were moved, not deleted, to `.onboarding/goal-graph-ci-portability/artifacts-before-full-check`; a fresh artifacts directory and umask022 are used for the full modified-domain run (lib69 and sessions55). Current batch is in progress. After it ends, the recovery test's final path will use the already canonical f.root instead of f._dir.path to avoid a prospective Mac/var alias; that one-line byte difference and its focused recheck will be recorded separately. This is not a claim of Mac execution. No Cargo target is released while the batch is active.

## Final author handoff

Final source: `a2c48b7e399ab298047e838f2d6c4475de6f8a4f`, branch `codex/goal-graph-ci-portability`. The complete modified-domain run used `d4ecdf561f056840b40c41dba80c9f2c653ca92a`: Core library69 PASS40.47s, sessions IPC55 PASS196.52s, zero failed/ignored, umask022 and a genuinely fresh artifacts directory. The only later source delta is the recovery fixture's one-line `f._dir.path()` → already canonical `f.root`; the final source passed the focused recovery test1/1 in3.96s. Core all-target Clippy `-D warnings` passed1.03s; fmt/diff checks passed. Counts are124 unique tests, plus one repeated focused recovery test; do not report125 unique.

Immutable verification record: `/workspace/.onboarding/goal-graph-ci-portability/FINAL_SOURCE_VERIFICATION.json`, SHA-256 `05b0859ea8e6f58ed3d139ccaa0cf89c1c4d0655d836bea842e90f791df2334e`. It includes before/after tracked Rust/Cargo/lock maps, the exact one-line post124 delta, original CI log hashes and every local failed/passed log hash. Final sessions test SHA-256 `f884a060b61b689bffd7487798b7b0a77b5ffc2e3910085490ce44d55d435440`; IO test-module file SHA-256 `bae0720b9324dbd2634f05b39dd42d472da652480288ab9e621b65c44b286f57`. The production IO prefix remains identical to baseline, SHA-256 `9f78593e5aca462b6e87ff8d47bd0955f57a30ee8df0a04e696f2e1dd838ec36`. All other Rust files, Cargo manifests and locks are unchanged.

Commands used the existing Native cache, CARGO_INCREMENTAL=0, CARGO_BUILD_JOBS=2, RUST_TEST_THREADS=1, and `umask 022`:

```sh
cargo test -p agentbaton-core --locked --lib -- --nocapture
cargo test -p agentbaton-core --locked --test sessions_ipc -- --nocapture
cargo test -p agentbaton-core --locked --test sessions_ipc actual_graph_interrupted_validator_snapshot -- --nocapture
cargo clippy -p agentbaton-core --all-targets --locked -- -D warnings
cargo fmt --all -- --check
git diff --check
```

The Native cache was explicitly RELEASED to Root with no live Cargo/rustc/test writer. The old PID666879 is an inactive zombie from the separately documented earlier task, not a live worker or writer. Author source/locks are frozen and only these four records are updated after release. No remote push, PR, CI trigger, merge, new cache, production change, credential, model call or added fee was performed.

Independent review and the next exact-head MacCI are still required. Both original ARM and Intel system-rm failures are preserved; Linux confirms the replacement fixed stdin-script deletion and ownership checks, not the original Mac system-rm preflight cause. New Mac output must inspect the recorded actual program metadata/FD-only probe and run all assertions. No original syscall/program guard, runtime permission check,1200ms reservation,5s RPC,300ms insufficient budget check, stop/join/candidate/refund assertion or required CI check was weakened. Installer/OS dialog/build steps not reached by the failed original Mac runs remain unverified by those runs.
