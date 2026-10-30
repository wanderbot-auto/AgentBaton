# Factory summary proof — bounded readonly optimization

Requirement boundary: AB-SEC-007 and Goal/Factory rework readonly integrity. This package does not change execution authority, scheduling, permissions or acceptance. Root explicitly authorized the three source paths and these four records. Base: `059f96b0a1b008f560554731ad77cef2fa6a6d95`.

## Problem and evidence boundary

The existing summary directly repeats `checked_record` through `capability`, and directly repeats `adopted_plan`. Public get additionally builds strict details/origin. Intel head059 recorded a list call taking 1467ms and cumulative 25 list calls taking 29187ms, alongside a distinct plan2/run11 node settlement of 4013/4000ms, overrun and outcome_unknown. These facts do not locate producer phases or establish that readonly validation caused that overrun. No CPU/hash explanation or 4s guarantee is claimed.

## ADR: request-owned SQL snapshot

Only autocommit get/list starts an explicit Deferred SQL transaction. The initial historical scope check establishes its stable read snapshot. A private lifetime-bound `ReadonlySummaryProof` holds that transaction reference and readonly record/run/plan data. It calls the full original `checked_record`, `run`, all four original capability record/run predicates, and `adopted_plan`. Failed runs still invoke the original complete `verify_confirmed_failure`; accepted-state verification in `run` remains unchanged. It constructs only the existing summary DTO, never `GraphAuthority`, execution scope, stop proof, permission or live token.

The eliminated direct repeats are justified only inside that owned stable SQL snapshot. There is no global, cross-request, cross-transaction or post-write cache. A caller-owned transaction uses the original strict summary with no bundle, so its own intervening writes cannot reuse older proof. Get details and origin retain their strict implementations and execute within the same read snapshot as the summary. Success explicitly commits the owned read transaction; errors drop/rollback it. No persistent query_only setting is introduced and no caller-owned transaction is committed or rolled back.

The original capability, strict summary, live scope, policy/expiry/revocation, FD/FS checks, effects, cancellation, admissions, budget and capture paths remain byte-for-byte unchanged. SQL snapshot consistency does not freeze wall time, filesystem state or live authority. No live authorization result is reused by this helper. A historical read after actual revocation retains original behavior; it does not grant fresh execution.

## Historical callee audit

- `checked_record`: original bounded source document/columns/digest, Factory approval/scope/Grant bindings, derived scope, initial saved raw plan, lineage and target request, exact ordered authorization event, idempotency receipt and historical caller proof; no current clock or external I/O.
- `adopted_plan` (included `goal_factory_adoption.rs`): original bounded ordered adoption chain, raw refs, receipt/CAS/request hashes, historical scope/caller and typed failure digest. No live expiry/FD/FS result is retained.
- Four capability bindings: scope, task, project, root Grant exactly match the original initial record. The strict capability itself remains unchanged for all execution callers.
- Failure/acceptance: original full stop/receipt/candidate/validator/Owner checks remain strict. They may repeat historical checks internally; this patch does not memoize those checks.

## Verification plan

Write four actual Core/SQLite tests using existing genuine approved queued/adoption fixtures; do not manufacture approval or stop success. Compare serialized summaries and complete public Views against original strict behavior, verify real r2 adoption and immutable initial ref, reject six deliberate row/history corruptions, and prove caller-transaction writes plus later requests cannot reuse a prior result. Preserve original tests and assertions.

Author runs only rustfmt, diff and bounded source/API inspection. Root owns compilation, selected tests, Core/Factory/sessions IPC/Clippy and exact performance/CI evidence. Independent source review is required. Preserve every previous Intel failure and distinguish old source results from this source. No Mac fix or measured improvement before actual evidence.
