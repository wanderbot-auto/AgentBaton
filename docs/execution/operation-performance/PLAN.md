# Operation performance

Owner request: assess frontend/backend operation latency, rank TOP5, implement and verify each. ACK OWNER-001/002; no authority changes.

Base: origin/main `b78ffbb5bcbe741de2e0cd8fcf2e568c4c7ef28c`; task branch `collab/operation-performance`. Worktree is isolated from the owner's running desktop. Runtime/build/test output uses `scripts/collab.sh run`. No Provider calls, production publishing, or signing.

1. Main refresh: start independent catalog reads alongside snapshot on connected refreshes; preserve cold daemon bootstrap, sequence-dependent events, stale-response guards and failures. Test using deferred promises, including rejected snapshot/catalog reads and overlapping refreshes.
2. Task grid: memoized child and linear lookup indexes; preserve ordering and first-match semantics. Verify input does not render unchanged cards, and updates/navigation still work.
3. Journal page: single live cursor with SQL CASE for individual size and cumulative byte guard before Rust String conversion; preserve 4MiB output cap, continuity, oversized errors and UTF-8 checks. Compare same fixture before/after on Apple Silicon; run storage regressions.
4. Notifications: memoize retained evidence/aggregation with primitive observation dependencies. Test no repeated aggregation on unrelated renders and invalidation on data/freshness changes; retain existing fail-closed tests.
5. Detail reads: parallelize independent plan/run and task/project reads, keep scope/revision validation and stale-response guards. Test pending reads and existing regressions.

Acceptance: focused tests per item, full frontend suite/typecheck/build, relevant Rust tests/format, independent source/diff review, scope and merge-conflict preview. Real native/Provider acceptance is separate from mocked frontend tests. Record actual commands/results, baseline measurements and unverified dimensions. Feature branch delivery; no main write or owner's app restart.

Integration scope extension: root owns global STATUS for this task. After verifying no other active reservation owns BOOTSTRAP_MANIFEST.json, root reserved this literal path under the collaboration registration lock to refresh only the reviewed STATUS digest. The verifier and other hashes remain unchanged. Independent metadata review required before acceptance.
