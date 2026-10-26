# Actor writer-lock handoff independent review

Reviewed source merge `17b71d5b6cd046ecaad3777cdefbd3f3cac0174a`, final documentation HEAD `3816a20e980c23f4d4132c02b84d5c2ac4d9022a`, parents reviewed session `a763d895e420fa0b25a5d546ad744c93c57d216c` and protected-main snapshot `eb115e0915a4dd0ac46c8c188101b64b89e9d35f`. Independent reviewer; no source/lock edits, staging, commit, push or subagents.

## Verdict

No blocking finding in this narrow actor teardown composition. Ready for fresh Session PR6 exact-head required CI. The earlier independent no-Supervisor duplicate-FD review was not treated as approval of this actor variant. PR9's prerequisite native checks/installers do not establish Session native acceptance. No current macOS/native GUI or whole-product acceptance is claimed.

## Source and behavior

Relative to session parent a763d89, the only product source changes are `ipc.rs` and `sessions.rs`. Storage, schema4, policy, workspaces, dependencies/locks, desktop and Provider code are unchanged; no schema5/profile/adoption code is imported. Both existing first-frame tests remain, and the incoming main duplicate-FD regression is retained.

`Daemon::drop` now calls `Supervisor::reap_owned_workers` **before** removing its socket and explicitly unlocking its own writer File. The former Supervisor Drop cancellation/join/probe-join body was mechanically compared and is verbatim in the new method. It cancels every worker, drains and joins workers, then drains and joins bounded probes. It does not touch SQLite, release durable leases or fabricate stop receipts. Unpersisted outcomes remain conservatively fenced for restart reconciliation. The later Supervisor field Drop repeats the drained method harmlessly. This preserves writer authority and the old socket throughout owned cleanup, and removes the old socket before a successor can acquire the lock.

The generated cleanup fixture is entirely cfg(test). It waits for actual cancellation, attempts a competing bind from the joining worker, requires WORKSPACE_BUSY and the old socket still present, then confirms that a new bind succeeds after full Drop despite an old duplicate descriptor. Closing that old duplicate cannot release the replacement's independent lock. A swallowed worker panic cannot produce a false pass because the parent also requires the checked flag. The existing duplicate-FD test independently retains live-owner refusal and final restart assertions. No retry/delay or acceptance assertion was weakened.

The primary's preserved intermediate late-reap control log was read and hash-verified: test exit101 while cleanup occurred after unlock. It demonstrates this ordering regression. This reviewer did not rerun the wrong-order variant, and neither that control nor the duplicate-FD mechanism establishes the historical Darwin WORKSPACE_BUSY root cause.

## Independent verification

Cwd `/workspace/.worktrees/session-integration`, actual command:

`cargo test --locked -p agentbaton-core --lib --test project_ipc --test sessions_ipc`

Exit0: **27 top-level tests** (11 lib,3 project IPC,13 session IPC), including both first-frame regressions, duplicate-FD handoff, active-cleanup ownership, real Unix restart, responsive cancellation, crash-unknown lease fencing and generated Provider process lifecycle. One nested FIFO child emits an additional1-test result and is not counted twice. These are local Linux/generated fixtures, not live vendor/model or native GUI tests.

All **68 source hashes and16 primary log hashes** in the current EVIDENCE.json were independently recomputed and matched. The source diff and verbatim Drop-body comparison were independently inspected. Primary's full201 Rust,24 workspaces,35 UI, Clippy/fmt and real daemon/CLI SIGKILL-restart results remain primary evidence; no duplicate independent execution is asserted. Fresh exact-candidate CI/native evidence remains required.

## Reviewed source SHA-256

| File | SHA-256 |
| --- | --- |
| `crates/core/src/ipc.rs` | `5e836853bdeafe960c896ed1d60491f520f951b85cbf6952c4de8e39a4c1b11e` |
| `crates/core/src/sessions.rs` | `d4b00faaa200be458d9528c611fe037827b81501cb6e43ba66c171bf9568c0f8` |
| `Cargo.lock` | `3f518d6cfeb5cdf126a24708120b4ec8349c5aab1d80ff4512b8296efcd5a33c` |
| `crates/core/src/storage.rs` | `905de3f02f01465c07690d3f88c5940e52e68531ad82acb13090bf91096a2c5a` |
| `crates/core/src/policy.rs` | `77a775f2befa7c70afa9ba5e8928ecb28d490da9de2c9bdef67b79469b4ca449` |
| `crates/workspaces/src/lib.rs` | `796e8cf7b2f92052f01b4140ac7f76fa47631959190138fbaa755ff7b682491e` |

Independent log: `artifacts/session-pr9-writer-integration/reviewer-actor-handoff.log`, SHA-256 `3eb0af2949ae1aaf35f38c5039164daa96743aad39538955bf7c568258652c9e`.
