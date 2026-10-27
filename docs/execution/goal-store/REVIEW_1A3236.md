# Independent GoalStore / schema 9 review

Reviewed source commit `1a3236a94b1b6a6346d1ee604bb23b1a595425bc` in `/workspace/.worktrees/goal-store`. Current HEAD `b43b396912527906a83698ab710d6a13303d9342` adds only the author's five documentation files after that freeze. Verdict: R1/R2 are closed and the independent frozen-source Backend regressions pass. Two subsequently reported UI/history-read integration issues remain open below; this source is not final integration-ready or whole Goal / Design v0.2 acceptance.

The reviewer changed no source, dependency, lock, index or implementation evidence. Only this review was added. Shared Cargo ownership was explicitly handed over after the separate Command fixture review and released to the primary after the checks below. Touching this tree's lib.rs changed only its timestamp to avoid stale cross-worktree metadata; source bytes were checked against the frozen Git objects before and after verification.

## Findings and closure

**R1 — real execution counters could reset.** The original finalizer inserted missing actual `budgets` rows with consumed/active zero despite recovering shared history from other tasks. These rows are used by real attempt admission, so correct preview counts alone were insufficient. The author preserved an actual old-source regression failure: newly created root consumed was 0 instead of 1. The fix seeds absent rows from recovered facts and reconciles existing rows upward with MAX, preserving established total/parallel limits. Its reconciliation event, grant CAS/history, Goal record, proposal consumption/result and confirmation events share the same IMMEDIATE transaction. Independent tests now prove actual root consumed starts at 1, seven further attempts exhaust limit 8 while project capacity remains, and the ninth shared-root attempt is rejected. A confirmation-event trigger rolls back upward reconciliation, grant, result and journal together; replay adds no event. Historical consumption is never reduced or converted into a fresh budget.

**R2 — unreadable ledger could become fresh zero.** The original size-filtered SELECT treated an existing over-1MiB ledger as absent. The preserved old run returned a ready decision with known-zero dimensions. The fix uses one SQL CASE statement to distinguish no row from an existing over-limit row before materializing its text. The latter returns RESOURCE_LIMIT. The same bounded presence distinction now protects both payload and result of approval/plan idempotency records. Independent tests cover oversized meter preservation and all four cache payload/result cases without new history or authority.

The original source snapshot remains `/tmp/goal-store-review-old.rs`, SHA-256 `cd580c792c1a0e6de7c9799bf70dc1b43f44c06fad65e63b6903a1849235d7c0`; it equals the reviewer's earlier static snapshot. The old source control log `/tmp/goal-store-review-controls-old.log` is SHA-256 `83dfdcfc18541c298499bfc35152ca0b753f7aa396d0f14683d19985bff2191f`, actually exit 101 with 23 passing and the two failing tests. The reviewer inspected this preserved log and source; the old negative run was performed by the author, not rerun by this reviewer. Earlier compile/setup, cache and complete-snapshot-cap failures remain historical evidence and are not relabeled as final success.

## Subsequent integration issues still open at this freeze

The primary's real Goal UI harness identified an initial empty-history contract mismatch after this independent Cargo run: `list_goal_plan_revisions` calls `current_plan`, which returns NOT_FOUND when no head exists, whereas the UI contract requires an empty list. The reviewer confirmed that path statically. In addition, both `get_goal_plan` and `list_goal_plan_revisions` call live `read_scope`, so revocation, expiry or input drift prevents historical plan inspection. This conflicts with the subsequently clarified history-read contract. These issues are not fixed in source 1a3236 and were not tested as successful by this review. The author is preserving old failing controls and preparing a separate minimal fix; no further Cargo run is claimed here.

Historical reads must remain data-only and verify their stored lineage/digests; changing them must preserve live revocation/input/one-shot checks for new proposal/revision/ready actions. Existing tests that check drift through get currently prove the old read behavior, not the new history contract. The eventual source and combined UI require separate exact-source validation. The 207 passing tests and the R1/R2 closure do not waive these integration issues.

## Contracts inspected

Server resolution reads actual Task/Project, current grant and native-confirmed records, pinned workspace, immutable connection and bounded input bytes/SHA. Proposed JSON cannot deserialize ApprovedGoalScope, trusted plan evidence or a passed owner result. Separate consumed kind=goal confirmation is required; grant approval alone is insufficient. Only the displayed children/depth ceilings and grant revision change. Other capabilities and bindings remain equal, full old grant revisions and old attempt references remain available, and active/unresolved intents or even a retained dangling command fence prevent new Goal authority.

Full private resolved-record digests are checked at finalization and reload. The native review snapshot retains the 16KiB cap: shared immutable grant fields reconstruct exact before/after grants, and complete approved_contract_json round-trips the permission-bearing validator contract. It is not a digest-only substitute. Existing Integrity/Data/Command definitions supply additional fixed rules, not passed Goal evidence or Task acceptance. Command one-shot use is checked against actual run history and is not renewed by a Goal reference. Integrity and Command definition flows have actual positive controls; Data definition resolution was additionally inspected in source.

New plan/ready actions recheck live grant revocation/expiry, scope, task specification, project/workspace identity and clean input revision, stored connection snapshot, input bytes and referenced policy revisions. Later approved ceiling-only revisions do not replace old scope bindings. Raw plan_json is capped at 256KiB before direct strict serde parsing, preserving duplicate-field rejection; exact original text participates in idempotency, and canonical typed content defines the validated digest. Revision/head CAS, immutable lineage, result and journal commit atomically. Committed same-key replay is historical data and grants no fresh authority after revocation.

Ready preview uses a read-only transaction and makes no reservation, dispatch, attempt increment or TaskAccepted transition. Historical runtime/output dimensions remain unknown; old evidence/actions cannot be remapped into successful nodes or bypassed with a new scope/key/revision. The only generated passed input check follows a fresh bounded read and exact SHA comparison. Existing validator receipts are not adopted as Goal completion. Scope/plan count and raw-byte limits reject incomplete/oversized reads rather than truncate history.

Schema 8→9 was checked with actual old-shape fixtures: raw task/blob preservation, mode-0600 reopenable pre-v9 backup, transactional new-schema rollback preserving unknown owner data, and actual future-v10 refusal without changing database bytes. Legacy pre7/pre8 assertions remain; fixture changes remove new tables and advance expected final/future versions. Workspace libraries and root/native lock files are byte-unchanged from Core88.

## Independent commands and results

All commands ran under umask 022 with CARGO_HOME=/workspace/.toolchains/cargo, RUSTUP_HOME=/workspace/.toolchains/rustup, its bin on PATH, CARGO_TARGET_DIR=/workspace/.worktrees/validation-integration/target, CARGO_BUILD_JOBS=2 and CARGO_INCREMENTAL=0. No concurrent Cargo writer or new target was used.

| Command | Observed result |
| --- | --- |
| cargo test --locked --offline -p agentbaton-core --test goal_store --test goal_store_migration --test goal_plan -- --test-threads=2 | 48 + 27 + 3 passed; no failures/ignored/filtered |
| cargo test --locked --offline -p agentbaton-core | 207 unique top-level tests passed; raw 208 includes one existing FIFO nested child reported twice; 0 doctests |
| cargo clippy --locked --offline -p agentbaton-core --all-targets -- -D warnings | PASS |
| cargo fmt --all -- --check | PASS |
| git diff --check | PASS |

The complete count is the prior 129 Core tests plus 48 Goal contract, 27 GoalStore and 3 schema9 migration tests. This run covers the real Unix IPC flow and generated local command/control children, not a vendor/account/model request.

| Independent local log | SHA-256 |
| --- | --- |
| /tmp/goal-store-independent-controls.log | c0de3274d7646a16df91f4080d7fb1445209d65dceb66718b4a23362f07978f1 |
| /tmp/goal-store-independent-core.log | 221909816c3207ad1e0357e743d30f552ab6946f75982cb49b11780adba038f6 |
| /tmp/goal-store-independent-clippy.log | f363263643fb0df8421a8a6ba9606a78cd8a41d5dab5caa2c3585d016d9e82a5 |
| /tmp/goal-store-independent-fmt.log | e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 |

## Frozen source fingerprints

Every file below was checked byte-for-byte against source commit 1a3236 before/after tests. The local manifest `/tmp/goal-store-independent-manifest.json` also verifies the author's listed source/log hashes; SHA-256 `c9f9395f66edc27da5852b597c36a0945425ee23194fc7c86bb61a729f1dee6f`.

| File | SHA-256 |
| --- | --- |
| crates/core/src/goal_plan.rs | 243ef4b34ef963d41a07287360a192a4f9336c21ccc48b6650170793cac81c21 |
| crates/core/src/goal_store.rs | 38bfe1f629831ffd1ae5b951590e89a7b82cc909ac2b825c0daf4f74d1588421 |
| crates/core/src/ipc.rs | a18e3508921d466db8619e857687a9e11d91bb513754904cd5be9f3d099ae1b2 |
| crates/core/src/lib.rs | 2d732f787e7e8e416c8473323be922aa1e1fe5fbe70dae8ce5733a5fb9e1ad43 |
| crates/core/src/sessions.rs | 6bccef078d6f4fa4201fd8cb7405469d6f362f9ff45a28d4650a6e1ae24ebee9 |
| crates/core/src/storage.rs | 8a7e75de1bf7006672cd32999d48022509497d253cbd84a32ec1d4cd7b078b52 |
| crates/core/tests/command_validation_migration.rs | ce112630eb96c9881530d459bd5b40c473cdac89b425df554e78b9ed5c099d37 |
| crates/core/tests/goal_plan.rs | 8430aa22a3cf76c5d027a0465d1ef4760192e04b482fc4a5a0d88820e037c0e8 |
| crates/core/tests/goal_store.rs | 8b2735ca7646a83695461afed0481e4aad80d9f9d2eeaedf7fcf2cafe8102fbb |
| crates/core/tests/goal_store_migration.rs | 91862b7cb28b1ce7dac88ec3c3befbdad1e05cfa2dec07ae8db072567de65752 |
| crates/core/tests/migration_v3.rs | 446e3a5b7200d1ba24813a07a9f71093060903465d011f297670cce6bed36a7f |
| crates/core/tests/migration_v5.rs | 5800ce54d84ab89ae034a93a75c8cbc5d144c1387aab9dda6b676a18380ad34e |
| crates/core/tests/migration_v6.rs | 0c91ebbfdccf2b3e74c14d1cffa90a4b2b8df0b90d62894981f08c06ed79bdad |
| crates/core/tests/migration_v7.rs | 16a496a8d5cad59775c6c4c542231d78705db173e27feaf847b9dcd79eca27cb |
| crates/core/tests/storage.rs | 65518ed49d5fb687ab3415631e241508e3d1f4268a82bd01eddbdce5dfb85876 |

## Validation limits

These tests use private local SQLite and trusted host/native-finalizer calls, with explicitly synthetic connection readiness and simulated owner confirmation. Those are trust premises, not protection against an arbitrary writer of the private database or malicious trusted Rust host. They do not establish actual executable/account readiness, continuous filesystem isolation or an OS sandbox.

This review ran no browser, real Goal owner dialog, macOS build/window/package or native Goal approval flow. Complete snapshot readability is still unverified on the actual dialog. The sibling UI/native review and historical CI are separate evidence; this Backend source and the eventual combined tree require their own integration and exact-source CI. No natural-language planner, real Goal dispatcher/Router, child reservation/settlement, complete runtime reconciliation, reliable token/cost meter, scientific correctness, Git adoption, TaskAccepted or production release is established. No real Provider/model/account or additional paid service was called.
