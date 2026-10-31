# Owner read before Factory ticks

Baseline: f55a805a952f84dc65e83c5e33f98e975871c3f1 (PR #18).
Requirements: preserve Owner/Scoped authority, local IPC framing, bounded scheduling, cancellation/stop priority and original observation budgets for the approved Design v0.2 Factory/Goal runtime.

1. Seal the exact Intel failure evidence and distinguish unknown transport timing from post-shutdown product facts.
2. Move only an already queued, FIFO-head, closed typed Owner `factory_runs.get` read ahead of the next Factory/Goal ticks, after the original supervisor/worker/history/fatal pass.
3. Reuse the original request handler and post-request services; keep sixteen total ready requests per turn and all existing wire/deadline/authority checks.
4. Author focused FIFO, admission-count, envelope, actual handler, expiry and retained-error test sources. Root executes and obtains independent review before integration.

No new request, channel, worker, cache, quota, timeout, retry, Scope lifetime or public API. No latency or root-cause claim from source review alone.


## Root actual checkpoint 2026-10-31T00:21:23.878308+00:00

Actual source cbb16c7: Core180, Factory33, strictClippy/fmt and same-source normal browser4checks/4PNG pass. Test-only b183c86 fullinfra284 pass after retained NameError. Exact commands, immutable log/result/input hashes and f55 failed CI are in ROOT_RESULTS.json (SHA256 3a2e5cd21f2a3b00967b69e0543ab8348484f1c2af69ace2626a2760a7ebf627). New exact CI, dualMac/Native/audit/installer and full130 acceptance remain open; no deadline, retry, budget or authority expansion.
