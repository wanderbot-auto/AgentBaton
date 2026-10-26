# Core and native artifact-integrity independent review

Reviewed baseline HEAD `914ed5fab9d718f9eae0759cdc99332dd9cb75d4` plus the uncommitted source snapshot below in `/workspace/.worktrees/validation-integration`. AB-START-001 permits this implementation; real Provider/model calls, production release, new spending and signing remain closed. Separate independent context; no reviewer source/lock changes, staging, commits, push or subagents.

## Verdict and scope

No unresolved blocking finding in the reviewed Core/schema6/native-command slice. This verdict covers owner-approved **artifact integrity** only. It does not accept whole tasks, establish code/numerical correctness, authorize Git adoption/CAS or prove completion of AB-VAL/full Design v0.2. Reviewed workspaces primitives have their own source-bound review. Known shared Git include guards and retiring-writer integration changes are separate prerequisites and have not been silently assumed present in this snapshot. Merge needs their integration and exact-head required CI.

## Findings closed

- **F1, P2 — listing allocated raw documents before applying a byte budget.** The initial count-only Vec listing could decode many large policy/evidence JSON documents before the IPC response cap, creating avoidable daemon memory pressure. Found by code inspection; no old OOM or exploit run is claimed. `validation.rs:648` now queries only IDs/raw lengths first, refuses >100 records or >4MiB aggregate bytes before decoding, restricts SQL to two fixed table names and rechecks the remaining budget on each document read. Oversized history remains stored and returns explicit RESOURCE_LIMIT. The new regression uses oversized invalid JSON and aggregate valid policy documents; it passed in the independent full Core run. This API has an explicit refusal limit, not pagination.
- **F2, migration coverage — genuine legacy missing spec revision.** The first v5 fixture was created through the latest Store, so it still included the newly introduced field. The corrected fixture explicitly removes `spec_revision`; independent tests now prove it defaults to1 while lifecycle revision and original stored bytes remain unchanged, including the private backup.
- The primary's first full-workspace run failed an existing v1 migration assertion still searching for `pre-v5` backups. That failure remains in `/workspace/.onboarding/validation-integration-full-rust.log`; only the expected prefix was updated to `pre-v6`. The independent current Core run passes the retained backup permissions, contents and reopening assertions. This was a test expectation correction, not a passing historical run.

## Audited authority, state and persistence

- Strict IPC selection accepts a session ID and relative artifact requirements, not caller-supplied root/task/attempt/write-epoch, owner booleans or validation results. The server derives project/task/session/attempt/workspace/input and root identity. A matching stopped intent and stopped/failed session, ready registered workspace, empty lease holder and **equal session/lease fencing token** are mandatory. A later writer invalidates the earlier policy even when bytes later match. A session label alone cannot prove stop.
- Proposal finalization recomputes the full artifact contract/digest and current server references; root replacement or reference drift fails. Approved rules are reconstructed from the persisted owner policy and verified bound artifact contracts, not a public preview DTO or Agent text. Runtime checks compare current specification revision separately from lifecycle revision. The newer-spec denial branch is statically reviewed; no new spec-edit endpoint or separate dynamic spec-edit test is claimed.
- New validation requires an unrevoked matching policy revision. Exact same-key replay returns its original historical receipt without reading files again or implying current validity; mismatched payload is rejected. Result plus journal commit atomically, and injected journal failure leaves no receipt/idempotency row. Revocation retains earlier evidence. The workspace fence covers Baton writer history, not an OS filesystem freeze against external same-user writers.
- Bound file reads use the separately reviewed no-follow, descriptor/root identity, regular-file/hardlink, size/hash and read-time drift checks. This layer caps total requested artifact size at128MiB/64 artifacts and refuses validation while owned workers exist; it does not run project commands. Output hashes may be absent only in the explicitly approved rule. Existence/hash/reproducibility metadata cannot establish semantic correctness or prove who created the bytes.
- Successful records are `integrity_passed`, with `scope=artifact_integrity_only`, `correctness=not_established`, `task_acceptance=not_established`, `integration_authorized=false`. Validation does not change the task lifecycle revision, call `.accept`, merge branches or perform delivery.
- Schema6 creates separate policy/run tables transactionally. Pre-v6 backup is private0600 and reopenable. Genuine v5 task bytes are preserved; failed migration retains old version and unknown sentinel data. Existing v3/v5 sentinel tests and future-version7 refusal remain.
- Five narrow Tauri commands are registered. `validation_policy_approve` uses the existing Rust-only native propose/confirm/finalize bridge with the complete escaped server snapshot; over16KiB is refused rather than truncated. `ApprovalKind::Validation` explicitly states the limited scope. No renderer finalize, arbitrary root/command or acceptance route was added; plugin permissions remain empty. Standalone lock adds the reviewed local workspaces sha2 edge. This is a static native wiring review, not an owner dialog or macOS GUI execution.

## Independent commands and evidence

Cwd `/workspace/.worktrees/validation-integration`; Rust toolchain under `/workspace/.toolchains`:

- `cargo test --locked -p agentbaton-core`: exit0; **89 top-level tests** (including9 validation tests,2 new schema6 tests and the genuine authenticated Unix IPC normal/forged-request test). A nested killable FIFO child emits an additional1-test result and is not counted twice. The validation session/connection/confirmed-stop setup is an explicit storage fixture; it does not invoke a real Provider or native owner dialog.
- `cargo clippy --locked -p agentbaton-core --all-targets -- -D warnings`: exit0.
- `cargo fmt --all --check`: exit0.
- Primary's `/workspace/.onboarding/validation-integration-full-rust-accepted.log` and `validation-integration-clippy-final.log` were inspected separately as primary evidence; not relabeled independent execution. UI behavior has its own `UI_REVIEW.md` and was not rerun here.

No current native compile/GUI, macOS dual-architecture, installer or live vendor/account validation was executed by this reviewer. Exact-head CI and full integrated regression remain the integrator's responsibility.

## Reviewed source SHA-256

| File | SHA-256 |
| --- | --- |
| `Cargo.lock` | `d07f4bc79c5653a3bb641d68dff0cad94bd9ade97ca443ff7d1f993541064a02` |
| `crates/core/src/validation.rs` | `3ad22674ed8ba354085c4f578333cd0b4f87d6c8ee907aaa8eb082c3271fb394` |
| `crates/core/src/storage.rs` | `1dba48a47b42bbb58498d7fd746f1fe8fa8d10d1e31efe703cac11053ac61b7b` |
| `crates/core/src/ipc.rs` | `6237563ae1976b7f008339bc9ace8e82dd2dfc3df135d813bdea690c53899276` |
| `crates/core/src/lib.rs` | `98217049a32fa381e040c2eebef849e75a3fbe9c86562469f9607777218a563f` |
| `crates/core/src/policy.rs` | `f082e800c4ceaab6312324350c6a6b71e3697197636e4d47e514f05239513bb3` |
| `crates/core/src/sessions.rs` | `26debf9270d88218c8930a98548183d4bf43cdc6f3f733a5c443e380e435265e` |
| `crates/core/tests/migration_v6.rs` | `91fc8a18ba5be7895f48aed64bd25111b34fa0ac37edf10e0ce871e8a55e093c` |
| `crates/core/tests/migration_v3.rs` | `c02133aacf1107294fa4cf80c6d5b4e44aaff673f781ac77336a80f4fe276687` |
| `crates/core/tests/migration_v5.rs` | `34c88e16e47c0ec9834fd433bef174ccdbf5692bb6b8ad440ba2bf8e3fa7f404` |
| `crates/core/tests/execution.rs` | `510beb54065b036e2784751c56ff19a9c617351e4f66dd905da44ecb9d3a758f` |
| `crates/core/tests/storage.rs` | `63ae9f6e275abf767675955940c9854cd9017318a451f797456772b60bb7d9b3` |
| `apps/desktop/src-tauri/src/approval.rs` | `20529b3a0c2c3de2d3b35c16d598b3a784bcd6e84ffaee332777d513fa9bdfa1` |
| `apps/desktop/src-tauri/src/lib.rs` | `6b424769da00eb674b76af98747b207eb8a93adfc58f6abdbf43ba43c8194288` |
| `apps/desktop/src-tauri/Cargo.lock` | `8bfdfd48228f3a26a860747d3d08aea8814f794f9218d721142a5e3686442aad` |
| `apps/desktop/src-tauri/capabilities/main.json` | `090491e6dce371a7853acf44c4a65c38e2e372ac50df6a5e313a36a5ec8acf9a` |

## Independent retained logs SHA-256

| Log | SHA-256 |
| --- | --- |
| `artifacts/validation-integration/reviewer-clippy.log` | `59639588a6f1826f129cbcd121edc07279dab94786f5fa5bde6b3439b2e845cc` |
| `artifacts/validation-integration/reviewer-core.log` | `e6152b0e515ac19f4215b2b4445a541fd061f8064d4f157757aea75c63fefaa9` |
| `artifacts/validation-integration/reviewer-fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
