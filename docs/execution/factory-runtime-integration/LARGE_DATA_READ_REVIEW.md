# Factory large Data owned-read controls — independent limited review

Verdict: no unresolved must-fix found within this tests-only, synthetic Linux read-stall delta. This is not formal approval, an independently executed runtime result, or complete Factory/product acceptance.

Frozen test source `259b2f7bc8b3e98e230e3c158992a93efab99c02`, base `265036ccf77771e066bddfd0a5f0768efbe0cd0b`. Only `crates/core/tests/factory_runtime_ipc.rs` and two new `tests/fixtures` source files change. All 46 production bindings remain byte-exact to the previously sealed db949/CAS candidate; no schema, locks, production hooks, DTO, resource limit or authorization changes. Source snapshot SHA-256 `29716cca9cff6f7536259f3feebd11fdee87de20b86c03d436eb819e3477664d`.

## Reviewed behavior

The Rust fixture compiles a private C shared library with an explicit cleared process environment and `/usr/bin:/bin` compiler lookup. Only its own generated daemon's `Command` receives `LD_PRELOAD`; there is no global environment mutation or Cargo/rustc preload. The generated Provider initializer explicitly asserts the variable is absent, and the real Provider process builder clears its inherited environment before installing approved configuration. This does not purport to inventory all unrelated host processes.

The interposer first performs a real `read`. Only an actual 65,536-byte return from the exact selected regular single-link file can claim the gate: effective UID, private mode 0600, device, inode and total 16 MiB size are checked. Control directory is private 0700 and identity/marker files are private. The test holds its own no-follow file descriptor and matches the recorded device/inode to it; the marker records original daemon PID and a different TID. It therefore distinguishes an actual off-actor in-flight observation from an already-ready race or just an artificial pre-read sleep. This deliberately pauses userspace after the kernel read has completed; it does not interrupt a blocked kernel syscall.

Three independent cases use real generated ACP/fixed-MCP sessions and real Owner Unix IPC. A successful scoped request is required before arming. While the read is held, a second scoped read and another proposed Owner read must refuse with `WORKSPACE_BUSY`, the original pending reply must remain unresolved, the recorded thread must remain alive, and the project active slot and workspace lease must both remain exactly 1. Owner history/status must respond in less than the existing 3-second bound. Each case then applies one trigger: Owner cancel, Owner scope revoke, or the genuinely approved 4,000-ms scope expiry. Revoke and expiry issue no Owner cancel. The fixture retains the original 5-second waiting bounds and production deadlines; its 10-second emergency interposer release causes a failing assertion rather than a passing outcome.

After explicit release, the original client response must have the original nonce, an error and no result. The gate has actually returned, the original worker TID is gone, and the existing production manager joins the owned data thread before making its retained admission result available. A subsequent loop requires the original process-reaped session receipt, stopped dispatch intent and active zero before accepting terminal cleanup. Lease count is then zero, consumed attempts remain 1, Factory attempts remain 1, and no delegation/child is created. The original source dataset is unchanged. Finally the fixture sends SIGTERM only to its still-owned unreaped daemon Child, waits on that same Child and requires exit status 0. These are different proofs: read-worker join, original Provider leader reap and daemon process exit are not interchangeable, nor proof of arbitrary escaped descendants or known side effects.

## Exact final author evidence reviewed

Only the final three artifact directories selected by the frozen snapshot are used for current-source runtime claims:

| Trigger | Status ms | Trigger RPC ms | Held active / lease | Explicit Owner cancel | Approved scope TTL |
|---|---:|---:|---|---|---:|
| Owner cancel | 26 | 23 | 1 / 1 | yes | 30000 ms |
| Owner revoke | 25 | 30 | 1 / 1 | no | 30000 ms |
| Scope expiry | 22 | not applicable | 1 / 1 | no | 4000 ms |

All three report `CANCEL_REQUESTED`, exact original nonce, 65,536 bytes read before stall, original worker gone after the response, actual original leader reaped, one consumed attempt/no child, and own daemon exit 0. Author command `cargo test --locked -p agentbaton-core --test factory_runtime_ipc actual_large_data_read_ -- --nocapture` passed 3 distinct cases, 0 failed/ignored, 50.28 seconds. Core all-target Clippy `-D warnings` passed after removing the redundant argument (0.44 seconds); format and diff checks passed. Existing product binaries are unchanged; a fresh product build was not needed or claimed for this test-only revision.

All 125 declared artifact hashes, three frozen test-source hashes, 46 production-source hashes and both immutable raw daemon/CLI hashes were independently verified. Each final retained generated C source and helper library matches its actual per-case build record; compiler version/argv are saved. The compiler executable hash was collected after the run and checked against the current resolved executable; it is expressly not a pre-execution compiler-image pin. Each temporary stripped installed daemon's identical SHA is recorded in its build packet, but the temporary images have been removed with the fixture directory and were not independently rehashed by this reviewer. Immutable raw tools remain available and were rehashed.

## Preserved earlier evidence

The first cancel-only run passed 16.89 seconds at an earlier fixture revision. The initial three-case compile failed with E0425. An intermediate three-case run passed 51.19 seconds but explicitly cancelled after revoke/expiry, so it does not establish their independent behavior. The next strict trigger run passed 50.82 seconds before a parameter-only cleanup; Clippy then failed on 8/7 arguments. Final 50.28-second evidence is the selected, current-source run. These logs and the saved intermediate source remain intact, are separately hashed, and are not summed into extra distinct tests or relabeled as final-source passes.

## Independent work and limits

Reviewer executed only Python static/hash assertions and read git/files. No Cargo, compiler, helper library, test executable, LD_PRELOAD process, daemon, browser, remote operation or target mutation occurred. This review acquired no Cargo/CPU lease. Repository source/locks/schema/STATUS were not modified.

Not established by this delta: kernel syscall interruption, macOS behavior, original authorized ReadyRequest queue deadline, large capture/preflight/terminal-journal failures, full unknown/restart matrix, whole process-tree containment, complete current workspace regression, real Vendor/model/identity, UI/Native approval or full Factory acceptance. Metadata/stall and data-size performance generalization remain bounded by exactly the synthetic case above.
