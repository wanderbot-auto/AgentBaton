# Data workspace runtime

Base b71492fd49fb9e72a775572d4933cbc2789e6939, isolated codex/data-workspace-runtime. OWNER001/002 ACK; gate AB-START-001 authorizes development, not real Provider accounts/model calls/production/paid resources. Merged independently reviewed librarya10c573 without reverting the current five-Git-call safety batch.

Implement schema10 durable bounded preview and copy intents, native owner kind data_workspace, actor-owned SQLite/journal/idempotency, host-selected roots/IDs, asynchronous single bounded I/O worker, private restore-derived current data input witnesses, actual registry/grant/session/data-contract wiring and failure/restart tests. New DTOs are selection declarations only; roots/IDs/pass/creation authority cannot be supplied by renderer. Preserve old Git guards, project/root budgets and all previous migration/assertion semantics. No UI/native/lock/workflow/global status changes. Generated local fixture children only; no credentials/model/network use.

## Requirement evidence scope

- AB-PRJ-002/005: registered non-Git sources, native-approved explicit input copies, isolated Directory registry, immutable original inputs.
- AB-VAL-002/004/006: current selected-input identities and hashes, fixed contract witnesses, deterministic JSON/integrity output observations, safe relative paths.
- AB-SEC-001/002/004/005: declared selection is not copy/execution authority; host-only native finalization, typed restricted IPC, no external upload, private runtime exclusion.
- AB-SEC-006/007: exact journal/idempotent receipts, unknown interrupted copy and no automatic replay, rollback, private exact-byte schema9 backup and schema10 migration.
- AB-REL-001: Linux actual Unix IPC and generated protocol processes are recorded separately from macOS, real Provider, actual owner dialog and distribution evidence.

These are partial implementation contributions to the complete requirements, not a claim that each requirement or the full Design v0.2 acceptance is finished. Dynamic Goal dispatch/factory capabilities and full native desktop/remote-host acceptance remain outside this slice.
