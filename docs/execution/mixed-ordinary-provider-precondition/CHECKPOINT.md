# Root handoff

Worktree `/tmp/mixed-ordinary-provider-precondition`, branch `codex/mixed-ordinary-provider-precondition`, base `accc1aeb4cf33e770a623824a56651b5f30e0eb8`. Changed production paths: none. Changed test source: only `crates/core/tests/factory_runtime_ipc.rs`; only the held busy turn's readiness predicate and position of its existing Connection::open changed. The added statement is SELECT only; connection open flags are unchanged.

Root next: independent patch/source review, then actual `cargo test -p agentbaton-core --test factory_runtime_ipc mixed_live_ordinary_owned_process_blocks_factory_dispatch_without_refunding_shared_budget -- --exact --test-threads=1 --show-output` and the complete `factory_runtime_ipc` target on authorized executors. Real Mac evidence remains pending. Preserve the original ARM failure. Require the unchanged final captured bytes/runtime/settlement/no-refund/counters/owned-stop proofs; a timeout or refusal remains a failure.

No Cargo, SQL, generated process, Core, Node, browser, network, CI or push ran in this subtask. Source-only checks are reproducible through `/tmp/mixed-ordinary-provider-precondition-source-check.py`; output and hashes are in EVIDENCE.json. No formal Approve or merge authorization is given here.

## Root composed verification — 2026-10-31T06:21:02.745312+00:00

See [PLATFORM_CLOSURE_RESULTS](../authorized-integration/PLATFORM_CLOSURE_RESULTS.json) SHA256 d76e816d670f9d213d88830275d24f5d640f1c55e008c5d52b98da94f20b055b for exact sources, actual commands, preserved failures, independent reviews and pending platform dimensions. This supplements historical source-only records; no full requirement/native/Provider acceptance is implied.
