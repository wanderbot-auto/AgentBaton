# Checkpoint

Source baseline: f55a805a952f84dc65e83c5e33f98e975871c3f1. Branch: `codex/owner-read-before-factory-ticks`. No PR or merge performed by this author. No parent worktree changed.

Implemented: private strict classifier, one FIFO-head custody value, shared sixteen-request count, original handler/post-request service, conservative early-path retained-fatal guard. Added Rust test sources for closed/duplicate/malformed envelopes; wrong owner, key/revision and scoped-handle exclusion; held-write FIFO/reply ownership; early and normal sixteen-request accounting; empty-head behavior; actual original-handler response and no write; actual expiry; before/after retained-fatal propagation and original socket cleanup.

Verified: formatting and static byte/order proofs recorded in EVIDENCE. Not run: test compilation, Rust tests, live Actor/IPC, Core/Factory regressions, UI/native/Mac/CI. Test-control Provider behavior and binaries were not created.

Next: Root applies fixed source commit to its integration candidate, runs the focused module plus existing Core/IPC/Factory suites, and commissions independent review. Only actual same-candidate CI can establish both Mac lanes and full product evidence. Preserve original two retries, 5 s client read, 55 s observation, Scope TTL and budgets; failures remain failures.

## ADR: FIFO-head custody before Factory/Goal ticks

Decision: after the original supervisor/finished-worker/history/fatal pass, dequeue at most the existing FIFO head. A strict Owner API 1 `factory_runs.get` with one closed typed `scope_id`, no key/revision, no scoped handle and no private dispatch ticket may use the original handler before ticks. Parse typed params from original bytes so duplicate `scope_id` keys cannot qualify through `Value` normalization. Other heads remain in one stack Option and are drained first at their original phase. Do not scan, select later reads or reorder writes.

Consequence: dequeue frees one slot in the existing bounded channel earlier, including when a non-read is held; this is a small explicit enqueue-timing change. There is no new slot quota or persistent authority/data cache. It cannot help a socket not yet accepted, interrupt an active tick, guarantee a client latency, or establish the f55 failure cause. The original listener stays at the end of the turn.

The early request uses the exact original post-request services. Those services can retain a fatal admission error, so the new early path immediately takes and returns that original error before more Factory/Goal ticks. Normal drain and Drop/join/socket semantics are retained. This is error propagation, not a fallback or new business permission.
