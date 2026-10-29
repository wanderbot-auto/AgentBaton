# Factory runtime

Base e751992780a778d78edbc6b577359f9bbc3759ec, branch codex/factory-runtime-core. Fetched protected main a1be94d26d10bb245bb35eb50226b82012c967da and read AGENTS/gate/APPROVAL/OWNER001/002: ACK, no new authority. AB-START-001 permits implementation; actual model/account, paid host, production and signing remain separately gated.

Requirements: AB-TSK-002/003/004/005/006/007/008/012, AB-SEC-003/010, AB-COR-002/003/006, AB-DOM-001/003/005. Existing Goal scopes do not grant Factory or new workspace authority. An independent native factory proposal fixes all root/grant/config/input/output/recipient/limit bindings. Complete proposal/response/event serialization including escaped envelopes must fit1MiB; older kinds keep their bounds.

1. Freeze strict Rust DTOs and fixed owner/scoped-tool contracts with the integration owner.
2. Add schema13 durable Factory approval, task lineage, scoped-token digests, handoffs/waits and history. All writes and idempotency receipts commit atomically; child roots share existing cumulative ledgers.
3. Expose only delegate_task/get_task_status/get_task_result/cancel_task to a session/attempt/fence/expiry-bound handle. A fixed Baton companion serves bounded MCP stdio and never receives or forwards the daemon's general token. Runtime MCP configuration accepts only its separately approved fixed binding; external native configuration remains read-only.
4. Execute a real generated parent→child→parent lifecycle. A live owned parent still occupies all actual resources; stop/join before child admission. Parent continuation uses a new attempt and explicit handoff, not fictitious native context restoration. Unknown effects never replay automatically.
5. Exercise normal, negative, cancellation, expiry, backpressure, journal/rollback, migration/restart and shared-budget paths, then full applicable tests/lints with an explicitly granted Cargo lease, independent review and precise evidence.

Initial supported authority uses already approved connections/workspaces. create_workspace and child Goal/planning capabilities fail closed until their exact independent runtime contract is implemented. This is a staged implementation of the complete design, not full Factory/product completion. No new dependency/lock changes, UI/native/workflows/global status edits, recursive agents or remote push are allowed in this worktree.

Owner-read subunit: offload fixed Factory Owner propose/finalize/start hashes to the existing bounded request worker, preserving original raw/deadline and exact business-row/FD proofs. Maintain full original clean/base semantics with bounded whole-worktree and loose-ref metadata observations; explicit overflow refuses, never truncates. Freeze this delta plus two actual Owner IPC regressions independently before continuing the remaining timed/fault/accounting matrix.

Mixed-ledger subunit: execute an ordinary managed conversation, an explicitly approved Goal node, then the actual fixed-MCP Factory parent/child/new-parent lifecycle under one original native-confirmed Grant/project. Reconcile consumed attempt/runtime/selected-artifact measurements against actual durable receipts and true owned stops. Separately hold an ordinary process active and require Factory start to refuse without creating a run/child/reservation, then admit the original request only after its true join. Preserve any counterexample and narrowly repair shared reservation logic rather than increasing limits.


## Owner start CAS correction

Before any new Factory Owner start read plan, projection, descriptor/image lookup or worker allocation, compare the requested revision with the persisted approved scope. Preserve exact-key historical replay before this fresh-request check. Exercise a real Directory/Data Owner IPC wrong-CAS request with zero intent/key/journal effects, then the corrected same key through generated MCP execution and exact history after removing the companion. Keep the existing driver STALE_REVISION assertion, caps, deadlines, public DTOs and schema13 unchanged. This is a narrow correction, not the remaining large-read/fault/full-regression acceptance.


## Large Data owned-read test boundary

Use only a Linux generated fixture shared library, loaded through the explicit environment of this test's own standalone daemon. No Cargo/rustc/Host/other-daemon/Provider injection, public RPC, product flag or production changes. The interposer matches the selected private regular-file device/inode/size, UID,0600/single-link identity while the fixture retains its original FD. It pauses only after a real65536-byte read and uses a private marker/release with10s test-failure bound. Keep original cancellation/deadline/resource semantics and distinguish approved Scope expiry from the original queued ReadyRequest deadline.

Exercise independent Ownercancel, Ownerrevoke without later Ownercancel, and actual approved4000ms Scope expiry without later Ownercancel. While the original read is held, require Owner reads/control responses, an unfinished original request/thread and unreleased attempt/lease. After explicit fixture release, require the original nonce's refusal, actual worker exit/join and original owned leader reap before final active0/lease0, with consumed1 and no child. Preserve bounded original-daemon SIGTERM cleanup and all failed attempts; do not infer complete descendant containment or interrupted kernel I/O.


## Admission current-schema fixture correction

Following the actual full workspace failure, update only admission_performance's exact current Store schema expectation12→13. Keep every admission/Git dirty/path/performance guard, input, budget and timeout unchanged. Classify all other12 matches before editing: original pre-v13 fixtures/backups/rollback/intermediate migration steps retain12; genuine current-version expectations elsewhere require separately scoped integration updates. Do not rerun a known stale suite or modify Root's source tree in this limited task.
