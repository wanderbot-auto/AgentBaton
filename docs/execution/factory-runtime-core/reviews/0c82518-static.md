# Early static Factory authority review

Reviewed commit: `0c82518a818202be550e1518bfc4e4f075442787` (WIP), baseline `e751992780a778d78edbc6b577359f9bbc3759ec`.

This is an independent static review of Git objects exported into `source/`; the active author worktree is not the reviewed source. No Cargo, daemon, Provider/model, CI, executable companion, source modification, commit, or formal approval was performed. Python was used only to export/hash files and compare provenance. The current AGENTS/gate/APPROVAL/GOAL/OWNER-001/002 were read; this review grants no additional authority. See `manifest.json` for all 22 exported source/document hashes and authorization-read hashes.

## Findings (blocking before runtime wiring)

### F1 — P1: persisted scope expiry is not checked against the immutable owner-approved result

Path: `crates/core/src/factory_store.rs:503`, especially 519–530; consumer `crates/core/src/factory_runtime.rs:154`.

Trigger: after an owner approval, change only `factory_scopes.document.expires_at_ms` to a later time while retaining the original proposal/result/receipt, selection, bindings, and digest. This can arise from a malformed persistence update or local database corruption; this is not an OS-sandbox claim against the database owner.

Static evidence: `scope()` reads the immutable `approval_proposals.result` into `FactoryScope` but compares only `result.id` and `result.contract_digest`. The computed digest covers only schema/selection/bindings. There is no comparison of `s.expires_at_ms` with the approved result. Its column-mirror checks do not contain expiry either. Therefore all current scope predicates remain unchanged by the described single-field drift, and `live()` then uses the changed expiry. Top-level root/project/grant metadata likewise is not checked for derived consistency with the approved selection/bindings/result (changing its mirror columns can avoid the mirror checks).

Necessary fix: compare all immutable authority envelope fields with the original approved result; check root/project/grant references and revisions against their bound selection/snapshot, and preserve approved expiry. Permit only explicitly legal revision/revocation evolution, with its authoritative receipt as appropriate. Add a generated database regression changing expiry alone after approval and asserting fail-closed even while the replacement time is live; separately exercise root/project/grant envelope drift. No dynamic exploit was run in this review.

### F2 — P1: run admission does not recheck the complete approved bindings

Paths: `crates/core/src/factory_runtime.rs:154` and `:248`; related finalization commit boundary `crates/core/src/factory_store.rs:664` and `:693`.

Trigger: approve a Factory scope, then change selected input bytes, connection runtime/configuration/identity/host metadata, workspace registration/root identity, or the companion executable before the first `start_factory_run` request. The task and original Grant document can remain unchanged.

Static evidence: start calls `scope()` and `live()`. `live()` checks scope revocation/revision/expiry/digest, exact Grant document, project archive flag, and root goal/spec/project/state subset. It does not resolve/recompare selected connections/workspaces, hash selected inputs, pin the companion, or invoke the original Grant's registered-binding validation. Hence those drifts do not affect the current start predicates and a queued run is admitted. The finalizer does resolve all bindings before opening its write transaction, but its inside-transaction CAS compares only task and Grant documents; connection/workspace/project updates between resolution and transaction are not fenced.

Necessary fix: use one shared freshness/admission predicate for the full selected contract. Perform database snapshot/CAS checks under the actual write transaction at finalization and start, and recheck external input/workspace/executable identity immediately at actual dispatch. Preserve the original requested/effective/enforced distinctions; no prompt or worktree is a sandbox. Add post-approval drift cases for each selected binding, including non-first selected connections, plus a controlled between-read-and-commit database update case. In this frozen commit the consequence demonstrated by control flow is stale *queued intent*, not an executed Provider action: the runtime backend is absent.

### F3 — P1: Draft-only/no-unresolved-attempt admission is not transactional

Path: `crates/core/src/factory_runtime.rs:267` and `:340` (uses `live()` at :154).

Trigger: another permitted local Store/operation admits an ordinary attempt or changes the root from Draft after `get()`/the Draft check at 267–273 but before the Factory immediate transaction begins at 340–344.

Static evidence: the transaction calls only `live()`, which does not compare task revision or require Draft and only rejects CancelRequested/Cancelled/Accepted. Running/Ready/Failed and new unresolved `dispatch_intents` are not rejected there. The transaction can therefore insert Factory run/lineage/binding for a root whose exact Draft state has already changed. The root-scoped unresolved-intent predicate exists at scope resolution in `factory_store.rs:355`, not at this start commit boundary.

Necessary fix: inside the start transaction compare the exact expected root task/revision, require Draft, and reject any live/unreconciled original dispatch intent for that root before the first durable run write; preserve the single-scope/single-run idempotent replay path as historical data only. Add a controlled two-connection/barrier regression proving no run/lineage/binding/idempotency rows survive the competing admission and no original resources/index/state are overwritten.

## Verified by static inspection; not a runtime acceptance

- The Factory kind and separately consumed owner approval are distinct from old Goal/Grant scopes. Selection-only DTOs do not provide caller identity, roots or a consent flag. Existing zero-depth Grant delegation remains unchanged; its RESOURCE_LIMIT test is present.
- Unsupported child planning/child Goal/new workspace capabilities fail closed in `limits()`. Selection remains explicit and bounded. Propose/finalize bind all selected connection/workspace pairs through existing Grant registration validation and preserve identity/config/host references in the snapshot, subject to F2's timing gap.
- Scope/proposal/events/idempotent results use checks on complete escaped envelopes and a 1 MiB bound; list reads aggregate raw document bytes before decoding. Core Factory helpers use count/byte bounds rather than truncating authority/history.
- Approval transaction writes a receipt with `spawned:false`; it creates no run, child session, token, process or Provider call. Run start creates only queued durable intent. Its run/lineage/binding/event/idempotency writes share one immediate transaction; journal error unwinds the transaction by ordinary transaction drop. There is a single-run-per-scope database constraint.
- Recovery revokes old scoped handles and marks interrupted queued/starting/running/waiting/resuming/cancel-requested runs outcome_unknown; it does not auto-replay a start. Same-key start replay returns original historical result, even after recovery; callers must read current run status rather than treat the historical queued result as execution.
- Schema13 adds nine new tables atomically. A private pre-v13 backup is created before migration; future schema14 is refused before a backup/migration. The new-table migration does not itself upgrade old Goal/Grant permissions.
- Companion reads a bounded private 0600 credential under a 0700 owner directory with nofollow traversal, exposes four fixed tools, and never reads a global owner token. The scoped backend/credential issuance/dispatch authentication is not implemented in this snapshot, so no live scoped authorization or token non-leakage claim follows from the bridge code.

## Budget and unfinished implementation boundary

The nullable `factory_budget_reservations.scope_id` is explicitly legacy shared accounting, not Factory authority. `authority_kind` and origin consistency are checked; a legacy reservation has no run/scope. Its selected-artifact output reservation is zero only for `empty_selected_artifacts_no_validator_streams`. `filesystem_total_bytes` and complete Provider capture remain unknown; they are not zero. Token/cost hard caps are preserved as unknown/unsupported rather than silently lifted.

The frozen snapshot defines `reserve_ordinary_shared_budget`, `shared_session_duration`, and `settle_shared_execution_meter`, but contains no ordinary session/settlement call sites for them. Factory dispatch/session/scoped-caller backend and ancestor reservation implementation are absent. The new adapters therefore do **not** yet establish shared ordinary/Goal/Factory project/root/ancestor/global accounting. These are disclosed unfinished WIP, not independently discovered working accounting or a completed runtime. Required follow-up includes mixed ordinary/Goal/Factory admission+settlement+restart tests, cumulative existing caps without reset, measured artifact-only output versus unknown FS/Provider output, and no blanket permanent ban based merely on unrelated project history. No final approval or full product acceptance is given.

## Evidence boundary

Author-recorded command `cargo test --locked -p agentbaton-core --test factory_scope --test factory_migration` passed 10 unique generated filesystem/SQLite tests, no ignored tests, for the exact **11-file** snapshot `55ee71c6b1fc1deb8abec50d0b00cfdc53a2fad447d5a073dfc18fd9c68f29b8`.

This reviewer independently read and SHA-256 checked the snapshot manifest and its retained log: log `08e4b4b02b0278b18b57b4c45b73dbe3704c877fd098753103258e3ac423352c`; both match the recorded values. See `provenance.json`. Eight of those eleven source files match frozen0c8. `factory.rs`, `factory_store.rs`, and `factory_runtime.rs` changed; the new `factory_budget.rs` and changed `goal_runtime.rs` are outside that tested set. The nullable-legacy/shared-meter delta at0c8 has not compiled or executed according to its checkpoint. This reviewer ran **zero Rust tests/builds**, and does not transfer the earlier 10 tests to the complete0c8 tree.

Actual MCP tools, private Factory authentication, parent→child→parent, cancellation/expiry/unknown reconciliation at dispatch, end-to-end budgets, native UI/IPC, real Provider calls and macOS are unverified/unimplemented here. Findings F1–F3 were sent to primary and Core author for fixes and appropriately leased executable regression tests.
