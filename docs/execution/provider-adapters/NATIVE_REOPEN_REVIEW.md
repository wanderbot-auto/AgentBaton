# Independent native reopened-window review

Reviewer: native `/root/integration_review`, separate execution context. Reviewed the single debug-harness source above root `4f4415f`, final frozen SHA-256 `8eb1d8114d60a86eaabf4d316a9f4d7931b1c463a13b1624dee7ed65af19d31f`, at 2026-10-26T22:25:51.288087+00:00. Only this review document was written; no source, lock, staging, commit, remote CI or permission operation was changed by the reviewer.

Verdict: no unresolved source blocker after the independently identified terminal-state race was corrected. Independent final checks pass: six native-bin units, fifteen Python runner/audit tests, native format and feature/all-target Clippy. The changes are ready for exact-source native CI; they do not prove the old Darwin stall cause or a successful reopened WebView.

## Actual partial native evidence

Independently verified artifact 12690344299 from ARM job 121869957665/run 40407799679 against retained API metadata and ZIP SHA-256 `16f7bcbd9a875e20b1dd0b34f07d028a868104311edfe6540e4b2d77d351f497`. All six safe ZIP member names/bytes match extracted files. The source commit is 4f4415fa62c142fd9ffd5b809f13b1bfbedf3967.

The native report records actual first-window DOM, real product composer submission/render, idempotency, invalid-input rejection and local journal checks, with a matching native app-window screenshot. Viewed that PNG directly: 1136×793, SHA-256 `4375fe2eb4327a9d7895aea0e10680e39639aef01c484ac066dc1d689146a746`. It shows the connected AgentBaton draft created by the harness. Window-close daemon survival, second-writer refusal, daemon restart readiness and owned cleanup are also recorded true. The external runner records actual Darwin all-zombie quiescence handling and cleanup, with exit 1/failed evidence.

The overall job/report remains **FAILED**: the reopened report did not arrive, and the original 60s native watchdog failed. No reopened persistence/cancellation acceptance, native dialog/manual installation or full lifecycle pass is inferred. Original reports lack new construction/page-load stage diagnostics, so the exact historical stall point is unproven. The valid first-window proof remains a partial result.

## Reopen scheduling, locks and permissions

Inspected installed Tauri 2.12.5/runtime-wry 2.12.3 source. Its user Destroyed callback precedes old WindowWrapper/WebviewWrapper removal; main-thread task dispatch executes inline on the main thread but queues through the event proxy when called by the worker. Moving daemon verification/restart off that callback, releasing its mutex before queued main-thread construction, and using fixed distinct `native-reopened` identity addresses a source-supported reentrancy risk. It is not a dynamic reproduction of the old Mac stall.

The worker never waits for the queued window build while holding the daemon mutex. Report-stage locks are released between operations; final joins occur without report/daemon/worker-storage locks held, before taking the daemon for cleanup. Main does not wait on a worker that itself needs a held main-side mutex. Existing JS 10s/native 60s/external 80s/cleanup 3s/screenshot 3s bounds remain; stage records are bounded diagnostics, never success substitutes.

One-shot phase and label checks prevent duplicate reopen scheduling and repeated/wrong-window scripts. Current fixed local custom invokes require no new app ACL permission; SDK plugin/remote-origin restrictions and the empty capability permissions remain unchanged. Production main/host/approval/capability files, Cargo manifests/lock and external runner/scripts are byte-identical to 4f4415f. The separate required-feature debug binary still rejects release builds. No shipping endpoint or generic control channel is added.

## Finding F1: late success could erase failure — closed

The initial frozen implementation used a phase load followed later by unconditional stores 1/3. A watchdog could publish terminal 4 while blocked on the report mutex; a stale handler could subsequently restore 3 and compete to exit 0. Final acceptance originally checked only code 0/phase 3, allowing failed status to be overwritten. This source concurrency/evidence blocker was reported before acceptance; it is not claimed to explain the historical Mac timeout.

The actual handler now invokes a shared report-commit helper under the report mutex, using CAS 0→1 and 2→3. Failure reserves the first reason and publishes 4 before waiting for that mutex, so stale successful commits fail without changing evidence. A stale timeout uses its own CAS loop and cannot overwrite completed 3 or failed 4. Watchdog completion is exactly 3/4; states 0/1/2/5/255 do not stop it. No new intermediate phase is introduced.

Before final acceptance, watchdog and reopen worker are joined outside locks. Early event-loop exit is marked failed before joins. Final acceptance requires code 0, exact completed 3, no separately published failure reason, no error field/failed status, expected checks-pending status and actual owned-daemon cleanup. The real held-report-mutex regression exercises both stale 0/2 commits against the actual failure and commit helpers, proves no mutation, preserves the first reason against a competing generic failure, and rejects final pass. Additional tests cover valid commits, task mismatch, stale timeout after completion and final error/failure predicates. This closes F1 on the final bytes.

## Independent commands and evidence checks

Native commands used umask 022, configured CARGO_HOME/RUSTUP_HOME/PATH, CARGO_BUILD_JOBS=2 and the existing private Linux UI sysroot/pkg/link paths described in NATIVE_CI_REVIEW.md; no system library/sandbox change or Linux GUI launch was used.

| Command | Actual final result |
| --- | --- |
| `cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol --bin agentbaton-native-smoke` | PASS: 6, including real private Daemon admission plus phase/script/concurrent-failure contracts. |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` | PASS, exit 0. |
| `cargo clippy --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol --all-targets -- -D warnings` | PASS, exit 0. |
| `python3 apps/desktop/src-tauri/tests/test_native_runner.py` | PASS: 15; existing runner/audit assertions retained. |
| `git diff --check` | PASS. |

All **32 final evidence-file hashes**, the final source hash and the archived earlier three-unit checkpoint hash match actual bytes. The original three-unit pass did not cover F1; six final units were independently rerun after its correction. Artifact files remain bound to 4f4415f and are not relabeled as results of the new source. No real Provider/model/account, private auth/key, signing, production action or additional paid resource was used. Full Design v0.2 and native/manual acceptance remain incomplete.

| Final reviewed bytes | SHA-256 |
| --- | --- |
| `apps/desktop/src-tauri/src/bin/native_smoke.rs` | `8eb1d8114d60a86eaabf4d316a9f4d7931b1c463a13b1624dee7ed65af19d31f` |
| `artifacts/native-ci-fix2/evidence.json` | `6fa30236a0cdc37655100d2968dfa4f1b7de83fb56def50197040346f522297a` |
| Archived pre-race evidence | `82e24e6569e8c131d774d2ea1cec7171687e8a95821f8943c3bb0045b8336390` |
