# Independent final integration review

Reviewer: native `/root/integration_review`, separate execution context from the integrator. Reviewed `636dbdd6e685d4fcb229aef100c6f64867ee1343` plus the frozen uncommitted integration bytes below on `codex/provider-runtime-adapters`. Protected `origin/main` observed at `1f34b444821640bf88f32a8bb39c6434d4ca26cc`. OWNER-001/002 and AB-START-001 were read; no sources/locks were edited by this reviewer and no vendor executable, private identity, model request, production action or extra paid service was used.

Verdict: no unresolved blocking source finding in this integration slice. This is suitable for feature-branch/CI review, not full Design v0.2 acceptance. The earlier history count-bound finding is closed: older history replaces the visible window when combining pages would exceed either 500 records or 8 MiB; the older-page control remains available and the returned forward cursor permits continued reading. The original normal-page preservation assertions remain intact. The new count and Unicode-byte regressions passed independently. A final follow-up also prevents a history-only RESOURCE_LIMIT error from disabling authenticated task cancellation: snapshot connectivity and secondary read results are now handled separately; the error remains visible and successful task/session projections still refresh. Its new cancellation regression passed independently.

## Independently executed checks

All commands ran in the actual shared Linux workspace; generated local Python children were the protocol peers. Rust used `CARGO_HOME=/workspace/.toolchains/cargo RUSTUP_HOME=/workspace/.toolchains/rustup PATH=/workspace/.toolchains/cargo/bin:$PATH CARGO_BUILD_JOBS=2`.

| Command and working directory | Observed result |
| --- | --- |
| `cargo test -p agentbaton-core --locked` at repository root | PASS, 76 top-level tests: lib 9, execution 7, IPC 5, migration v3 2, migration v5 2, policy 13, project IPC 3, registry 10, sessions IPC 17, storage 8. The nested FIFO child is not counted again. |
| `python3 apps/desktop/src-tauri/tests/test_native_runner.py` at root | PASS, 10 tests, including missing/stale/malformed evidence, fatal/timeout cleanup and artifact inspection failure/marker contracts. |
| `npm run typecheck && npm test -- --reporter=dot` at `apps/desktop` | PASS, typecheck and 39 tests before the count-bound correction. |
| `npm test -- --reporter=dot` at `apps/desktop`, after count-bound correction | PASS, 40 tests / 7 files on the final frozen frontend bytes. |
| `npm run typecheck && npm test -- --reporter=dot` at `apps/desktop`, after the history-read connectivity correction | PASS, typecheck and 41 tests / 7 files on the second final freeze. |
| `cargo fmt --all -- --check` and `git diff --check` at root, after the second freeze | PASS, exit 0. |
| `artifacts/browser-venv/bin/python -m pip check` | PASS, no broken requirements; inspected CPython 3.12.14, Playwright 1.62.0, pyee 13.0.1, greenlet 3.5.5, typing_extensions 4.16.0. This is dependency evidence, not browser execution. |

The core command covered the actual new native/ACP generated-process integration, exact runtime binding, legacy-plan rejection, local launch-failure receipt and reservations, cancellation, schema5 success/failure backups and eight storage regressions including Unicode byte limits and pre-decode rejection of an oversized single record. Core/native-audit bytes were unchanged at the second final freeze; the later IPC portability correction and private test-fixture changes are reviewed separately below. Frontend tests use mocked transport and are identified as such.

## Boundary assessment

- Provider labels map to explicit routes; native launch argv is generated, arbitrary native flags are rejected, and selected-route failure does not try another engine/identity/billing path. Custom ACP literal argv remains part of owner-visible recipient evidence.
- The current connection authorizes only the explicit generated default profile. Logical plan/materializer/native-contract digests bind the configuration revision; legacy missing plans and broader selected profiles fail closed. This does not implement the remaining selected Skills/MCP/model-profile product scope.
- Per-attempt materialized bytes are durably bound before dispatch, verified again immediately before launch, and associated with exact session/attempt/configuration IDs. Rebinding different bytes fails. Snapshot integrity is local evidence; native loading, hidden native inheritance, account identity and OS enforcement remain unknown/unproven.
- Native owner grant proposals now identify the server-resolved name, Provider/route, executable, argv and digest alongside exact immutable bindings. Finalization compares the complete current references. The two-recipient drift regression passed.
- Prepared/sent/local-no-start receipts preserve distinctions. A local launch error cannot be labeled no-start if an owned worker exists. Confirmed no-start releases active reservations but does not refund consumed attempts. Cancellation waits for owned stop evidence. Native submission ACK does not complete a turn or accept a task.
- Journal byte lengths are inspected before each document is loaded/decoded. Pages stop at the byte bound; a single oversized record returns an explicit error with its sequence and is not skipped. Frontend forward/earlier pages remain bounded and disclose partial evidence.
- CI adds generated-wire validation against a hash-verified immutable official schema, actual frontend/Unix-IPC browser validation, and real macOS WebView/release-bundle audit lanes. Browser dependencies are pinned with Linux CPython3.12 wheel hashes and installed with `--require-hashes --only-binary=:all:`. Existing three Environment checks and their workflow bytes are unchanged from protected main.
- The release marker inspection fails closed if `strings` fails; grep inspection errors cannot print success. The actual default release executable remains a CI validation target.

Two nonblocking limits remain explicit: `sessions.create` returns its pre-launch projection, so the first response does not yet include the subsequent runtime binding/revision (normal UI refresh obtains it); initialization failures after a child started conservatively remain outcome-unknown when the transport failure cannot carry reap evidence. Neither permits silent replay or claims successful execution.

## Prior evidence and remaining validation

Confirmed commits `951eb9a89ed4b1eb839fe3e370ec8a47866c3f51`, `0ef3b86e6fd6ce6dbceb2166e8b640054628603b`, `9862456ed25dfc63075b0c9123811a7cec67ed53` and `f1978b810f6caf3f42028fe92f028fec8530639a` are ancestors of current HEAD. Automated comparison with `REVIEW.md` found all five frozen config and all ten frozen provider fingerprints unchanged. Native harness source fingerprints are unchanged except the reviewed integration lock, expanded Python tests and fail-closed audit script. Earlier component test totals do not count as verification of the new integration code.

Pre-portability final-log follow-up: independently read the integrator's actual final logs under `/workspace/.onboarding/provider-runtime-final-*.log`. They show 242 top-level workspace Rust tests (config 67/core 76/providers 79/workspaces 18/daemon 2; the additional nested FIFO child is excluded), successful clippy/build and real daemon/CLI restart smoke, native Linux host compilation plus seven host tests, UI typecheck/build plus 41 tests, and completed infrastructure 28 validation. The fmt log is empty; this reviewer separately reran `cargo fmt --all -- --check` after the second freeze and observed exit 0. The bootstrap log was not yet present at this follow-up. Browser evidence records 22 passing checks and 8 screenshots against the actual built frontend/test-only localhost adapter/real Unix daemon. Independently verified all eight screenshot hashes and the tested daemon hash; this does not establish native GUI behavior. These are observed integrator logs, distinct from the independently executed commands above. Earlier pre-byte-pagination logs remain historical evidence.

Actual macOS ARM/Intel GUI/native confirmation/lifecycle and default release bundle audit are not run in this Linux review. The actual Linux WebKit GUI attempt remains blocked by its compiled absolute system-helper path; host unit tests and browser fixtures do not establish native GUI success. The final browser/real-daemon fixture result is observed as described above and remains separate from native WebView evidence. Real vendor versions, identities/model compatibility, native isolation/enforcement, manual installation, sleep/login, remote hosts and the rest of Design v0.2 remain separately unverified or unimplemented as recorded by the product ledger.

## Frozen integration fingerprints

Final freeze verified at **2026-10-26T20:22:54.716787+00:00**. All 35 integration source/test/lock/workflow fingerprints match the reviewed current bytes. At the second freeze only App.tsx and App.test.tsx changed from the first freeze; their history-read connectivity correction was inspected and all 41 UI tests plus typecheck rerun independently. The initial failed test fixture is preserved by the integrator in provider-runtime-ui-history-fixture-failed.log. The ignored `artifacts/provider-adapters-independent-review/source-final.json` records the earlier second freeze. The table here is updated for the third freeze: IPC and policy-fixture hashes changed and the newly changed registry-fixture file brings the reviewed set to 35. These table hashes identify the current reviewed integration independently of documentation changes.

| File | SHA-256 |
| --- | --- |
| `.github/workflows/product.yml` | `dae8b93309c42038015946557454fe45e1709ea3248cb766f5731c2decb56220` |
| `Cargo.lock` | `de7e34a31a2002f46b482cc597627ca08c152e297abc8e7549778aaa69d1cd43` |
| `apps/desktop/src-tauri/Cargo.lock` | `d07cfa2f95293f97be3f073266b50e7034ba68e903392f1a901aaf7594fff3fc` |
| `apps/desktop/src-tauri/tests/test_native_runner.py` | `e9968499ca78cbe286c78e370c7dcab041f37db53d770c22efe11db2e5b9868f` |
| `apps/desktop/src/App.test.tsx` | `93e12a2f1c7ec27ca9fa78033cb6e4010fda84db2281567a28117ef0a6f8424f` |
| `apps/desktop/src/App.tsx` | `c8dd45e3daa830d3daeb5788fb7ace0574ad0f757c2f8a8035f79174b21fcc01` |
| `apps/desktop/src/Connections.tsx` | `a73d7253e5feb3f3376ae299f6ea32b41c359292d76fdc36e374f6c0e30b3867` |
| `apps/desktop/src/ManagedSessions.tsx` | `ca0a6b4e31a58813aea9b783e879efa93edb8cfbb15de7af7a147de5f401e38a` |
| `apps/desktop/src/SessionWorkbench.test.tsx` | `b6d06ed1bfcdc133f159b65dde278f6d4d3418f5e68c3aa291f570ae22b5d41f` |
| `apps/desktop/src/api.ts` | `0db92cc42f718c0cfbc2653ad6499c9ff462d24f91e4593c0033eb354addc8c5` |
| `apps/desktop/src/event-window.test.ts` | `413ba3065526250d9fdd6df066f55c941ee81dd847553232da6076ee09af3be4` |
| `apps/desktop/src/event-window.ts` | `28a838dc7a12a86c3a941f19bd070aafac4b3f87385f1c3cb841c58598964775` |
| `apps/desktop/src/i18n.ts` | `02cb9744b0a775b9f2fcbfbdc9a7b7ef55573a2b3096164c432d78f94d34d318` |
| `apps/desktop/src/session-api.test.ts` | `9b4efb7161e2ded2950a1a1ca729cf36ae175f75f19b6386b1518f6ef3d08c22` |
| `apps/desktop/src/session-api.ts` | `db67e28fc775a96f7d0523cdf9078d2fda9eeb8fe9a8ef7d0a28d20929bdf7a8` |
| `apps/desktop/src/session-i18n.ts` | `d5098be760cf918af636c3f30f75cd671dd905293953451d012288132428ed80` |
| `apps/desktop/tests/quiet-canvas-real-ipc.py` | `7ef49dc01306c5f1cccae68d9b4babaad8c9cb3bdcdab9e366a60ee444eff746` |
| `crates/core/Cargo.toml` | `d7edfdfe4ee3efe2f718134ff73d63aa040599cc37fbaa4d94a6afa232a9a4bc` |
| `crates/core/src/execution.rs` | `45d2caf90dde5c7b94c825a89479fcc805389705d0a2a248b56737ee3ba8c402` |
| `crates/core/src/ipc.rs` | `ebae454204c1c712f7f595db621d5db716b3de26ad885a349015332f8a41ce89` |
| `crates/core/src/lib.rs` | `a8927906f94f51551da58a2848ef3ed3eea7e21020304ee41c11f2715543cd96` |
| `crates/core/src/policy.rs` | `95d8c1e150657d5419d332e43d83fe2a3d80507b9e865cff6f1a695a696a0f63` |
| `crates/core/src/routes.rs` | `259642508b6bba313f2545b122986d377b434af6f6caccfcf3208a8bb3c128a1` |
| `crates/core/src/runtime_config.rs` | `73e326b050e7fe661fe8a83e19775ea1d0d9da431e2909a1cb4e14c51d0ff3da` |
| `crates/core/src/sessions.rs` | `5814172cd77073008f52bbcae27362fe2042869cb19300b7da7d64e444f477cb` |
| `crates/core/src/storage.rs` | `9e6e395a74f2362a123ffe2b23e9c0fe2bfe6e2dc0b60484036ffb99d8712026` |
| `crates/core/tests/execution.rs` | `0aa99a756a42ea7aa278915a6adb880283461e42bf091f4cb8e8aa1241f6e037` |
| `crates/core/tests/migration_v3.rs` | `76174ca53d9302272e08034c368883c4244facf6e3bb0c5f260291217ac7fbe4` |
| `crates/core/tests/migration_v5.rs` | `3508d8fcf4a65a1ba5e66bb80ba079377eabce5b99ef4e12acbda5acbcc2974f` |
| `crates/core/tests/policy.rs` | `52eb621f116060ec2e7447d0375de14ebd81f7d6043beeef5e1bf0474fa8ed4f` |
| `crates/core/tests/sessions_ipc.rs` | `f73923772eb3c9ac69e3defc458780aa615e74369806633a9938b15c9feb50c9` |
| `crates/core/tests/storage.rs` | `502db5c069ab68f1e448bf46aea3b83c509f047ee731e15f06f4a8df06159250` |
| `requirements-browser.txt` | `89c9026218ec5d3805fd616896f87d21a7304ca5f62de32881fc8b4cb6b5d62b` |
| `scripts/native-audit-release.sh` | `dd60a45803ebc66c8dc5a19bba24da35b6e20716589b332314c7d1ecc4fa52d0` |
| `crates/core/tests/registry.rs` | `9355f135a16a3f4cb2a2523053f11bc3442d640101be7f5e2b04926891f60e19` |

## Observed final integrator log fingerprints

These bind the files actually read at follow-up; the reviewer independently reran only the commands in the first table.

| Log | SHA-256 |
| --- | --- |
| `/workspace/.onboarding/provider-runtime-final-rust.log` | `1c911a26a5db69b4c1d13557d5ec05a944a194154cb7f442673561b5f016a648` |
| `/workspace/.onboarding/provider-runtime-final-clippy.log` | `cb39dc7728452e2aa3e690d88cbd8f4565f0f57d04a32d56f8aea8ab1f215a4e` |
| `/workspace/.onboarding/provider-runtime-final-fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `/workspace/.onboarding/provider-runtime-final-build.log` | `9d342e45462d9d7a0fa3b3e3eca6b5216218d7dfa32438fea3270e83672087a0` |
| `/workspace/.onboarding/provider-runtime-final-smoke.log` | `3e4b9b3606e773bb0c39912df2ea993aac9bbbc73b7cfe097c06d2081e5dd805` |
| `/workspace/.onboarding/provider-runtime-final-native-check.log` | `53da3cab870faf3130a46a4918014d386144ac3eb18168d521589f9324a723bd` |
| `/workspace/.onboarding/provider-runtime-final-native-tests.log` | `c76416cf850fc605e3e57ba8927091d60feac227e2a88d505fc3a52e3c9edffe` |
| `/workspace/.onboarding/provider-runtime-final-ui-typecheck.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
| `/workspace/.onboarding/provider-runtime-final-ui-tests.log` | `a259985e3e35b5d928d2a0d1c8a8596801a61406c7a64a05e1194e2822e72339` |
| `/workspace/.onboarding/provider-runtime-final-ui-build.log` | `5534e08f7b20a95a4ff5582f0719c1949bb58103975a71967764be750b2e3018` |
| `/workspace/.onboarding/provider-runtime-final-browser.log` | `fbf7eb3ea9a94caf35468c39c0ef889e2dc17fbdbcfbcbcf63fc11b4e33a048f` |
| `/workspace/.onboarding/provider-runtime-final-infrastructure.log` | `53e40697666b825f1b43f205cdbc828fc95d8c33a3194960534b9206f3a4a692` |
| `/workspace/.onboarding/provider-runtime-final-browser/evidence.json` | `8085780b8d5db7ded9692c659748e46c1160ac0c4ff3744e5d0f2f08903834f6` |

## PR4 CI portability follow-up

A later actual CI result required a narrow correction after the second freeze. Reviewed only three additional root deltas: `socket_worker` explicitly clears inherited O_NONBLOCK before the existing bounded IO, a delayed-first-frame regression, and fresh owned managed test roots created with explicit 0700 permissions in policy/registry fixtures. Existing production root permission checks are unchanged; the negative 0755-root refusal and all frame/time limits remain. No existing user directory is chmodded.

Independently ran, under `umask 022`, `cargo test -p agentbaton-core --locked --test policy --test registry` (23 PASS) and `cargo test -p agentbaton-core --locked --lib inherited_nonblocking_stream_waits_for_the_delayed_complete_first_frame` (1 PASS). This verifies the changed fixture and first-frame mechanism on the current root candidate; it is additional targeted validation, not a second full-core count.

Read actual preserved controls: the uncorrected managed-root fixture under 022 failed all 13 policy tests with PERMISSION_DENIED; the isolated 636dbdd delayed nonblocking-stream control without the production reset failed with BrokenPipe (exit 101). The accepted current-root full-workspace022 log has 244 passing result-line tests including one nested FIFO child, hence **243 top-level tests** (config 67/core 77/providers 79/workspaces 18/daemon 2). This supersedes the prior 242 workspace count after adding the new IPC regression. The control comparison supports the socket mechanism; it does not prove the historical ARM CI failure cause without fresh macOS CI.

The minimal registry PR4 candidate eef1e95 was reviewed separately in its worktree and `docs/execution/project-registry/CI_REVIEW.md`; no native GUI/CI acceptance is inherited from this Linux check.

| Preserved follow-up log | SHA-256 |
| --- | --- |
| `/workspace/.onboarding/provider-runtime-umask022-control.log` | `ea199358f4e33dab4e04a47c7a200c7ab43a8a353f9bcbbeded648a4f821efc3` |
| `/workspace/.onboarding/provider-runtime-umask022-accepted.log` | `4aad9e5041730e530c50c8ad35a45538d073d4513d98e2cc0b46716ba709d493` |
| `/workspace/.onboarding/nonblocking-first-frame-control.log` | `0ae9644a96fe6c55020deba03a39507ff296c16128cae7b21af624abebae5154` |

## Final documentation verification

At 2026-10-26T20:40:17.959099+00:00, independently verified all 35 current source fingerprints still match and all 28 EVIDENCE log-hash references resolve to matching bytes. Counts agree: 243 top-level workspace tests (77+67+79+18+2), 41 UI tests, 7 Linux native-host tests, 10 runner/audit contracts and 8 pinned-schema messages. Requirement ledger preserves the exact ordered 130 unique baseline IDs; specs are unchanged and no requirement is marked fully accepted. ADR 0002 now correctly describes a stored/raw-JSON byte budget rather than a bound on decoded object memory.

The final browser rerun at `provider-runtime-portability-browser/evidence.json` actually uses the freshly built current daemon SHA-256 `032737aa5aaaa8492426ecdbf900a489f0d3a5dcfbb3d9ba3bbdcc1bef52664c`: 22 checks pass and all 8 screenshot fingerprints match. Current browser evidence is now separated from the preserved pre-portability browser result. This closes the daemon-provenance clarification while retaining the browser/native distinction.

STATUS/checkpoint record the integrator's guarded PR4 merge `dc5aea00013af0bf0fdae41d09c83ed0c4dacfd7` after its seven checks/artifact review, and continue to mark this root candidate's remote/macOS GUI checks and full product scope unfinished. This reviewer did not infer a native GUI pass from Linux or browser tests, nor independently rerun PR4 macOS CI. Root source remains frozen with no further source blocker identified.

Final EVIDENCE.json SHA-256 at this documentation check: `e76dc2436485ba1f874d51df36f4edb0e657c971eeabf0c6b604d2f7169de9a2`. ADR 0002 SHA-256: `9b4dfec2cef56191f9ab7088261de86e7256700efdda261bcbe737a45d9cac07`.

## Apple-target FIFO test compilation follow-up

At 2026-10-26T20:44:04.110523+00:00, reviewed the only source delta after committed `5eba068`: `crates/config/tests/runtime.rs` now creates its real private FIFO using `/usr/bin/mkfifo -m 600` and explicitly asserts FIFO file type. Original no-writer/bounded rejection assertions are unchanged; no production behavior or test skip changed. Independently reran all 22 runtime tests and workspace fmt: PASS. Current reviewed root source set is now 36 files (the prior 35 remain unchanged plus this test file). The frozen config-library source is unchanged, while its earlier test-file fingerprint is superseded by this fixture correction.

Actual first root CI ARM job 121835580552 failed with rustix mkfifoat E0425; failure is preserved in `/workspace/.onboarding/provider-runtime-ci-121835580552.log`. This correction is ready for exact-candidate CI retry, not evidence of macOS compilation success. `CI_FOLLOWUP.md` records scope, commands and failure provenance.

| New reviewed fixture file | SHA-256 |
| --- | --- |
| `crates/config/tests/runtime.rs` | `f9d3446916a0ec4961c077a2dc9cd85fe39d2eb9cbc7059cce3a90a1c09821ae` |
