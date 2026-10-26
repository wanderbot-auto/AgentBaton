# Project registry progress

Implemented persistent read-only project import/identity, expected revisions and idempotency, reassociation/archive without source deletion, exact-HEAD detached workspaces under the host-owned private runtime, durable creation intents and unknown-outcome recovery. Task drafts bind registered active projects through real IPC and CLI. Desktop import/list/archive/reassociate/create-workspace/task selection uses narrow commands.

Added foundational capability source snapshots/provenance/precedence and freshness/session binding, plus pinned executable SHA-256/version probes and distinct Provider/config/identity/host connection contracts. These remain trusted-caller libraries, not accepted end-to-end session dispatch. No real Provider model/account used.

Final local integration:121 Rust tests pass (core34/config26/providers48/workspaces13), full workspace Clippy with warnings denied and fmt pass;15 frontend transport-mock tests, TypeScript and frontend build pass; real daemon/CLI SIGKILL restart smoke pass; design and28 infrastructure Python checks pass. Independent review with exact41-file source hashes in REVIEW.md.

Review fixes: new/renewed leases recheck actual canonical directory/Git identity; capability evidence rejects empty/malformed session association; source binding IDs must be Baton-global. Lease TTL/fencing cannot stop a stale Provider writer, and future dispatch must reconcile/stop before reuse. No OS sandbox or full scheduler acceptance claimed.

Prerequisite PR#3 remains unmerged pending actual macOS checks. Three recorded fixture failure variants led to independently reviewed transport-correct parser/integration assertions; fourth candidate30a70d8 pending CI. No native application/DMG artifact exists yet. Full remainder preserved; next is persisted connection/policy/session supervision and live daemon wiring with fixture-only external processes.


## Foundation integration checkpoint

Local merge57f4665 retains registry plus approved foundation222aafd/OWNER002, preserving92 manifest paths and both historical STATUS reports. Production backportaa258a6 fixes version-probe idle scheduling with no timeout/volume relaxation. Local123 Rust/15 mocked UI tests, fmt/Clippy/typecheck/build, actual daemon/CLI restart, design/infrastructure28 tests and integrity pass. See INTEGRATION.md/EVIDENCE.json for exact source/log binding and the deliberately failing old-production control. No push/main merge or registry native CI acceptance.
