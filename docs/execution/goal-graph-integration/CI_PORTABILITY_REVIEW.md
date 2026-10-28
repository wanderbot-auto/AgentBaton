# PR16 CI fixture portability independent review — a2c48b7

**PASS for the narrow test-fixture patch and independently executed Linux controls. No remaining blocking source finding in this patch.** Actual new PR16/macOS CI remains pending; the original Intel system-rm preflight root cause is not established. No production, permission, timeout, lock or required-check change is accepted by this report.

## Frozen source and exact boundary

Base `6912fa9b72bc3ab9fbc1ddba18cd76e7c052677c`; final `a2c48b7e399ab298047e838f2d6c4475de6f8a4f`, isolated `/workspace/.worktrees/goal-graph-ci-portability`. Author manifest SHA `05b0859ea8e6f58ed3d139ccaa0cf89c1c4d0655d836bea842e90f791df2334e` was independently rehashed. All104 tracked Rust/Cargo/lock files were checked before/after; only two allowed files differ from base:

* sessions_ipc.rs SHA `f884a060b61b689bffd7487798b7b0a77b5ffc2e3910085490ce44d55d435440`.
* goal_validator_io.rs SHA `bae0720b9324dbd2634f05b39dd42d472da652480288ab9e621b65c44b286f57`; every change is inside its cfg(test) module. Production prefix is byte-identical to base, SHA `9f78593e5aca462b6e87ff8d47bd0955f57a30ee8df0a04e696f2e1dd838ec36`.

All other Rust/manifests/locks and all production guards/deadlines remain byte-identical. Immutable a2 copies of both allowed sources and the full source maps are retained. Original70a/Linux593+7 report was not edited and is not promoted to Mac/CI success.

## Actual independent execution

Root explicitly granted the sole existing Native cache after author release. Same author tree, no new cache, CARGO_INCREMENTAL=0/jobs2/RUST_TEST_THREADS1, cargo --locked. Each of the seven affected sessions scenarios ran separately with umask022 and a genuinely absent artifacts parent. Original author artifacts were temporarily renamed, never deleted; each run's new artifacts were separately preserved, then original author directory restored. Full commands, timings, hashes and zero exits: results.json/run.py/fixture-restoration.json.

| Independently run final-source control | Result |
| --- | --- |
| Capture prestart queue exhaustion |1PASS;3.35s including Cargo |
| Complete new Graph proposal and normal fixture shutdown |1PASS;1.69s |
| Owner→dependent Summary→durable TaskAccepted portable export |1PASS;7.74s |
| Integrity Validator and Owner/Summary export |1PASS;11.10s |
| Data Validator and Owner/Summary export |1PASS;12.29s |
| Fixed Command observation/ledger and export |1PASS;11.53s |
| In-flight private recovery snapshot unknown/fence/no replay |1PASS;3.99s |
| IO module, including post-command failure, pins, links/FIFO, deadline and cancellation |11PASS;1.49s |
| cargo fmt --all -- --check |PASS;0.76s |
| cargo clippy --locked -p agentbaton-core --all-targets -- -D warnings |PASS;0.12s cached check |

Total independent18 tests, zero failed/ignored. These overlap the author's domain regression and are not added to its124 as unique product tests. Author lib69/40.47s and sessions55/196.52s logs belong to d4ecdf5; final a2 adds only the canonical-root recovery line, which author separately reran and this reviewer independently reran. This report does **not** call all124 exact-a2 execution. No repeat of unchanged Node/Native/fullworkspace was claimed.

## Failure causality and retained safety assertions

1. **Clean artifact output:** CI's four NotFound errors were fixture JSON export parent writes. Shared test-only helper now creates that parent before path construction. Existing JSON serialization, SHA filenames/create_new where used, real runtime assertions and evidence bytes are not replaced by mocks or skipped. Each independent successful export starts from no artifacts parent, removing earlier test ordering dependence.
2. **Private recovery:** a default0755 recovery directory was correctly refused by Daemon::bind. New DirBuilder0700 creates the owned fixture path, VACUUM backup is0600, and a2 uses already canonical f.root (important for Mac path aliases). No runtime permission check changed. Original umask022 PERMISSION_DENIED failure is preserved. Recovery still uses a consistent in-flight DB image with original process actually stopped/joined; it is not a SIGKILL test.
3. **Queue threshold:** CI actual elapsed1085ms was below the unchanged1200ms reservation, so candidate creation was not evidence of expired-job acceptance. Fixed CPU workload was replaced with a test-process SQLite function invoked by the actual prepared-job transaction. Independent final run measured **one real1500ms callback and actual_runtime1767ms**, then blocked with no candidate/no capture worker start/no fake join/consumed1. The original1200 reservation and5s RPC bound remain, as do producer stopped/completed and unknown actual output assertions.
4. **Auto-extension containment:** callback/RAII registration exists only in sessions integration-test executable. Direct FFI registers a zero-argument function on its SQLite connections; it does not enable dynamic extension loading or modify product SQL. Callback uses actual monotonic sleep and bounded atomics, no fabricated clock. Guard is created before Scenario and therefore drops after all test DBs and Scenario/server teardown; sqlite3_cancel_auto_extension unregisters its exact function pointer. Test-specific trigger is confined to its temporary DB. Production binaries do not contain this integration fixture function. Other tests and production database have no new function/trigger authority. The final run and author's complete sessions55 exercise normal teardown after this case; no separate OS-fault induction of SQLite unregister failure is claimed.
5. **Lifecycle:** old proposal local reproduction shows daemon.run INPUT_CHANGED then join panic during Scenario drop. Moving server before _dir in declaration order makes actual daemon shutdown+join precede root removal; no join/daemon error is swallowed. Struct initialization/destructuring remains compatible, unlike adding a new Drop implementation around existing move destructures. Final proposal independent run passes through real server Drop with no panic. The original failure remains in supporting author logs.
6. **Post-command IO:** Intel at949 returned this module's pre-command INPUT_CHANGED; both fixture root and program were already canonicalized, so alias was never proven. Patch adds original system-rm path/held metadata and genuine program-only FD-preflight diagnostics without launching it. On Linux these diagnostics are supporting evidence only; future actual Mac log is required to determine original rejection. Root explicitly authorized changing this fixture's execution shape. The deletion test now uses the existing approved PosixShStdin mode, empty argv, fixed `exec /bin/rm report` script with exact SHA/root/file pin. NativeExecutable shell-c was correctly rejected in an earlier attempted fixture and that failure is retained; no shell restriction is bypassed. Final independent test actually deletes report, gets INPUT_CHANGED in postprocess, returns retained owned CommandRun with process_started+owned_leader_reaped, passed=false. Original300ms insufficient-budget DEADLINE_EXCEEDED assertion remains and passes on the unchanged pre-spawn budget branch. Program, script, input/output nofollow/nlink/root/pin guards remain unchanged; script execution does not become TaskAccepted.

## Evidence, limits and lease

Original CI checkout `a11045e6a8f72bb5264c1f4cdbf792f51370bdfa` / Product40733070149 remains failed in INITIAL_BINDING/INITIAL_REVIEW. Original CI7 sessions failures and Intel1 failure, clean-directory/umask/Drop local failures and the rejected NativeExecutable shell-c attempt remain retained and separately hashed. New passed logs do not erase or relabel them.

This source patch is ready for exact new-head CI; it does not prove both Mac architectures, the original system-rm failure explanation, NSAlert/macOS GUI, installers, real Provider/model, or full product completion. Those remain required separate results. No PR, Approve, merge, CI invocation, account, fee, signing, production publication or product-source edit was performed by this reviewer.

All104 source/lock hashes remained identical before/after independent tests. All subprocesses completed and were waited. Original author artifacts restored. Reviewer explicitly RELEASED sole Native cache to Root and notified author; no further Cargo/target writers from this reviewer. Report/evidence-only writes after that boundary.
