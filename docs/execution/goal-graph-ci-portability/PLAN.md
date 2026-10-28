# Graph CI portability

Baseline: `6912fa9b72bc3ab9fbc1ddba18cd76e7c052677c`; PR16 Product40733070149 checked out a11045e6a8f72bb5264c1f4cdbf792f51370bdfa. Original CI logs remain `.onboarding/pr16-6912-ci/product-{rust,intel}.log`. OWNER-001/002 and AB-START-001 acknowledged, no permission change.

Only sessions_ipc test fixture code, goal_validator_io cfg(test) code and this unit's four records may change. Production guards, locks, deadlines, required checks and old frozen source remain unchanged. No real Provider/model/account calls, paid resources, remote PR/merge or signing.

Reproduce missing artifact parent and recovery runtime0755 with a clean new worktree and umask022. Validate Scenario shutdown-before-directory-drop lifecycle, replace speed-dependent recursive SQL with controlled real queue blocking, investigate Intel rm program preflight without assuming var/private alias (workspace is already canonicalized). Preserve exact1200ms, stop/join/zeroCandidate/noRefund checks. Run focused old-failure tests and full appropriate Core regression/Clippy/fmt, then independent review and actual future MacCI; Linux cannot prove Mac behavior. Native cache only, jobs2/INC0/testthreads1; explicit lease release at end.

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
