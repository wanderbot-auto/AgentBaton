# Genuine schema15 →16 Task migration gate

Requirements: AB-SEC-007 migration protection; AB-PRJ-003 identity and AB-PRJ-005 default code isolation compatibility. Designv0.2 / AB-START-001, OWNER001/002 ACK, no003 found in fixed baseline. Base `0d6a034785ffd3d47ca781b2ec41a5036567bb40`; Root's later integrated61c730b retains the same schema16 contract. This child edits only four new source files and this unit's four records. Existing genuine14 producer/consumer, Core/schema, Cargo/locks, workflow, globalSTATUS and all author work are untouched.

Schema16 has no new table/row write: validate complete predecessor15, FK-check and each old Task (forbid code_workspace_binding even null; require id/revision/spec), then atomically set user_version16. None remains absent in serialization. Do not relabel a controlled current-shape database as genuine15, invent an edit RPC, impose last-row iteration order, or fabricate a post-write fault. Existing binding-write event-failure unit evidence is a different dimension.

## Actual old input producer

Fixed old source baa72ac04944ccd1a1abc659084f5bc6cd148f33 / treefabfe01e74189f4986ea8c133f32a5f1ed52006b. Prefer the already trusted private pair at `.onboarding/factory-budget-history-cache-current-preflight1/frozen` for Root's first generation. Pin its manifest51949baa…1125ca, both measured binary hashes and original build RESULTS2dc019b3…a04f99. That original report has a real successful normal build command and probe, but no recorded toolchain version; do not fill that historical gap with today's version.

The new build helper can instead compile that exact public source/archive with locked offline normal Cargo bins, record actual compiler versions/source/lock hashes, freeze private500 single-link native binaries, and publish a build-only `compiled` manifest. It does not claim a runtime probe. The new generator performs the real schema15/IPC/CLI probe and real RPC history separately, then requires original unreaped-group SIGTERM/wait0, unchanged tool identities/bytes and a query-only exact logical SQLite backup before `passed`.

Real history: one registered Directory project, two registered Tasks (draft and subsequently cancelled), one personal draft, nonempty events/idempotency, exact old receipts replayed including after cancellation, and same-key/different-payload rejection. No approvals/grants/Provider/Agent login/managed workspace/Goal/Factory authority is created. Old15 exposes no edit RPC: spec2/editor history and nonempty Goal/Factory compatibility are explicitly not covered.

## Root-owned Cargo/CI connection

Root approved empty feature `actual-schema15-migration-tests`; target `task_workspace_actual15_migration`, path `tests/task_workspace_actual15_migration.rs`, required-features containing that feature. Root must register both before running ordinary Cargo suites; this source-only branch intentionally does not alter Cargo. Missing `AGENTBATON_ACTUAL_SCHEMA15_FIXTURE` fails the dedicated target. Ordinary-suite omission must be recorded NOT_RUN, with a separate mandatory explicit step for acceptance.

Root later generates the private fixture, then runs the exact16 consumer under its controlled window. The consumer checks full old rowid/SQLite Value/raw JSON/receipt/event/sequence and DDL equality, typed None serialization, immutable receipts, private old15 backup and idempotent reopen. Six controlled Task corruptions, one FK corruption and future17 refuse with all logical/version/byte evidence retained. The corrupt greatest-rowid Task is not claimed observed last by Core's unordered SELECT.

ADR: compare complete backup DDL/rows/user_version/application_id, but account for SQLite backup API's intentionally different destination schema cookie. Actual migration and rejection retain the original cookie comparison. This avoids claiming physical header identity while preserving every old logical byte.

## Consumer failure correction (source-only revision)

Root registered the dedicated target and generated genuine nonempty15 input at `/workspace/.onboarding/task-workspace-actual15-input1` using the fixed old pair. Root's integrated1ebf9e9 consumer run2 actually compiled and ran five functions: three passed, two failed. Preserve that outcome; it does not satisfy the migration gate. A narrow correction changes only backup enumeration and FK-negative-copy preparation. No producer/source identity, Core migration/DDL, ordinary acceptance comparison, Cargo/locks or genuine14 file changes.

ADR: a SQLite query-only reader may create `-wal`/`-shm` files alongside a WAL-format backup. Count only canonical `actual.pre-v16-<canonical-v4-UUID>.sqlite3` files, validate every counted file with the original owner600/single-link/NOFOLLOW/bounded FD checks, and preserve every sidecar. Keep exact-one backup and all raw/schema/version/permission comparisons. This is unrelated to the already documented backup schema-cookie allowance.

ADR: the FK negative must first create an actual orphan in the disposable copy; new rusqlite connections enforce FKs by default. Start with a clean FK check, explicitly disable enforcement for the one insertion, restore and assert enforcement ON, require exactly one managed_workspaces→projects violation tied to that inserted rowid, then close the fixture writer and invoke unchanged Store::open rejection/full-history/backup checks. Production enforcement is never disabled. Root performs the real rerun after independent review.
