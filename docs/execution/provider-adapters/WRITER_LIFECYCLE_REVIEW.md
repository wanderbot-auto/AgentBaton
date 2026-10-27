# Independent actor writer-lock lifetime review

Reviewer: native `/root/integration_review`, independent implementation context. Reviewed frozen worktree source over root `88adbb9` during intentional merge of writer-lock guard `34dc5`, at 2026-10-26T21:42:44.004236+00:00. Only this review document was written; no sources, locks, staging, commits or remote operations were changed. The worktree's resolved IPC content was reviewed and compiled; marking its intentional index resolution remains the integrator's operation.

Verdict: no unresolved source blocker in the actor lifecycle port. Ownership release now follows provider/probe cleanup on normal, error and unwind destruction paths. Existing advanced route/configuration/schema5/policy behavior remains intact. Fresh full regression and exact-candidate native CI are still required; this review does not establish the historical ARM restart cause.

## Production ordering and preservation

`Daemon::drop` first calls `Supervisor::reap_owned_workers`, then unlinks the old socket, then explicitly unlocks its existing writer-lock File. Retaining the lock while workers cancel/join prevents a successor business authority from starting before owned cleanup finishes. Unlink-before-unlock prevents old destruction from deleting a successor's socket. Explicit unlock releases the old open-file-description lock even when an old duplicate descriptor remains open; closing that old description later must not release a new owner's independent lock.

The extracted reap method is byte-for-byte the prior Supervisor Drop body: set cancellation for all owned workers, drain/join workers, then drain/join probes. Supervisor Drop delegates to that method, so calling it earlier is idempotent. It performs no DB writes, grants no permissions, invents no stop receipt and replays no ambiguous attempt. After a fatal actor error, persisted unknown outcomes remain for restart reconciliation. Normal shutdown still performs its existing database-aware cleanup before destruction.

Compared with 88adbb9, route matching/runtime-plan verification, schema5 storage/migration, policy/execution/provider code, UI, both Cargo locks and config materializer are unchanged. The change is limited to Daemon destruction, extracting/delegating Supervisor cleanup and cfg(test) fixtures/regressions. Both existing delayed-first-frame tests and oversized-parser assertion remain.

The inherited-FD regression uses a real File::try_clone duplicate, proves active owner refusal, drops the old daemon, immediately binds a replacement while the old duplicate remains held, and verifies closing the old duplicate does not unlock the replacement. The actor-specific cfg(test) worker waits for cancellation, then probes actual Daemon::bind and socket presence during cleanup: BUSY and the old socket must still hold before the join completes. The checked flag makes a swallowed worker assertion panic fail the parent test. This fixture is a private test method, not a shipping endpoint, and its synthetic receipt is never persisted.

## Independently executed validation

Under `umask 022`, configured CARGO_HOME/RUSTUP_HOME/PATH and CARGO_BUILD_JOBS=2, ran:

`cargo test -p agentbaton-core --locked --lib --test project_ipc --test sessions_ipc`

Actual result: **33 top-level tests PASS** (lib 13 + project IPC 3 + real generated-process sessions IPC 17). The additional nested FIFO child is excluded. This includes both writer-lock tests, both first-frame tests, actual project restart/lease behavior, cancellation responsiveness, explicit native/ACP routes, immutable config binding and unknown-outcome fences. No real vendor/account/model was used.

Read the integrator's writer-lifecycle workspace Clippy log: success. The earlier 13-unit log and 3+17 IPC logs also pass and are fingerprinted below. The new whole-workspace 254 result was still pending at this review and is not inferred from these targeted checks. The four previously reviewed native-harness source fingerprints and the 49-test UI fixture fingerprint still match; their prior Linux results do not establish native macOS acceptance.

The duplicated-FD mechanism and the actor cleanup barrier are directly verified locally. They do not prove that inherited descriptors caused a historical ARM WORKSPACE_BUSY or any other native startup failure. No native GUI/remote CI success, production action, private credentials or extra paid service is claimed. Full Design v0.2 remains incomplete.

## Frozen reviewed source fingerprints

| File | SHA-256 |
| --- | --- |
| `crates/core/src/ipc.rs` | `f03f26084df88ba6f4fec1a4a6f23aa7a0334740c7cbe8e17b9b121eb17f98ae` |
| `crates/core/src/sessions.rs` | `a2cf3b789316221743b2be6c1ab7391c4590fc384eb4390c2eb49c239f611752` |

## Observed integration logs

| Log | SHA-256 |
| --- | --- |
| `/workspace/.onboarding/provider-runtime-writer-lifecycle-lib.log` | `b1d89a8f92a64211052f57dcf6f93b447ed018a0980d7b0079ee0b0079403e76` |
| `/workspace/.onboarding/provider-runtime-writer-lifecycle-ipc.log` | `58b623abb6702816392c2425d3f3d3be4167aee04bb385a6d6e3b6ded8db53d9` |
| `/workspace/.onboarding/provider-runtime-writer-lifecycle-clippy.log` | `29b50adeec5e8928073cd3f6bd5cadb350fafb583243fedeb6e36460c08d462d` |
