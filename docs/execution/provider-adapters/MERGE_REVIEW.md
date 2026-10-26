# Independent root session/Quiet Canvas merge review

Reviewer: native `/root/integration_review`, independent execution context. Reviewed the intentional merge in progress from root `a5c1666` (including FIFO correction `ec77dfc` and dedicated Quiet Canvas review `4871ae4`) with session `b39ae1e`, which already includes protected main `dc5aea0`. Source snapshot verified at 2026-10-26T20:50:45.495808+00:00. No unresolved Git index conflicts remain. Only this review document was written by the reviewer; no sources, locks, staging, commits, pushes or merge operations were changed.

Verdict: no unresolved source blocker after the incoming cancellation test was adapted to Quiet Canvas navigation and all 48 UI tests passed. This review supports the integrator's conflict merge/checkpoint; fresh exact-commit CI and native macOS remain separately required.

## Preserved contracts and conflict resolution

- Compared with a5c1666, the advanced schema5 storage/migration, explicit route matching, runtime-plan/default-profile verification, immutable attempt binding, session supervisor, native driver/config materializer, Provider selector and API, and both Cargo locks are byte-identical. In particular, incoming schema4's custom-ACP default does not replace the root's exact native protocol-route binding. No fallback or authority expansion was introduced.
- The full existing IPC actor remains. Its worker now calls the reviewed shared accepted-stream helper, retaining blocking reset and three-second deadlines while failing only the client worker on configuration errors. Both worker and authenticated delayed-first-frame regressions remain alongside the oversized parser test. The original task-create integration test gains a failure diagnostic assertion, not a weaker success check.
- The newer Quiet Canvas/focused task layout, prior 41 tests and full forward/earlier history cursors remain. The incoming display helper adds strict inbound 8MiB/500-record validation, JSON delimiters and replacement reasons. Older pages replace the visible window when either bound would be exceeded, with honest count/byte messages and the existing returned cursor for newer records. Refresh and next-page reads now use the same inbound guard. Persistent history is unchanged.
- Both original 2 and incoming 4 helper tests are retained. All three incoming App behavior tests remain, including exact cancellation-enabled behavior on history RESOURCE_LIMIT. The final test initially assumed the old layout; it now opens the actual Details tab before the unchanged cancellation-enabled assertion. ManagedSessions preserves the root's actual partial-cursor indication and adds a byte-limited-page message; its required messages runtime import is present.
- Incoming small-pipe fixture modes/tests are additive: original 4MiB/500ms/version output/reaping/capacity assertions remain. They exercise the current advanced Provider transport without replacing its implementation.
- Foundation evidence is a deep union: every root 7 and incoming 9 failed-attempt entry is preserved within the current 9; every root 1 and incoming 3 remote-CI-history entry is preserved within the current 4. Root successful 222a artifact/integration fields remain, alongside incoming SQLite/stderr follow-ups. Old failures are not relabeled as new native success.

## Independently executed checks

Rust commands used configured CARGO_HOME/RUSTUP_HOME/PATH and CARGO_BUILD_JOBS=2, under `umask 022`.

| Command | Actual result |
| --- | --- |
| `npm run typecheck && npm test -- --reporter=dot` in apps/desktop, before test navigation adaptation | Typecheck PASS; UI 47 PASS/1 FAIL, missing Details navigation in the incoming cancellation test. |
| Same command after the literal Details click was added | PASS: 48 UI tests /7 files, preserving all behavior assertions. |
| `cargo test -p agentbaton-core --locked --lib frame_tests` | PASS: 3. |
| `cargo test -p agentbaton-core --locked --test storage` | PASS: 8, including pre-decode oversized refusal and ordered byte pagination. |
| `cargo test -p agentbaton-providers --locked small_` | PASS: 3, including two incoming small-pipe fixtures and the existing native small-pipe contract. Other tests filtered. |
| `cargo test -p agentbaton-core --locked --test ipc` | PASS: 5. |
| `python3 scripts/verify_bootstrap_manifest.py` | PASS: 92 files. |
| `git diff a5c1666 --check` | PASS. |

Independent targeted Rust total: **19**. Read the integrator's actual full Rust log: 247 passing result-line tests include one nested FIFO child, hence **246 top-level tests** (core 78/config 67/providers 81/workspaces 18/daemon 2). The two added Provider regressions and added authenticated-frame regression explain the increase from 243. Full clippy/build/restart/design/manifest logs show success; fmt log is empty with the integrator's separate exit evidence. These full checks were observed, not independently rerun in full here.

The initial missing-messages-import typecheck error and the 47/1 incoming-test failure remain in the integrator's preserved logs; the accepted 48 output is separate. The fresh merged browser/real-daemon evidence was subsequently observed: 22 checks pass, 8 screenshot hashes match and its recorded daemon hash matches the current built daemon, as detailed below. Earlier browser results remain separate. Linux tests/generated children/mocked UI do not establish macOS GUI/native confirmation/lifecycle or live vendor/account/model compatibility. No private identity, production action or extra paid service was used; full Design v0.2 remains incomplete.

## Frozen reviewed source fingerprints

This 39-file set is the union of the previous 36-file root integration/fixture snapshot and three newly changed IPC/provider test files. The policy test-file fingerprint was refreshed after the last Apple-target FIFO fixture correction. It supersedes prior merge-sensitive fingerprints for this worktree snapshot; it does not claim the old component/CI results automatically cover changed code.

| File | SHA-256 |
| --- | --- |
| `.github/workflows/product.yml` | `dae8b93309c42038015946557454fe45e1709ea3248cb766f5731c2decb56220` |
| `Cargo.lock` | `de7e34a31a2002f46b482cc597627ca08c152e297abc8e7549778aaa69d1cd43` |
| `apps/desktop/src-tauri/Cargo.lock` | `d07cfa2f95293f97be3f073266b50e7034ba68e903392f1a901aaf7594fff3fc` |
| `apps/desktop/src-tauri/tests/test_native_runner.py` | `e9968499ca78cbe286c78e370c7dcab041f37db53d770c22efe11db2e5b9868f` |
| `apps/desktop/src/App.test.tsx` | `1e957d2689f305195d9a1edc7af7a5d9d8a33ab31a8db9e2f0ef20ff45bde376` |
| `apps/desktop/src/App.tsx` | `18b829334b9e451f79aa35beb1151cdaf4abeb14628df709e62b70dd4ecf47d9` |
| `apps/desktop/src/Connections.tsx` | `a73d7253e5feb3f3376ae299f6ea32b41c359292d76fdc36e374f6c0e30b3867` |
| `apps/desktop/src/ManagedSessions.tsx` | `5dcd061742a80316e8ddfdbc99f4d9c68aaa4c44978a6c9404f00ae79c0f4e59` |
| `apps/desktop/src/SessionWorkbench.test.tsx` | `b6d06ed1bfcdc133f159b65dde278f6d4d3418f5e68c3aa291f570ae22b5d41f` |
| `apps/desktop/src/api.ts` | `0db92cc42f718c0cfbc2653ad6499c9ff462d24f91e4593c0033eb354addc8c5` |
| `apps/desktop/src/event-window.test.ts` | `b66baa960f3e5c3354328e0ac194c744c6a99cb49d573fdee10586a8256b1e89` |
| `apps/desktop/src/event-window.ts` | `73074b1d53281d6f738614f292b3f8a7cbfd95f3694c3ed3491cda9d637ad7a2` |
| `apps/desktop/src/i18n.ts` | `d5f8fc94d8343f8934df3f4206965fe000cfa3a845b366c3c79aca289a8e995f` |
| `apps/desktop/src/session-api.test.ts` | `9b4efb7161e2ded2950a1a1ca729cf36ae175f75f19b6386b1518f6ef3d08c22` |
| `apps/desktop/src/session-api.ts` | `db67e28fc775a96f7d0523cdf9078d2fda9eeb8fe9a8ef7d0a28d20929bdf7a8` |
| `apps/desktop/src/session-i18n.ts` | `d5098be760cf918af636c3f30f75cd671dd905293953451d012288132428ed80` |
| `apps/desktop/tests/quiet-canvas-real-ipc.py` | `7ef49dc01306c5f1cccae68d9b4babaad8c9cb3bdcdab9e366a60ee444eff746` |
| `crates/config/tests/runtime.rs` | `f9d3446916a0ec4961c077a2dc9cd85fe39d2eb9cbc7059cce3a90a1c09821ae` |
| `crates/core/Cargo.toml` | `d7edfdfe4ee3efe2f718134ff73d63aa040599cc37fbaa4d94a6afa232a9a4bc` |
| `crates/core/src/execution.rs` | `45d2caf90dde5c7b94c825a89479fcc805389705d0a2a248b56737ee3ba8c402` |
| `crates/core/src/ipc.rs` | `86bff67f5690d83552178133980b2fc434b792decccdf4ffa357d64ed494ecc7` |
| `crates/core/src/lib.rs` | `a8927906f94f51551da58a2848ef3ed3eea7e21020304ee41c11f2715543cd96` |
| `crates/core/src/policy.rs` | `33b41dff1df5af1ad3de90299a41de9d82f179cd9ee81f4c06dab910d2fa22a0` |
| `crates/core/src/routes.rs` | `259642508b6bba313f2545b122986d377b434af6f6caccfcf3208a8bb3c128a1` |
| `crates/core/src/runtime_config.rs` | `73e326b050e7fe661fe8a83e19775ea1d0d9da431e2909a1cb4e14c51d0ff3da` |
| `crates/core/src/sessions.rs` | `5814172cd77073008f52bbcae27362fe2042869cb19300b7da7d64e444f477cb` |
| `crates/core/src/storage.rs` | `9e6e395a74f2362a123ffe2b23e9c0fe2bfe6e2dc0b60484036ffb99d8712026` |
| `crates/core/tests/execution.rs` | `0aa99a756a42ea7aa278915a6adb880283461e42bf091f4cb8e8aa1241f6e037` |
| `crates/core/tests/ipc.rs` | `26c7540c6ec88df74365268aaa81de4feb4be7e57418e7fc221a1559d4023e28` |
| `crates/core/tests/migration_v3.rs` | `76174ca53d9302272e08034c368883c4244facf6e3bb0c5f260291217ac7fbe4` |
| `crates/core/tests/migration_v5.rs` | `3508d8fcf4a65a1ba5e66bb80ba079377eabce5b99ef4e12acbda5acbcc2974f` |
| `crates/core/tests/policy.rs` | `52eb621f116060ec2e7447d0375de14ebd81f7d6043beeef5e1bf0474fa8ed4f` |
| `crates/core/tests/registry.rs` | `9355f135a16a3f4cb2a2523053f11bc3442d640101be7f5e2b04926891f60e19` |
| `crates/core/tests/sessions_ipc.rs` | `f73923772eb3c9ac69e3defc458780aa615e74369806633a9938b15c9feb50c9` |
| `crates/core/tests/storage.rs` | `502db5c069ab68f1e448bf46aea3b83c509f047ee731e15f06f4a8df06159250` |
| `crates/providers/tests/installation.rs` | `90724f1d9d6c3fe5641ae7076d9c9442293a6550a4b20bb7baecbfcd6e6b1531` |
| `crates/providers/tests/stdio.rs` | `1be8c06e63a6107e94fc903bcdbcbddf662f77d4d96cf475f3eddb6485972dfd` |
| `requirements-browser.txt` | `89c9026218ec5d3805fd616896f87d21a7304ca5f62de32881fc8b4cb6b5d62b` |
| `scripts/native-audit-release.sh` | `dd60a45803ebc66c8dc5a19bba24da35b6e20716589b332314c7d1ecc4fa52d0` |

## Preserved integration log fingerprints observed

| Log | SHA-256 |
| --- | --- |
| `/workspace/.onboarding/provider-runtime-merged-rust.log` | `ac6acb3dbf8d531127988246286c0cab0481c184130749bcd11be7a11fe92c9f` |
| `/workspace/.onboarding/provider-runtime-merged-clippy.log` | `2ab3d0b789425d225256174ff2a7c64ace59f1b80248af49c8980929703cdaef` |
| `/workspace/.onboarding/provider-runtime-merged-fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `/workspace/.onboarding/provider-runtime-merged-build.log` | `997c7cd3cb6dcf530a350806927739a3576df742371ee22d89980ebc7af1cae7` |
| `/workspace/.onboarding/provider-runtime-merged-smoke.log` | `3e4b9b3606e773bb0c39912df2ea993aac9bbbc73b7cfe097c06d2081e5dd805` |
| `/workspace/.onboarding/provider-runtime-merged-ui-typecheck.log` | `0e75f33d0b98fce3b5d54870fcb8342745e150a7f1fb43bb3b4ae76a705f4d36` |
| `/workspace/.onboarding/provider-runtime-merged-ui-tests.log` | `52ad2d9f1f084ec06ce54cdb734be13cac805f390b595b246436cd76c15e6a20` |
| `/workspace/.onboarding/provider-runtime-merged-ui-typecheck-accepted.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
| `/workspace/.onboarding/provider-runtime-merged-ui-tests-accepted.log` | `a4b0c234c4889abf8257309ce7f76f7e0229d76782acab8ddf537190f6c35c3b` |
| `/workspace/.onboarding/provider-runtime-merged-design.log` | `a7f57df3133b08ba0d97172c9214e632e8801b83fdb94ab4679bcda978c86c3d` |
| `/workspace/.onboarding/provider-runtime-merged-bootstrap.log` | `116d97a6e9f493a093a6b3c5b04db551781ddf57f0dcc59b4595ebd95603096f` |

## Final test-fixture portability and browser follow-up

At 2026-10-26T20:52:04.757354+00:00, reviewed one additional test-only change in `crates/core/src/policy.rs`: actual Session macOS CI job 121838837098 exposed Apple-excluded rustix mkfifoat in the nested live-writer FIFO regression. Creation now uses fixed `/usr/bin/mkfifo -m 600` with literal path argv and an actual FIFO-type assertion. Both gitdir/commondir cases and their unchanged rejection/bounded-child assertions remain; production policy bytes before the test module are exactly equal to a5c1666. No skip or deadline increase was introduced.

Independently reran `cargo test -p agentbaton-core --locked --lib` under 022: 11 top-level tests pass; the nested child result is excluded. This includes the three already executed frame tests, so it is not added wholesale to the earlier 19 targeted unique count. The observed full 246 Rust/48 UI and native-host/browser runs preceded this final fixture-only correction; the new 11-unit run verifies that final delta on Linux, and exact-candidate Apple CI remains necessary.

Read `/workspace/.onboarding/provider-runtime-merged-browser/evidence.json`: 22 checks pass and all 8 screenshots match their recorded hashes. Tested/current daemon SHA-256 is `2e44f2b22f0d6aa336e6d1999c91ad5115bcb45d662f22ae44f22354f094101f`. This is actual built frontend/test-only localhost adapter/real Unix daemon evidence; it remains distinct from native macOS GUI and live Provider compatibility.

All 39 source fingerprints in the table were reverified after the last fixture change. No unresolved source blocker remains for the integrator's merge/checkpoint and fresh required CI.

| Follow-up evidence | SHA-256 |
| --- | --- |
| `/workspace/.onboarding/session-ci-121838837098.log` | `3dfe65fb33be4cdac79523d0b9a5d64d2559a50917fbe7566be5cac33c6f781a` |
| `/workspace/.onboarding/provider-runtime-merged-browser/evidence.json` | `30e574ff70bb10555d2ecc90167597de7948819c651da7ed0cb7bd8d6ad500a9` |
