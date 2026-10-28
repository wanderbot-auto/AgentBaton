# Checkpoint

Author task: `/root/provider_startup_diag`; local managed execution only.
ACK AB-START-001, OWNER-001/002; no authorization expansion or recursive agents.
Runtime status reported connected, current observations, no configured secrets
or identities, network policy state unknown. No network work was needed.

Branch `codex/provider-startup-evidence` in
`/workspace/.worktrees/provider-startup-evidence`, base
`0a0beac38260cc426b4a0887b2bf59937ec9ff90`; source commit
`f61842cf7d03b3967ceff60a0e6883d0d37dd682` modifies only
`crates/providers/tests/native_codex.rs`. Documentation is a subsequent commit.
Other worktrees, existing review files, shared Core/Native targets and STATUS
were preserved. Main was untouched. No push/merge or CI trigger/rerun/cancel.

Implemented failure-only helper evidence: mode, typed failure enums, monotonic
preparation/start durations, existing fixture PID-record presence and bounded
4096-byte wire-prefix summary. No raw wire/stderr/paths/argv/env/account values
are emitted. A recorded response does not prove stdout flush or client receipt.
Start elapsed includes verification/spawn, initialize and failure cleanup; it
must not be compared directly to the initialize-only 500ms deadline. Preparation
also includes Python discovery and the fixture's initial digest computation.

Original fixture bytes and all 19 original cases unchanged; one added regression
checks a real local delayed-child timeout and hostile/truncated wire redaction.
Original 500ms, 4MiB, fault/reap/no-replay/secret assertions are intact.

Final Linux x86_64 / Rust 1.99.1 / Python 3.12.14 test result: 20/20, zero ignored,
exit0; log SHA
`731a7e06c10b7377f46f88e661fb070178386d7c1e2286f16fa8c6b9646f490b`.
Dedicated target remains 151MiB; no shared target used. Local Python handshake
profile: 5/5 responses valid, 5/5 children reaped, 17.616–22.246ms initialize.

ARM original log/18-of-19 failure remains preserved. ARM root cause unknown,
not reproduced locally. Intel run was not inspected/cancelled/rerun by this task.
Goal aliascfgtest, actual Mac GUI and packages remain outside this local
validation; no Linux result establishes Mac or real-Provider compatibility.

Review status: author self-review only; Root must arrange independent review.
Next: review/cherry-pick exact source/docs commits, then use actual authorized CI
failure evidence if startup fails again. The new log can identify mode and last
recorded stage. Do not infer a performance cause or change production behavior
without stronger evidence and Root's separate scope approval.
