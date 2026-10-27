# Validation/runtime composition review and regression correction

Reviewed merge-in-progress: validation HEAD `9895ba669bfcae6190fb9d2d4b0a1f6127977910` with incoming runtime `9edf14e152e59f98971ffce925814a520a33a0a7`. No merge commit existed during this review. Scope: adapt the single conditional-configuration regression and review Core actor/schema6/artifact-policy composition. Reviewer changed only `crates/workspaces/tests/validation_adoption.rs` and this document; no production/lock changes, staging, commit or push. Because the reviewer implemented the test adaptation, that small test edit is self-reviewed; the production composition assessment uses a separate execution context from its authors. Primary must review/commit the final merge and run exact-head CI.

## Regression and correction

The preserved initial full workspaces run at `/workspace/.onboarding/validation-composite-workspaces.log` has41 passing tests and1 failure. The old conditional-driver fixture called `inspect(...).unwrap()` after installing includeIf configuration. The newly merged shared guard correctly refuses all include/includeIf indirection before that inspection. The failure does not indicate execution of the configured driver or a production safety regression.

The corrected fixture creates the legitimate `AuthorizedIntegration` before configuration drift. After installing the same generated driver/includeIf configuration, it retains the direct Git configuration-visibility controls (linked root sees no driver, pinned common Git context sees the driver), explicitly requires `inspect` to return UnsafeRepository and still requires `prepare_candidate` with the previously issued authorization to return UnsafeRepository. It does not require a specific later internal audit to execute when an earlier guard already refuses the repository.

All original marker, linked-index, target-ref and absent-destination assertions remain. Added invariants require source index, source/linked file bytes, common configuration, included configuration and source HEAD to remain unchanged. No helper is authorized or deliberately executed. The fixture continues to test executable configuration and refusal, not successful external execution. No production guard, negative assertion or resource bound was removed or weakened.

## Composition verdict

No new blocking issue found in the reviewed composition. `validation.rs`, schema6 storage, live policy checks, native ApprovalKind/commands and both lockfiles are byte-identical to the already reviewed validation HEAD. Integrity receipts still use server-derived task/session/attempt/input/root contracts, require matching confirmed stop and session/lease fencing epoch, reject later writers, and never confer code correctness, task acceptance or integration authority. Renderer cannot finalize approvals. The added shared Git guards reject includes earlier and retain raw-object/promisor protections; local validation/adoption primitives and their42 tests remain present.

The actor production delta is the previously reviewed ownership handoff: Daemon Drop reaps owned workers/probes before removing its own socket and explicitly unlocking. A mechanical comparison confirms the original Supervisor Drop body is moved verbatim; repeated field Drop sees drained collections. This cleanup performs no database mutation and cannot fabricate a durable stop receipt. Graceful shutdown still persists authoritative receipts; fatal paths retain conservative restart reconciliation. New duplicate-descriptor and active-cleanup lock tests pass in the actual schema6 composition, alongside both first-frame tests and authenticated validation IPC tests.

Incoming native WebView harness/runner fixes have their own independent reviews. This record does not claim to rerun those GUI lanes or establish full Design v0.2 acceptance. No live vendor/model/account, paid resource, remote host or production operation was performed.

## Actual independent commands

Cwd `/workspace/.worktrees/validation-integration`, managed Rust toolchain:

- `cargo test --locked -p agentbaton-workspaces`, umask022: exit0,42 tests (2 unit,5 shared guard,18 workspace safety,17 validation/adoption). Rerun after the final formatting adjustment also passes42; final log fingerprint below.
- `cargo test --locked -p agentbaton-core`: exit0,91 top-level tests. The additional nested FIFO child emits1 result and is excluded from this count. Includes22 lib tests (9 validation plus duplicate-FD/active-cleanup handoff), migration tests, real Unix IPC and generated Provider child lifecycle tests.
- `cargo clippy --locked -p agentbaton-workspaces --all-targets -- -D warnings`: exit0.
- `cargo fmt --all --check`: exit0. An initial direct formatting check identified one multiline assertion; it was formatted in the sole allowed test file before final regression.

No fresh full-workspace, desktop/native GUI, macOS installer or remote exact-head CI run is asserted by this reviewer. Primary's final merge and broader checks remain necessary. Independent logs under `/tmp` are current local evidence; primary should retain them with the final integration evidence before replacing the workspace.

## Reviewed source SHA-256

| File | SHA-256 |
| --- | --- |
| `crates/workspaces/tests/validation_adoption.rs` | `eba06f2101967215f73705a83bd8f1dbce0cb02e1456bf5df65e8d59c886a6a8` |
| `crates/workspaces/src/lib.rs` | `793468aa4586138ae8dcb7d4f249fe4ce0e966e3f8b93b79db1f8f65f854f379` |
| `crates/workspaces/src/adoption.rs` | `be970ed01ee18bebe9b06e62087d0bd7c52e97bfca6db0dcaec60cbedeba9472` |
| `crates/core/src/ipc.rs` | `100e552744cf02caa64734f554b37835487f0e07f522255633d62f2a4af112d3` |
| `crates/core/src/sessions.rs` | `f5e8a4ce96c767d9b988017f3ecb41159ecd99276b569444a51e68b57010cfe7` |
| `crates/core/src/validation.rs` | `3ad22674ed8ba354085c4f578333cd0b4f87d6c8ee907aaa8eb082c3271fb394` |
| `crates/core/src/storage.rs` | `1dba48a47b42bbb58498d7fd746f1fe8fa8d10d1e31efe703cac11053ac61b7b` |
| `crates/core/src/policy.rs` | `f082e800c4ceaab6312324350c6a6b71e3697197636e4d47e514f05239513bb3` |
| `apps/desktop/src-tauri/src/approval.rs` | `20529b3a0c2c3de2d3b35c16d598b3a784bcd6e84ffaee332777d513fa9bdfa1` |
| `apps/desktop/src-tauri/src/lib.rs` | `6b424769da00eb674b76af98747b207eb8a93adfc58f6abdbf43ba43c8194288` |
| `Cargo.lock` | `d07f4bc79c5653a3bb641d68dff0cad94bd9ade97ca443ff7d1f993541064a02` |
| `apps/desktop/src-tauri/Cargo.lock` | `8bfdfd48228f3a26a860747d3d08aea8814f794f9218d721142a5e3686442aad` |

## Evidence SHA-256

| Log | SHA-256 |
| --- | --- |
| `/workspace/.onboarding/validation-composite-workspaces.log` | `5540212f7789622dd24b9a07188c51dc6bb85588652409cd70fd6d2fce45dab5` |
| `/tmp/ab-review-validation-composite-workspaces-final.log` | `90ce72026fa83aade12b21d78aa5acb68cc4ef1a2568c96fe7505b877250c17a` |
| `/tmp/ab-review-validation-composite-core.log` | `5489f441b21c3262dbb54e3e99f1272b9e9590e423d0eb0896dd397cc922f2d2` |
| `/tmp/ab-review-validation-composite-clippy.log` | `73e851148f0f72e1293700296c7968129c0216313c52dc06814a04837d2f49b4` |
| `/tmp/ab-review-validation-composite-fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |


## Follow-up — private runtime database fixture under umask022

At the subsequent composite checkpoint, the primary's actual full-Rust run failed `actual_authenticated_ipc_uses_server_policy_and_rejects_forged_result_fields`: Daemon bind correctly returned PERMISSION_DENIED for the existing nonprivate database. That preserved failure is `/workspace/.onboarding/validation-composite-full-rust.log` (SHA-256 `13730e264e71cc9aacbfbbbf109bbbd470ab96170c3a2d9ce627d42d105cf9ac`). The0700 temporary directory alone does not make a Store-created0644 SQLite file private under umask022. Earlier local Core passes above are historical and do not supersede this observed failure.

Independently reviewed the narrow fixture correction: inside cfg(test) only, create a new empty database with create_new+0600, assert the actual0600 mode and close that temporary File before Store::open. This matches Daemon's private-file preparation without changing permissions on existing files, production Store behavior or runtime privacy checks. The import addition is test-only. A mechanical comparison of the production portions of validation.rs against280c92a shows they are byte-identical. The actual Unix IPC normal/forged-field assertions remain unchanged.

Independently executed, explicit umask022, cwd `/workspace/.worktrees/validation-integration`:

`cargo test --locked -p agentbaton-core --lib validation::tests::actual_authenticated_ipc_uses_server_policy_and_rejects_forged_result_fields -- --exact`

Exit0;1 passed,21 filtered,0.77s. No full Core/workspace repeat or native GUI proof is asserted for this follow-up. No remaining blocker in this fixture correction. Primary's broader283-Rust/79-UI/native/browser evidence remains separately attributed to the primary.

Final reviewed `crates/core/src/validation.rs` SHA-256: `b1e507f8c23c8e66c1bed99669bdc2a14578e1e943ebaf179edbf2ba796c3c51`. Independent log `/tmp/ab-review-validation-private-db.log` SHA-256: `570fe062c72ac7db219eebc6cd6d862a16a3a9efcd8bff828967cad2f5a5b4a1`. Primary committed this source while review was completing (current HEAD40457076fb88a45f30c4a7212d00ce9d38aed684); reviewer modified only this document, not source/locks or Git state.
