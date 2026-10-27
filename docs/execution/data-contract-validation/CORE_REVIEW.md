# Independent schema7 data-contract Core review

Reviewed frozen HEAD `ddd6707e5c67ff66f65f1ab243f868ee35366959`: data implementation `0f23ff0` on validation baseline `3c1cd1a26b99350f13d14645118bb0a72f87b1de`, with reviewed JSON library cbd55093/a088529 and Root130f approval-outcome guard composition. AB-START-001 and OWNER-001/002 remain the authority; no permission expansion. Separate reviewer context from the Data/Core author. Only this review document was written; no source/lock edits, staging, commit, push, remote action or subagents.

## Verdict and independence boundary

No unresolved blocking finding in the reviewed **schema7 Data/Core** scope. Ready for the integrator's final exact-source checks and required CI. This is not full Design v0.2/AB-VAL acceptance or current macOS/native-owner verification.

This reviewer authored the earlier AuthProbe production hunk and canonical executable test helper. Those changes are **excluded from this independent verdict**. Their separate independent review is `docs/execution/approval-outcome-guard/REVIEW.md`, including the8441fe6 fixture follow-up. That document was read, not relabeled as this reviewer's work. The diff0f23ff0→ddd6707 has only that already-reviewed production IPC hunk plus the three approval_outcomes tests and documentation; Data implementation/schema/native/UI/locks are unchanged by that merge. The Data dispatcher/finalizer routes remain separate from the connection-only probe branch.

The reviewed deterministic library has its own authority/parser/resource review at `docs/execution/deterministic-validators/REVIEW.md`. Its test/source evidence is not substituted for the independent Core execution below. Native and UI reviews remain their own contexts in NATIVE_REVIEW.md/UI_REVIEW.md; no repeated native/browser acceptance is claimed here. The primary's concurrent prepare-sidecars script correction is outside this Core snapshot/review.

## Authority and exact version binding

- Strict `DataValidationSelection` accepts only session ID, relative input/output requirements and the typed bounded table contract. It cannot supply approved flags, root, task/attempt/spec/input/dataset bindings or results. Nested contract DTOs are proposals; serialized views and the record's evidence Value are never deserialized into an authority/acceptance object.
- `resolve_data_validation_selection` derives project, task, session, attempt, workspace and execution binding from Store. Shared `validation_workspace` requires matching intent/task/attempt and confirmed Stopped receipt, stopped/failed session, nonarchived project, ready pinned workspace, empty lease holder and **equal session/current fencing token**. A stopped label alone is insufficient; a later writer invalidates old policy even if bytes match. Root identity/linkage is checked. This is Baton writer fencing, not an OS filesystem freeze against external same-user changes.
- Specification revision is separate from task lifecycle revision. Both `input_revision` and Data `dataset_revision` derive from the immutable attempt input binding. The selected input hashes bind actual files; the dataset identifier is the recorded attempt input revision, not an inferred population/catalog version.
- The native proposal kind is independently `data_validation`. Full canonical file/table/validator/digest and server references are shown in the proposal and recomputed at finalization. Changed specification/root/references fail. Only the trusted local Store finalizer creates the persisted policy; public/native DTOs cannot manufacture its approval.
- Each new run additionally requires the matching consumed owner proposal with a persisted result, matching data kind, approved file/table/validator/digest snapshot, current unrevoked policy revision and reconstructed server subject. The complete private ApprovedJsonTableRule is rebuilt from trusted stored policy and verified bound artifact contracts; canonical digest and contract are compared again. A policy created before failed approval.confirmed metadata cannot run until the same proposal is reconciled and consumed. No approval boolean or deserialized acceptance token is accepted from a renderer.

## Contract-only execution, limits and persistence

- Exactly three outputs and1–16 fixed-hash inputs are selected under the registered workspace. The reviewed library caps total approved artifacts at16MiB, each at the bounded table byte limit (at most4MiB), with independent row/depth/value/string limits and duplicate/path protections. Bound regular-file/no-follow/hardlink/root identity and read-time drift checks remain. Core rejects new runs while any owned worker exists before looking up a policy or validating files; its IPC read timeout is the existing60s heavy-operation class, not a Provider/runtime budget extension.
- Only the fixed built-in JSON-table validator runs. Reproduction script/environment bytes are hash references, never executable commands. Population size, sample/full designation, units, timezone and seed are owner-approved declarations checked against output metadata. They do not prove parsing of the input population, execution of a reproduction script, scientific/algorithmic correctness or native account identity.
- Outcomes are contract_passed/contract_failed with scope data_contract_only, correctness approved_schema_ranges_metadata_only, task_acceptance not_established and integration_authorized=false. Stable failed receipts persist their actual failure codes. JsonTableContractAcceptance is a separate private type used locally and not elevated to generic task/adoption acceptance; no task lifecycle update, CAS, merge, script or Provider call is introduced.
- Data policies/runs have independent SQL tables, event names and namespaces. Integrity policies/runs remain untouched. Real Unix IPC tests exercise normal/failed checks, reject forged approval/root/binding/task/result fields and refuse using a Data policy in the integrity namespace.
- Exact same-key/payload replay returns historical evidence before any fresh file read, even after revocation/drift, and never asserts current validity. Changed payload/revision with the old key conflicts. New keys recheck revocation/current subject/fence/contract. New receipt plus journal commit atomically; a trigger fault leaves neither receipt nor reserved key, and same-key retry succeeds. Owner policy plus its own journal is also atomic. The later confirmation receipt is a separate existing transaction; unconsumed policies fail closed until reconciliation, with no duplicate policy.
- Revocation uses optimistic revision and an immediate transaction; a new run rechecks the entire current policy/revision/revocation at its commit boundary. Historical evidence remains. The daemon is the intended sole database writer; this is not authentication against arbitrary same-user SQLite mutation.
- Raw policy/proposal/replay documents are size-checked before decode. List metadata is read first and capped at100 records/4MiB aggregate raw bytes; single reads and replay payload/document each have4MiB guards with bounded SQL rereads. Table selection is a fixed allowlist (two old integrity plus two Data tables), never a user SQL identifier. Oversize/history limits return explicit RESOURCE_LIMIT without deleting or silently skipping records. These Vec APIs refuse oversize listings; they are not pagination or complete unlimited-history acceptance.

## Schema7 and retained migration assertions

Store refuses version8 before migration, creates a private0600 pre-v7 backup for prior schemas, and creates Data policy/run tables plus version7 in one transaction. The actual v6 fixture preserves exact formatted task document bytes and an unknown binary blob in both upgraded database and reopenable backup; reopening does not make a second backup. A conflicting unknown run table forces schema7 transaction rollback: version remains6, new policy table is absent and original sentinel/task data remain. Future8 raw database bytes are unchanged. Existing legacy downgrade fixtures only drop the newly added tables and update current-version/pre-v7 expectations; original source backup versions, raw bytes, permissions and sentinel assertions remain.

This review does not claim an all-version multi-step upgrade is one global transaction; the existing per-version migration/backup behavior remains. The dynamic rollback proof here is the new v6→v7 transaction.

## Independent actual commands

After primary released its target, cwd `/workspace/.worktrees/data-contract-validation`, explicit umask022, CARGO_HOME/RUSTUP_HOME under `/workspace/.toolchains`, CARGO_BUILD_JOBS=2 and existing CARGO_TARGET_DIR `/workspace/.worktrees/validation-integration/target`:

- `cargo test --locked -p agentbaton-core --lib data_validation::tests`: exit0,13 passed,23 filtered,4.51s. Includes actual authenticated Unix IPC, metadata/contract failures, namespace separation, owner-confirmation drift, real artifact symlink/size failures, stop/fence/spec/input checks, revocation/history, transaction fault rollback and predecode byte/count limits. Sessions/connections/stop proof are explicit synthetic never-dispatched storage fixtures; no native owner click or Provider is invoked.
- `cargo test --locked -p agentbaton-core --lib ipc::frame_tests::data_validation_run_refuses_owned_workers_before_any_file_validation -- --exact`: exit0,1 passed,35 filtered,0.01s. Synthetic owned cleanup fixture; journal remains empty and no policy lookup/file validation occurs.
- `cargo test --locked -p agentbaton-core --test migration_v7`: exit0,3 passed,0.01s.

Total17 independent targeted tests, zero failed/ignored. The source fingerprints were rechecked after execution. No redundant full workspace/Core/Clippy repeat was performed: primary's original321-top-level/2-doctest Core108 evidence and later324-top-level composition/Clippy/fmt/release build are explicitly primary results. The original CORE_EVIDENCE contains12 source fingerprints:11 match this final snapshot; its IPC fingerprint describes the earlier pre-AuthProbe source. All26 original log hashes matched when inspected. The merged approval_outcomes fixture belongs to the separate independent review cited above.

No current native compilation/GUI/dialog, fresh Mac dual-architecture CI, installer, live Provider/model/account, remote host, signing or production result is asserted by this reviewer. These remain separate acceptance dimensions and full-product scope remains open. Independent logs below are local /tmp evidence; the integrator should retain them in final evidence before replacing the environment.

## Reviewed final Core source SHA-256

The IPC hash binds the whole final file; its self-authored AuthProbe hunk remains excluded as stated above.

| File | SHA-256 |
| --- | --- |
| `crates/core/src/data_validation.rs` | `5c5ee6842f1342e95f9e19d39591c25c0ec5716d89f30e3384b0967ce670b5ce` |
| `crates/core/src/lib.rs` | `89e59ad6b1afaca403859095646e4b93d5d8ebcc6ae8d61ba2a6014f78532770` |
| `crates/core/src/storage.rs` | `6e9a8cb81d88586200f61a4ecd81f11c9e0637370c1a07254ae873eb4deea4ac` |
| `crates/core/src/sessions.rs` | `b861a6486d8f6498e78343b1b7d15a879b62516fb01782823c9b90bfeffc37bb` |
| `crates/core/src/ipc.rs` | `8084d7e193e84e425d6d36518bcb617ab8513aa19b6bfe4a066a95f0dfbbcd64` |
| `crates/core/src/validation.rs` | `54fb2c26f8e483a430e6af29966d302cb89ff3437d3318e1059587550aded2ac` |
| `crates/core/tests/execution.rs` | `cc275e1641e4e32aed89f4c3de5b7284cad84824b031bcd5c52b0fbc7b30d113` |
| `crates/core/tests/migration_v3.rs` | `927f908cefe42b762948eaca7e6f1677e776f2cdb34be1343cd4841948b4468c` |
| `crates/core/tests/migration_v5.rs` | `75c9752bf14d7fa6b3091104bd9828930f94f6a604321e04b6241c2bf8110db0` |
| `crates/core/tests/migration_v6.rs` | `9dfc5deb7eb55b3deb8487406b6fc8cb83425282e9530b472ec71adff2127cca` |
| `crates/core/tests/storage.rs` | `50a853c57c706fd27f7f03be31da0a47d85eec1faa356967e87bd7ee2a870b71` |
| `crates/core/tests/migration_v7.rs` | `6afbd809d0320b4950475da9720edbec840eb2c7a7ce69989ac6952a78d3c4c9` |

## Independent logs SHA-256

| Log | SHA-256 |
| --- | --- |
| `/tmp/ab-review-data-core-data.log` | `1eb5571b37de91b9305f47a2228870d91880f6b3109dc3f80c28773a7fe1a389` |
| `/tmp/ab-review-data-core-busy.log` | `46b848d4818a7e27df1e215772f8cd46f9bd93f8b5123b03fd77ce47a6fdfdcd` |
| `/tmp/ab-review-data-core-migration.log` | `97ba41f0baab989d13217569bcaa42b722dc8a44c04bbd271585be10307b67de` |

Historical Core evidence SHA-256: `63d7005b75632da7a608bfff378729ec3a477777f94b31f36faed54d3b50bda4`. Separate approval review SHA-256 at inspection: `60b99fae03da238b5938526be398faf9100e450a179ca095527c77d7ed35b14e`.
