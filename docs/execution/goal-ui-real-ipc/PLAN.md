# Goal UI real IPC plan

ACK OWNER-001 / OWNER-002. This unit adds a test-only browser harness to reviewed Goal UI source `98a38d4cd486afa2a4fe476d0f464f1763d0805a`, worktree base `4953abe4e9607551d0694016be35b1d52158fac8`. It does not modify product files, existing tests, CI, locks, Core or permissions. Full Design v0.2 work continues separately.

1. Read the exact schema9 public DTO, raw `plan_json` IPC and existing real command-browser fixture. Keep the original plan string, CAS and idempotency key intact.
2. Generate a private local Git/input/ACP fixture. Use real connection proposal/finalization and successful `--version` probing; never insert synthetic ready connections with SQL, create sessions or call models. Finalize the separate actual `goal` proposal outside HTTP.
3. Serve built React through a loopback-only, bounded, fixed-command test adapter. Permit metadata, Goal history, fixed-scope plan propose/revise/revoke and readonly preview. Deny approval/finalization, permission changes, generic IPC, session creation, dispatch, reservation and acceptance. No production entry or capability is added.
4. Exercise lazy expansion and empty history, raw original plan persistence, revision CAS/history, real readiness preview without journal/budget/execution mutations, exact-key recovery and invalid duplicate JSON, input drift and revocation/history access. Observe SQLite strictly read-only to compare actual durable records; never fabricate records.
5. Preserve failures, screenshots, exact binary/harness/frontend hashes and honest dimensional limits. Run the same script against the old frozen daemon and a subsequently supplied repaired schema9 daemon. Do not rebuild/cherry-pick the backend in this worktree.

Completion needs successful real execution of the remaining checks and independent review. Chromium/Unix IPC cannot establish native OwnerDialog, macOS, Goal dispatch, reservation, task acceptance or natural-language planning.
