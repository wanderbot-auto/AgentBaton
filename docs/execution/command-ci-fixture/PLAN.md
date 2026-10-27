# PR12 fresh-grant fixture repair

Base7ba60e0822129d474be1ade845640d1d72e79b21; isolated codex/command-ci-fresh-grant worktree. Original CI Product run40446762573 Intel job121999784372 checked out merge56ff4227f2bae71fdbf377fbd07c3af7bcd6e403. The real lifecycle test failed PERMISSION_DENIED expired grant, before Intel native/package validation. No claim of those stages succeeding. OWNER001/002 ACK at protectedmain66061439c79fa44b66a33782febc1fb0f31c73df.

Scope only command_validation test fixture and this unit's documents. Never change production authority/expiry/gates/locks, original1000ms grant/runtime budget or3sec waits/assertions.

1. Preserve original CI raw log. Insert actual1100ms tasks.create fixture-preparation delay into the old grant→task→admission sequence; record method/prep/RPC/total and grant-return→admission timing. Save the actual old compiled test binary before rebuilding.
2. Move task creation/preparation before final grant approval. Keep the same1100ms preparation delay before grant, and add a real1000ms grant explicitly allowed to expire for1100ms. Assert its admission has no session/attempt/lease-dispatch/ACP effects.
3. Approve a fresh1000ms grant last and immediately request admission. Preserve actual initialize→session/new failure→owned reap→Stopped intent/released lease→TaskOutcomeUnknown→separately approved command_passed without TaskAccepted. Record exact remaining TTL/admission timing and typed journal stop evidence.
4. Run this real control, all14 command tests, Core all-targets Clippy and format check under the existing shared target, incremental0/jobs2. Independent reviewer replays old/new controls. New exact-candidate Intel/ARM CI remains required; never call the controlled Linux mechanism proof of the historical Intel cause.
