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


## Complete schema13 test expectation alignment

The integration owner expanded the narrow followup to13 explicit test files: admission_performance, command_validation, data_workspaces, sessions_ipc, migration_v5/v6/v7, command_validation_migration, data_workspace_migration, goal_store_migration, goal_runtime_migration, goal_graph_migration and storage tests. Change only12 completed/fresh current Store expectations12→strict13; change7 future-refusal SQL inputs13→14 and their6 versioned test names plus1 same-site diagnostic. Preserve original old-schema migration inputs, intermediate migration PRAGMA12, pre-v13 backups/rollback12, unknown99, raw-byte and every other assertion. Freeze exact source/normalization for independent review before another full suite. No Cargo lease is held by this author.


## Historical pre-Factory fixture construction

Fix only the ten explicitly scoped historical fixture targets and one tests-only helper. Fresh schema13 fixture construction checks version13 and ALL nine named Factory tables empty in one transaction before any DROP; use only a fixed child-before-parent nine-table DROP and set12 as the genuine pre-Factory baseline. Missing/nonempty/wrong-version inputs fail without mutation. Keep every original historical SQL step, old input version, raw bytes, backup/rollback, CAS, permission, budget and timing assertion. Never change production migration to tolerate a mislabeled database.

The existing sessions migration case uses a real Goal first, so verify actual owned stop/join, settled original reservation, consumed project/root meters and zero active/lease state before the strict empty-table inverse. Snapshot every other table's rowid and typed original values, preserving all legacy rows and raw Goal/stop/meter receipts across the projection. Do not implement production debt clearing or a generic nonempty-table deletion helper. The exact Goal path skips ordinary shared reservations; if a later fixture has a nonempty Factory reservation, STOP rather than delete it under this empty-only contract.


## Large Data initial-read phase diagnosis and fixture sequencing

Instrument only the existing test/helpers with Source, proposal/finalize/start, owned marker/credential, initial GET and actual gate timing plus bounded read-only trusted business metadata. Persist no raw token, credential document or outgoing scoped request. Distinguish Scope's actual absolute expiry, client5s, actor ReadyRequest60s and DataIO30s source budgets; internal actor/worker Instants are not exposed over wire. Capture failure before any fixture change, then one existing30s-scope comparison. Do not guess TTL from CANCEL_REQUESTED alone, repeat unchanged failures or tune dataset/budgets/timeouts.

After measured diagnosis, preserve the Provider's existing real successful MCP GET as the initial validity proof: complete delivered scoped-response text, original UUID and full MCP response frame, actual times and current host-private scope/task/spec/session/attempt/fence/lease/hash binding. Keep the original no-error validity assertion. This marker is test evidence, never a product authority, caller override or reusable private Data witness. The next controlled nonce still goes through actual fresh scoped authentication and a new owned Data read; it must enter a real64KiB read before Scope expiry, with Scope expiry earlier than its original client deadline. Retain all16MiB/4000ms/5s/60s/30s contracts and stop/join/consumption assertions.

## Goal/ordinary input and budget control separation

After the fourth exact integration regression, keep the dirty-input refusal as a distinct safety case and test budget exhaustion on a different, genuinely clean fixture. Only that fixture precommits exact generated report/start-marker ignore rules and approves its original Grant with the same1/2 attempt cap as its Goal. No report deletion, post-execution input rewrite, production policy change or expanded timing is permitted. The default55-case construction remains unchanged. Require exact errors, no denied-request writes, and a real ordinary turn/owned reap plus shared root/project accounting for the remaining-budget positive.
