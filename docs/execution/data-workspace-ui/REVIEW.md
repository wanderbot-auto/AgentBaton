# Independent Data Workspace UI/native review

Status: **UI/native leaf PASS on final source `a436be79c5a9099c0b40341db1984629bdb9ba71`; F1/F2 CLOSED. Independent UI325/typecheck/build and Native17/Clippy/fmt all PASS.** Initial source `df0178a55ae1424840c76c0b7009b842b9080788` (base `b71492f`) findings and failing controls below are retained as history. This is UI reconciliation/evidence validation, not a demonstrated Core permission bypass. Independently read latest `origin/main` `bcf10bb` OWNER-001/002 and ACK. No source snapshot or fixture instruction expanded authority.

## Scope and source review

Ten owned files introduce folded Projects DataWorkspaces, typed fixed commands, selection/DTO checks, bilingual copy warnings, namespaced styles, Projects ready refresh/base-input label, new tests/fixtures and native DataWorkspace approval/handlers. No App/Core/library/schema/lock/harness/capability change is in this UI commit. All 16 baseline UI test files (259 tests) are byte-identical; 43 new cases were added. Test fixtures are not imported by application entrypoints.

The UI sends only registered project ID, bounded relative file requirements and limits. It cannot send source/destination roots, approved/verified flags or copy authority. Paths reject traversal/absolute/backslash/case/ancestor aliases and bounds are safe integers. Preview only reads; copying requires the separate native approval button. Active/unknown/failed copies retain evidence and block new copies; cancel-requested does not claim stop/deletion. Explicit refresh/recovery uses cached same request/payload/key/revision, no automatic mutation. A synchronous in-flight guard and 32-intent bound apply; automatic observation stops after 20 readonly polls.

Projects mounts one lazy folded section and refreshes actual workspace list after a valid ready observation. Workspace base/input revision is labelled without pretending that a directory dataset hash is a Git HEAD. Graph/planner, model execution, Task acceptance or Git integration is not introduced here.

Native adds separate kind `data_workspace`, distinct from data-validation policy approval. Eight fixed handlers forward exact references/CAS; only native `data_workspace_approve` obtains a server proposal, checks requested preview ID/revision, shows native Ok/Cancel and privately finalizes the proposal's distinct ID/revision. Renderer does not receive a finalize/generic dialog/shell/filesystem capability. Full JSON-escaped snapshot and file hashes fit 16KiB or are refused before prompt/finalize; false sandbox/freeze/continuous-immutability/DLP guarantees are explicit. Valid Core errors pass through, malformed/pre-confirmation errors return structured not_started with zero confirmation/finalization, decline never finalizes, and finalize transport/post-finalize malformed responses retain uncertainty. Static wire shape was compared with the in-progress Core `data_workspaces.rs` and library DTOs, not treated as real schema10 execution.

## Findings and independent failing controls

F1 — stale terminal records can clear uncertain cancellation across remount. Observed revision floors live only in the component, while pending intents survive navigation. Refresh's resolved check uses matching record ID/state without comparing the pending cancellation revision. Reproduction: observed creation rev3 creating → cancel transport error/unknown → unmount → remount with same project/creation but older ready rev2. Current code clears pending, removes Recover and advertises ready. An independent temporary test fails because the retained disabled Recover button is missing. The legitimate older record shape is valid; the bug is loss of monotonic history/context. This must not unlock recovery or new actions from stale observations. Approval reconciliation likewise needs the exact pending project version, and validated mutation responses must raise floors before subsequent list reads.

F2 — ready observation inconsistencies pass the UI shape gate. `dataCreationShape` accepts state ready with a missing observation, observation state failed, wrong dataset revision/destination, or copied_files zero despite one selected file. Such a record can display ready and invoke `onReady`. Independent five-case API controls all returned true where false was required. The formal Core/library DataCreationView has actual creation/destination/dataset/selection/state/count/error fields; frontend should reject contradictory ready evidence without trying to recreate a private library witness or treating server JSON as execution authority.

Controls were appended only in reviewer's `/tmp` immutable export, never the author tests. Original complete suite was executed before these extra controls. F1 log: `/workspace/.onboarding/data-workspace-ui-independent/stale-remount-control.log`, one selected failure (22 unselected original component cases), exit1. F2 log: `ready-shape-control.log`, five selected failures (21 unselected API cases), exit1. Selection filters are targeted failure demonstrations, not skipped acceptance tests in the full suite. Both findings were reported promptly to primary and author; Both F1 and F2 precise repairs were authorized by primary; replacement source and independent closure checks are pending.

## Independently executed initial-source checks

Exact `git archive df0178a55ae1424840c76c0b7009b842b9080788 apps/desktop` exported to `/tmp/agentbaton-data-workspace-ui-independent-df0178a`. Only that reviewer copy borrows the existing locked `/workspace/AgentBaton/apps/desktop/node_modules` via an exact symlink. No author dependency path or installed dependencies were modified.

```sh
npm test
npm run typecheck
npm run build
npm test -- src/DataWorkspaces.test.tsx -t 'independent:'
npm test -- src/data-workspace-api.test.ts -t 'independent:'
```

Original immutable source: **302/302 tests in 18/18 files PASS**, start 2026-10-27 07:02:49 UTC, duration 13.73s; **typecheck/build PASS**. The added independent F1/F2 controls fail as recorded; they are not passing product tests. No timeout/config/old test was altered.

| Initial log | SHA-256 |
| --- | --- |
| `tests.log` | `34a0a9f996d35c1a4dd391d8c3222e41fbb3997cdd357689ecfcb459da2c09da` |
| `typecheck.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
| `build.log` | `88143921c7d037be7e03925285c32ca6c602708f26d955a9057cee56151f5e27` |
| `stale-remount-control.log` | `07c02a2d94b49b3886f57cf91e084fec947b9035e1a65e73510bb0bff1e94f97` |
| `ready-shape-control.log` | `6df42ddd7589658cd862e89b86b1e80c8e5f932bebc8d17ed776d34f2eed835a` |

## Initial phase: native execution and remaining limits

Native source review is complete, but this reviewer has not run Cargo/native17/Clippy/fmt: primary reserved the shared native cache for another author and requires explicit handoff. The UI author reports native17/Clippy/fmt passing; those are not independent results. This UI builds against inherited Core9 compile resources, not proof of pending schema10. No actual data preview/copy RPC, OS OwnerDialog, browser screenshot, native WebView/macOS architecture, model/account/paid resource was exercised here. Mock ready records do not prove real copies. Core private-witness/storage/cancel/restart/source-immutability/managed-root enforcement require separate implementation/integration evidence.

No product source/lock/staging/commit/push, author dependency path or recursive subagent was used by this reviewer. Only this report and ignored/temporary review artifacts were written. Final F1/F2 source checks and exclusive-window native execution remain pending.

## F1/F2 closure: final immutable source

Source `a436be79c5a9099c0b40341db1984629bdb9ba71`, docs `aa1c2814a30f43405f3c545ec178cf665f96e673`, was independently reviewed against `df0178a`. Only the component/API guards, their new tests/fixture and explanatory bilingual pending receipt text changed. No Rust, Projects mount, style, Core, dependency, lock, shipping permission or old baseline test changed in this repair.

F1 closure: each cached pending request retains bounded per-ID record/project revision floors, restores them across remount, merges verified mutation response floors before refresh, and holds a receipt until an equally fresh or newer matching read observes it. Reads must not omit an observed record or regress a revision/project version. Pending approve/cancel reconciliation requires its exact historical project version and requested record. Old asynchronous callbacks cannot replace or recreate an intent that has already been cleared or replaced under a different key. Acknowledged writes awaiting a trustworthy read do not resend automatically; readonly refresh is available. The existing 32-intent and 20-read limits remain. Valid newer terminal receipts, including valid historical ready records, still reconcile without asserting cancellation success or expanding authority.

F2 closure: the official public observation now includes destination, dataset revision and selection digest. Its creation reference/bounds/guarantees must match the record. Ready additionally requires observation state ready, exact full copied-file count, and null record/observation errors; absent or conflicting observations are rejected before `onReady`. Readable copy-contract snapshot/file inputs are cross-checked with the record, and available preview/project-version/selection/dataset must agree. These are defensive consistency checks, not JavaScript reconstruction of the host's private witness, contract digest or execution permission. Historical ready reads remain allowed without turning the historical source/project version into a current grant.

The original independent F1 case and all five independent F2 cases were copied with the same semantic assertions into the final suite and passed in the independent complete run. New positive and negative cases also cover later historical ready, mutation-ack-before-stale-list/remount, wrong project-version approval recovery, corrupt ready refusing `onReady`, conflicting available preview, and the legitimate sixteen-component source versus seventeen-component `inputs/` destination distinction.

All **259 original tests in 16 files are byte-identical** to `b71492f`. All original 43 DataWorkspace assertions remain. The one changed original new-test fixture now assigns both its historical preview and corresponding creation `project_revision=2`; previously it contradictory assigned 2 and3. That fixture correction preserves every assertion (archived/stale approval disabled, existing cancellation enabled). Author retained its intermediate fixture mismatch failure. No timeout, config or acceptance assertion was weakened. The native `approval.rs` and `lib.rs` are byte-identical to the initially statically reviewed source.

## Final independent frontend execution

Exact frontend was exported with `git archive a436be79c5a9099c0b40341db1984629bdb9ba71 apps/desktop` to `/tmp/agentbaton-data-workspace-ui-independent-a436be7` (an ordinary temporary archive, **not another Git worktree**). Only the export borrowed the existing installed locked dependencies by an exact symlink. Actual commands in its `apps/desktop`:

```sh
npm test
npm run typecheck
npm run build
```

Results: **325/325 tests in 18/18 files PASS**, start 2026-10-27 07:32:10 UTC, duration 14.55s; **typecheck PASS; production build PASS**. These include the original 302 cases and 23 additional regression/consistency cases. The earlier independent 1+5 failing controls and original full302 pass remain separately recorded; they were not overwritten or relabelled.

| Final independent evidence | SHA-256 |
| --- | --- |
| `final-tests.log` | `c46378798878cf503de0cbbcf117e63ac73f59cd26b4a8308e90342309c5c306` |
| `final-typecheck.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
| `final-build.log` | `4a6dd840ebd8a26b50bee7003bd150c26d6c47e5621775d6bf06a0cc6bf6e60d` |
| `final-ui-verification.json` | `8b3019301bb72f6d65469392f7f4e5ffd6baa7ae55fec9b83afffa17be27073c` |

Final source fingerprints:

| File | SHA-256 |
| --- | --- |
| `apps/desktop/src-tauri/src/approval.rs` | `32309c297b893778c8ac5af3c66f5f7e07aeed228349be3ff3955db0bfd0df65` |
| `apps/desktop/src-tauri/src/lib.rs` | `57db079ba3f96b81ba28616fb83f1496224cab37d506989cc3aae4240b328da4` |
| `apps/desktop/src/DataWorkspaces.test.tsx` | `690dafc4cc42f7ad38b00a60158f3d59f5cbed80a277468fdcb72638531f833f` |
| `apps/desktop/src/DataWorkspaces.tsx` | `334ae7c8a7c08a16c7c771057953140d0d7b99683759dc0a0356c6af2057de5a` |
| `apps/desktop/src/Projects.tsx` | `8a94b6c5ec766f3acddb22648c89b74595fb0fa7b5c3439e9b923a0c925d18f4` |
| `apps/desktop/src/data-workspace-api.test.ts` | `fa5b9e0575c6197130b7d3c88a306d9295950a915d99e2175cf244284ca1ad9e` |
| `apps/desktop/src/data-workspace-api.ts` | `0323365017f4cedbebc5350931b8ea5b308eff416776c0f532c5490d4d7424d6` |
| `apps/desktop/src/data-workspace-i18n.ts` | `3c52daaf946ef5138a845b2e5f2eea957fa675ddb7fd177b949c17423a2240d1` |
| `apps/desktop/src/data-workspace-test-fixtures.ts` | `b4cbc62a7a9f7a6de3a541c3e7310e1f51cb5f3311afa35cd284e4866587b1bf` |
| `apps/desktop/src/style.css` | `425ecf4f1d9849bd873ea7e9daa7c2efc169a76179bc4cbdf41828a9dc819d2e` |

Independent built frontend artifacts:

| File | SHA-256 |
| --- | --- |
| `index.html` | `71318408d4586c00df756b7f4da5eaf39cd90f8a1961b978bef302965a9bef18` |
| `assets/index-D6UB0Jo6.css` | `1208bc54eb8aeea54bca803a4d78dc7435688106f6b6eb0224dfbf1b5b390595` |
| `assets/index-Cj0Bu_O8.js` | `700b2d52f14affd5a82f5884b5644fef8a9639bef28ec7e632be8f198ee8f001` |

Production JS does not contain the new fixture markers (`data-project-`, `/registered/source`, `/host-managed/data-`). Both reviewer-created temporary dependency symlinks were verified to point exactly to the existing main-tree installed dependencies and removed after their respective checks; no installed dependency directory or author path was changed.

### Historical UI-only checkpoint: native pending and final scope

Primary explicitly assigned the newly released native cache to another independent Goal reviewer first; **this reviewer did not run Cargo/native17/Clippy/fmt** and will not contend with that writer. Native hashes above bind the completed static review to unchanged code. The author's earlier 17-unit/lint/fmt results remain reported evidence, not an independent execution. The final frontend result has no remaining blocking finding in this scope, while independent Native17 execution remains pending primary's next exclusive window.

Mock/native-closure checks do not establish schema10 Core RPC, actual file copy/cancel/restart, private host witness, real OS owner consent, native WebView, macOS, Provider/model, Task acceptance or full product completion. Those dimensional limits remain unchanged. Only this review report and ignored/temporary evidence were written; no source/lock/index, commit/push/merge or subagent operation was performed.

## Independent Native17 completion and target release

The previous UI-only review checkpoint had SHA-256 `782817710d68507e2474f8c03e37b61960e6d59bf73fb14dbf4a47ffda50fccd` and correctly marked native execution pending. After the preceding independent Goal reviewer released the cache, primary explicitly granted this reviewer exclusive use of `/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target`. The following commands have now completed successfully; the historical pending statements above refer to that earlier phase.

Exact checked-out documentation head was `aa1c2814a30f43405f3c545ec178cf665f96e673`, product source `a436be79c5a9099c0b40341db1984629bdb9ba71`. Native source remained exactly:

- `approval.rs`: `32309c297b893778c8ac5af3c66f5f7e07aeed228349be3ff3955db0bfd0df65`.
- `lib.rs`: `57db079ba3f96b81ba28616fb83f1496224cab37d506989cc3aae4240b328da4`.

To avoid reusing a different worktree's Core/Workspaces crate, only this worktree's `crates/core/src/lib.rs` and `crates/workspaces/src/lib.rs` modification times were refreshed before running. SHA-256 values and bytes were verified unchanged both before and after, including exact comparison to the frozen Git source: Core `2d732f787e7e8e416c8473323be922aa1e1fe5fbe70dae8ce5733a5fb9e1ad43`, Workspaces `2065213a1eb32be1e2a953adb0c5f0435d55987b4f8d0b40885797f5d11eb6e6`. The actual build logs name this worktree's crate paths. Main, in-progress DataCore and other execution targets were not touched.

Non-sensitive execution selectors, preserving inherited library search path suffixes:

```sh
export CARGO_HOME=/workspace/.toolchains/cargo RUSTUP_HOME=/workspace/.toolchains/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR=/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2
export PKG_CONFIG_SYSROOT_DIR=/workspace/.toolchains/linux-ui/sysroot
export PKG_CONFIG_LIBDIR="$PKG_CONFIG_SYSROOT_DIR/usr/lib/x86_64-linux-gnu/pkgconfig:$PKG_CONFIG_SYSROOT_DIR/usr/share/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig"
export LD_LIBRARY_PATH="$PKG_CONFIG_SYSROOT_DIR/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export LIBRARY_PATH="$PKG_CONFIG_SYSROOT_DIR/usr/lib/x86_64-linux-gnu${LIBRARY_PATH:+:$LIBRARY_PATH}"
cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --lib
cargo clippy --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --lib -- -D warnings
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
```

Working directory `/workspace/.worktrees/data-workspace-ui`. Commands ran sequentially using existing toolchains/SDK/cache; no dependency installation, new target, prepare-sidecars, source change or new backend binary was introduced.

Actual results:

| Command | Result | Elapsed |
| --- | --- | --- |
| Native `cargo test --lib` | **17 passed, 0 failed, 0 ignored, 0 filtered** | 61.588s including crate compilation; test execution 0.02s |
| Native `cargo clippy --lib -- -D warnings` | **PASS, exit0** | 36.423s |
| Native `cargo fmt -- --check` | **PASS, exit0** | 0.081s |

All original host/Goal13 tests and the four DataWorkspace additions ran. The latter test separate `data_workspace` kind and exact preview/CAS versus proposal/finalize revision, full native approval text/frame, malformed/wrong-kind/missing/oversize refusals with zero confirmation/finalization, explicit decline, and preserved post-finalize transport/malformed uncertainty. Permission tests also pass. Injected confirmation closures are test controls; no real OS owner dialog was displayed.

Native evidence directory `/workspace/.onboarding/data-workspace-ui-independent/native/`:

| Artifact | SHA-256 |
| --- | --- |
| `tests.log` | `7432cc8787152a3b7e21b1428c73bb03762d50af49e98aba6e8e5b9e0edb5830` |
| `clippy.log` | `bbe02fbcbd286b10e5674a88d6d3609d1c6dcb134b3172cd86566c33100a3d01` |
| `fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `inputs.json` | `d941d626011f6bed99f3d8d4e8a9b17a780c1f60891d0580cf6040dca42e7d96` |
| `results.json` | `ab2e55117e97509fc89fbe1091d3521774becea7a61a69a1af7693b890556355` |

Existing compile resources were independently hash-checked and left unchanged: `baton-x86_64-unknown-linux-gnu` `ab5962a6fa6cc981026572f9b0950db8743fc4cc69a492d7202b14413824d979`; `batond-x86_64-unknown-linux-gnu` `2b4a302c4e992df9c01cde404c03ba526cf411eb24ecbc7aadc4e85fb8be771e`. These are the existing **Goal/schema9 Linux debug** resources, not new Data/schema10 runtime evidence, native installation packages or macOS builds.

Immediately after all three commands exited0, this reviewer explicitly **RELEASED** the shared native target to primary and ceased Cargo/rmeta/target access. Subsequent activity only updates this report and reads source/logs outside that target. No source byte, lock or index was changed. F1/F2 closure and all original failure evidence remain intact.

No remaining blocking finding in this UI/native leaf scope. Real schema10 IPC/data-copy/cancel/restart/private-host-witness validation, native OwnerWindow/WebView, Apple Silicon/Intel macOS, Provider/model, Task acceptance and whole-product delivery remain separate evidence dimensions.
