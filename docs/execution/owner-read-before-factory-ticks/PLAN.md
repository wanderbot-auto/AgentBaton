# Owner read before Factory ticks

Baseline: f55a805a952f84dc65e83c5e33f98e975871c3f1 (PR #18).
Requirements: preserve Owner/Scoped authority, local IPC framing, bounded scheduling, cancellation/stop priority and original observation budgets for the approved Design v0.2 Factory/Goal runtime.

1. Seal the exact Intel failure evidence and distinguish unknown transport timing from post-shutdown product facts.
2. Move only an already queued, FIFO-head, closed typed Owner `factory_runs.get` read ahead of the next Factory/Goal ticks, after the original supervisor/worker/history/fatal pass.
3. Reuse the original request handler and post-request services; keep sixteen total ready requests per turn and all existing wire/deadline/authority checks.
4. Author focused FIFO, admission-count, envelope, actual handler, expiry and retained-error test sources. Root executes and obtains independent review before integration.

No new request, channel, worker, cache, quota, timeout, retry, Scope lifetime or public API. No latency or root-cause claim from source review alone.
