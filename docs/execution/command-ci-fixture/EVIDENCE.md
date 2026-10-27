# Fixture CI and controlled timing evidence

Original actual failed CI: Product40446762573, Intel121999784372, source merge56ff4227f2bae71fdbf377fbd07c3af7bcd6e403 of PR12 head7ba60e0. Raw `/workspace/.onboarding/command-7ba-intel-job-121999784372.log` SHA7bb109695acb3fbf3018eb4f793dad9fd806a23294ea6c5f4d57152bc99b76e3. Existing actual lifecycle test failed at generic call line955 with PERMISSION_DENIED 'Execution grant or an ancestor is revoked or expired', side_effect_state not_started, journal revision17.13 command cases passed/1failed in that job; Intel native/build/package stages were not reached. Other jobs succeeding does not make Intel verified.

Managed Linux local commands used CARGO_HOME=/workspace/.toolchains/cargo, RUSTUP_HOME=/workspace/.toolchains/rustup, PATH with Cargo bin, existing CARGO_TARGET_DIR=/workspace/.worktrees/validation-integration/target, CARGO_INCREMENTAL=0, CARGO_BUILD_JOBS=2, umask022:

```sh
cargo test --locked -p agentbaton-core --test command_validation actual_reaped_failed_acp_lifecycle -- --nocapture
cargo test --locked -p agentbaton-core --test command_validation
cargo clippy --locked -p agentbaton-core --all-targets -- -D warnings
cargo fmt --all -- --check
```

The first targeted command was run on old order with real1100ms task preparation. Actual result FAIL101: tasks.create prep1100/RPC11/total1111ms, approved return→admission request1111ms, sessions.create RPC11ms then expired rejection. Original1000 grant/3sec waits remained intact. The old Linux test executable is preserved before rebuild (69MiB), SHA c0ecfd248792f6e81a06affa2dd9acf51409881fc49565f7ae0987984070a144. Replay it directly with `--exact actual_reaped_failed_acp_lifecycle_can_approve_independent_command_while_task_result_stays_unknown --nocapture`; no source reversal/new target is necessary.

Fixed same-delay positive: tasks.create prep1100/RPC11/total1111ms occurs before final grant. Explicit expired control then waits1100ms after its1000ms approval; sessions.create rejects in7ms with no session row, no attempt row, identical holder/fencing token and no ACP initialize/session-new marker. Fresh approval is performed last: TTL remaining999ms, grant-return→admission-request0ms, sessions.create225ms. That actual local admission did not exceed1000ms. Generated account-free executable really receives initialize then session/new, malformed session startup fails, trusted stop journal confirms original process reaped and no_process_started=false, intentStopped/lease release/TaskOutcomeUnknown remain, and independently approved fixed-command validation passes without accepting the task. No model/prompt/account call occurs.

Final targeted control and all14 command integration cases PASS (0failed/ignored); all-targets Core Clippy and formatting PASS. Intermediate fixture additions initially used unsupported rusqlite u64 extraction, then queried a non-existent aggregate_id SQL column; both failures are preserved and corrected to i64/u32 and json_extract without changing production or any budget/assertion.

Local ignored logs in artifacts/command-ci-fixture:

| Artifact | Result | SHA-256 |
| --- | --- | --- |
| old-order-task-prep-delay.log | old sequence controlled expiry FAIL101 | 9f7aace12d1d6f6d9cd966a69383377f234fa684aa13fea733797bc4a28bf971 |
| old-order-control | independently saved old Linux test harness | c0ecfd248792f6e81a06affa2dd9acf51409881fc49565f7ae0987984070a144 |
| fresh-grant-order-control.log | intermediate added u64 SQL extraction compile failure | ab2ee6425440089593fe0ac276b9cf1785772d1a662a2e9a6dc2527718498c20 |
| fresh-grant-order-fixed.log | intermediate added journal SQL query failure after valid admission232ms | 99dd14f15dc5077dfa90320500ded08fdc203eed854c8c00cacfd4913fbde465 |
| fresh-grant-order-verified.log | final old-expired negative/new-fresh positive actual lifecycle PASS | a68ad1de203b91c00a1853df961f1cdb73a149354b5ce9d946a0d86399675e18 |
| all14-final.log |14 command tests PASS | 9fab92a1b40f532b3b9cbbfaeb25cae21539382bbce1a26927724031cbdf1a67 |
| clippy-final.log | Core all-targets -Dwarnings PASS | 81601b092436cfcc02d0f3da5c05e14350eebad6006266896a19df4c15360da1 |
| fmt-final.log | PASS empty output | e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 |

This controlled Linux reproduction proves that delayed fixture preparation can consume the old short grant, and reordered preparation avoids that delay after approval. It does **not** prove the exact historical Intel cause or that the new candidate passes Mac under its actual admission load. Any future per-method admission longer than the original1000ms must remain a real failure and be diagnosed, not hidden by extending timeout. Exact new dual-architecture CI/native/package evidence remains pending. Existing Environment checks/gates were not altered. Independent reviewer verdict is pending at implementer freeze.

## Subsequent actual Intel run: preparation reordering was insufficient

Actual PR12 candidate201fba91597d475d4ecce0c64f15106e17382d27 Intel job122010945047 failed12passed/2failed. Raw `/workspace/.onboarding/command-201f-intel-job-122010945047.log` SHA d8db5e19561f2af8c2b82df36eda538b8614dac49944efbde62d3e4847fbfa6d was read directly. Fresh grant remaining992ms and request gap0ms were real, but sessions.create itself took1232ms, above the unchanged1000ms window. The resulting TaskCancelled vs expected OutcomeUnknown is a real failed lifecycle assertion, not a reason to relax it. Grant preparation/approval RPCs were794/2365 and1402/1486ms in that loaded run, considerably slower than the local controlled observations; latency cause is still unproven.

The second failure's exact source stage is **second operation/revoke fixture marker startup**, a1second wait at line1177. It is not the2second thread.is_finished cleanup wait later in the function. CONTROL task_cancel had already printed, proving the first operation completed its cleanup assertions. An initial reviewer line-number mapping to cleanup was corrected after reading the exact201f source. No cleanup hang is established by this log.

Each independent test fixture performs several Git subprocess inspections/commits, SQLite transactions/syncs, daemon threads, owned worker processes and polling. Default Rust harness parallelism can make these independent fixtures contend on a constrained runner; that is a testable resource-pressure hypothesis, not established historical root cause. Primary prepared a CI-only RUST_TEST_THREADS=1 environment on the two Mac Rust/native test steps; Linux default parallelism remains. All test cases, deliberate within-case concurrency, production code, grant1000ms, marker1second, lifecycle3seconds, Environment checks and workflow job timeout remain unchanged. This controls competing independent fixture load rather than accepting later results or skipping concurrency tests. Exact new Mac CI is still required, and a serial admission that still exceeds1000ms must fail and be diagnosed.

No additional fixture source or Cargo command was executed by this author for this second investigation. The primary is separately running a local serialized saved-binary control and owns the workflow change/review/CI record. Results are not claimed here until actually returned. Data-workspace draft is safely checkpointed at localcacd4ce018648081166b64cb8be11eaa84f4cc98, uncompiled/unverified.
