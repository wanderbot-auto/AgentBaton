# Session timeout fixture independent review

Reviewed source `da4dda032bbb73d9ac484a71c5f0f99c1f5684e1` at documentation HEAD `ef8254466c0c29fcf8cd1f965d76610f58571f85`. This commit changes only `crates/core/tests/sessions_ipc.rs`; no production, lock, budget, policy or desktop source changes. Separate reviewer context; no source changes, staging, commit, push or subagents.

## Verdict and retained coverage

No blocking assertion weakening found. Ready for fresh PR7 exact-head CI, with the timing limits below. This local generated-fixture run does not establish fresh macOS/native GUI success.

The original short1200ms grant is an **absolute authorization deadline** established before session admission/Provider readiness. It cannot promise an observer will see a transient ready state. The first phase now waits for actual stopped enforcement, then deliberately begins the late-observer ready-or-stopped read and still explicitly requires stopped, a timeout/expiry diagnostic and durable stop evidence. The ready-or-stopped predicate therefore cannot substitute for the real running-worker coverage below. The1200ms duration and original10s waits were not extended.

The second independent phase retains the normal10000ms runtime budget. It creates a real stopped-daemon Store plus public Supervisor, observes actual ready, records running, sends hang, and reads the generated child's actual PID marker. While waiting for that PID to disappear, it performs **no Supervisor tick, cancellation or revocation**, so the absolute-grant policy tick cannot stop the child in place of the worker's own runtime/turn deadline. Elapsed time must be at least10000ms from before launch. Only after actual process disappearance does it tick to persist the receipt, requiring process_reaped=true, no_process_started=false, stopped state, a duration/turn-timeout reason and removal of Supervisor ownership. An unrelated independently owned sleep child remains alive. Thus accepting either existing timeout diagnostic does not permit launch failure, an unexecuted hang or early cancellation to pass.

The third phase independently reaches actual ready/running and the real hang PID, invokes authenticated grants.revoke, waits for stopped, verifies the durable reaped receipt and requires the unrelated child alive. Original cancellation responsiveness tests remain unchanged. PID probes use signal0; cleanup targets only the fixture's own child/owned process group, not an arbitrary process search. The fixture's unrelated Child guard cleans up its own process on success or panic.

## Evidence and remaining timing limits

The preserved old-ready control log was independently read and its hash matches the evidence: after intentionally observing a durable stopped/grant_revoked_or_expired state, the unchanged old ready-only waiter fails101 after its original10s wait. This demonstrates the missed-ready observation defect. This reviewer did not rerun the temporary old-code control. The two retained real Mac CI failures remain failures, not new passes.

The1200ms grant still must be admitted before expiration: Scenario creates the task after approval and then creates the session. A scheduler stall exceeding the remaining authorization window can still make admission fail legitimately. The fixture does not extend or resurrect expired authority, retry with a larger budget or guarantee robustness under arbitrary scheduler pauses. Likewise the revocation diagnostic intentionally combines revoked-or-expired; this fixture actually invokes revoke, but does not independently distinguish a coincident absolute expiry under extreme scheduling delay. These are limits of this timing evidence, not permission to relax readiness/running or stop-proof assertions.

## Actual independent command

Cwd `/workspace/.worktrees/quiet-canvas`, explicit umask022:

`cargo test --locked -p agentbaton-core --test sessions_ipc`

Exit0; **13/13 tests pass**,17.75s test execution (initial compilation14.05s). Real generated child/Unix IPC and public Supervisor fixtures on Linux only; no vendor/model/account/native Mac executable. All original13 tests remain. The reviewed source hash was unchanged after execution.

| Evidence | SHA-256 |
| --- | --- |
| `crates/core/tests/sessions_ipc.rs` | `87e9719bbcc6cb85847e7a3bc3eb8c98a05b0f9f10c716f69bef0e1a119d58d8` |
| `/tmp/ab-review-quiet-timeout.log` | `fcb85ca42b20970b607a03c3ec7b8c33c9194ee8f7f158e84d5ec86ea40d1a9d` |
| `/workspace/.onboarding/quiet-canvas-pr7-controlled-old-ready-failure.log` | `7cf8275fade71e8cd4bd5da91598011b7961c37a8e77b76ed2cfb5357d011c83` |
