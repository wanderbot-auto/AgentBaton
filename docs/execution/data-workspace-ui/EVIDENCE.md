# Selected-file data workspace UI/native evidence

## Inputs and exact source

Source `df0178a55ae1424840c76c0b7009b842b9080788`, branch `codex/data-workspace-ui`, base `b71492f`. ACK OWNER-001/002. Formal schema10 DTO/wire coordinated with Core author in `/workspace/.worktrees/data-workspace-core/crates/core/src/data_workspaces.rs` and the reviewed library's `data_workspace.rs`. Core implementation was not merged or executed here.

| Owned file | SHA-256 |
| --- | --- |
| `DataWorkspaces.tsx` | `3ea49b11da988ffc7345e4b01e8de1bc1f694b53c8073228ab6129ed0af8d313` |
| `DataWorkspaces.test.tsx` | `09d654d07200049db7bddb9dfd7cb85f8c22529b35ea49e736afb193cdac86af` |
| `data-workspace-api.ts` | `08ed11f99f86c75b55ad9a78162f1802f966e4fca78019a062fa930623c2f5e8` |
| `data-workspace-api.test.ts` | `f613a3714866e54eef2a88b3902e26dd1ad9ff17e224718792a9d71a01d2e3b1` |
| `data-workspace-i18n.ts` | `988f283d1ac84c0b2e6d75db9d653e6f06b037d74398d8dbf4ab10442e06c22c` |
| test-only `data-workspace-test-fixtures.ts` | `f6b1af1603c6941db51a15371b7f5b166a2724d12f0ff43985647ecb470ccdff` |
| `Projects.tsx` | `8a94b6c5ec766f3acddb22648c89b74595fb0fa7b5c3439e9b923a0c925d18f4` |
| `style.css` | `425ecf4f1d9849bd873ea7e9daa7c2efc169a76179bc4cbdf41828a9dc819d2e` |
| native `lib.rs` | `57db079ba3f96b81ba28616fb83f1496224cab37d506989cc3aae4240b328da4` |
| native `approval.rs` | `32309c297b893778c8ac5af3c66f5f7e07aeed228349be3ff3955db0bfd0df65` |

## Frontend actual execution

Working directory `/workspace/.worktrees/data-workspace-ui/apps/desktop`, existing locked dependencies borrowed by a worker-created exact symlink to `/workspace/AgentBaton/apps/desktop/node_modules`, created only while absent and removed after checks. No package installation or lock edit.

```sh
npm run typecheck
npm test -- src/data-workspace-api.test.ts src/DataWorkspaces.test.tsx
npm test
npm run build
```

Final full run 2026-10-27 06:55:58 UTC: **302/302 tests in18 files PASS**, 13.08s; types/build **PASS**. Original259 UI tests/old App/harnesses are byte-identical to base. New43 cases cover explicit selection/caps/hash/aliases, actual DTO canonical sorting and inputs prefix, false scope/ready enforcement, exact native command/CAS/key framing, lazy mounting, no copy on preview, separate approval, decline, ready-to-workspaces refresh, unknown remount recovery, failure freshness, previous uncertainty surviving not_started recovery errors, stale project callbacks/record revisions/missing history, duplicate click guard, cancel races/terminal failure reconciliation, retained partial/unknown records, text-only rendering and readonly20-poll limit.

Logs under `/workspace/.onboarding/data-workspace-ui/`:

| Log | SHA-256 |
| --- | --- |
| `typecheck-final.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
| `ui-final.log` | `f6299f4ac326a154b8e070d0d333b1996bd9b31e8ce3dfa39c48cfef205ff072` |
| `build-final.log` | `0999ab8064f242ad6c7ebcf1237d17ef81a88b359b7cd678577cf39be25c3d96` |

Earlier targeted37/39-case and full298 passing logs remain separately. The mock records, copied hashes and ready states in unit tests are **mock verification**, not a real filesystem copy or daemon proof.

## Preserved new polling failures

`ui-target-expanded.log` SHA `02e708cbfc73ca3c5e2a60544fede268cce4bf4c8d30761fa8c1ddd68ce14f90`: actual **2 failed /39 passed**, exit1. New fake-clock user-event disclosure setup timed out at unchanged5000ms; its following duplicate-click fixture also timed out in that failed run. Corrected the new timer fixture to explicit disclosure/toggle under React act, rather than changing product native disclosure behavior or raising timeouts.

`ui-polling-corrected.log` SHA `87033028c2286cb709a74e40d7e1e7636e857a3a19622a3f7efef98cac6bea11`: actual **1 failed /19 passed**, exit1, expected21 list observations received2. Stable unchanged response array references did not re-arm the effect. Added explicit completion/readVersion tracking; polling now continues independently of data identity and stops at the original20-read ceiling. No acceptance assertion was removed.

`ui-target-poll-fix.log` SHA `56cc81b2c018dd5ad3de434dc8f1afbacaaaac85275d6b1591aec48cdc6f514a`: subsequent **41/41 PASS**, followed by final43 new/302 full cases after the extra cancellation/archive tests. No old tests skipped, deleted, assertion weakened or timeout/config altered.

## Native leaf execution

Primary granted an exclusive window for the existing shared native target, then received explicit release after commands completed. Actual non-sensitive environment selectors:

```sh
export CARGO_HOME=/workspace/.toolchains/cargo RUSTUP_HOME=/workspace/.toolchains/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR=/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
export PKG_CONFIG_SYSROOT_DIR=/workspace/.toolchains/linux-ui/sysroot
export PKG_CONFIG_LIBDIR="$PKG_CONFIG_SYSROOT_DIR/usr/lib/x86_64-linux-gnu/pkgconfig:$PKG_CONFIG_SYSROOT_DIR/usr/share/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig"
export LD_LIBRARY_PATH="$PKG_CONFIG_SYSROOT_DIR/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export LIBRARY_PATH="$PKG_CONFIG_SYSROOT_DIR/usr/lib/x86_64-linux-gnu${LIBRARY_PATH:+:$LIBRARY_PATH}"
cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --lib
cargo clippy --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --lib -- -D warnings
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
```

**17/17 native unit tests PASS**, Clippy/fmt **PASS**. All original13 remain;4 new tests cover separate DataWorkspace kind, exact preview CAS versus distinct proposal revision, complete native review text/frame, malformed/wrong-kind/oversize before-prompt/refusal, decline and preserved post-finalize transport/error uncertainty. Confirmation/finalization closures are injected test controls; **no actual OwnerDialog was shown**. Renderer capability JSON is unchanged; existing no-generic-dialog/filesystem/shell permission test passes.

| Native log | SHA-256 |
| --- | --- |
| `native-test-initial.log` | `a942c5c212681d98bb5171ac5688c27e612367a654e0084ee22d904572a5637b` |
| `native-clippy-initial.log` | `342c7f33de3cad43d7760342aac4da678132c1d5eb2b620e2a3c6dff21912bdc` |
| `native-fmt-initial.log` (empty successful check output) | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

No new target, main-target Cargo, prepare-sidecars, WebKit helper launch, sandbox disabling or new dependencies. Own ignored compile resources copied from `/workspace/.onboarding/goal-editor-compatible-binaries/`: Goal/schema9 Linux debug `baton` SHA `ab5962a6fa6cc981026572f9b0950db8743fc4cc69a492d7202b14413824d979`, `batond` SHA `2b4a302c4e992df9c01cde404c03ba526cf411eb24ecbc7aadc4e85fb8be771e`. These are **old build resources**, not Data10 runtime, macOS, release or installation artifacts. Native leaf compiled against inherited Core9, not the pending schema10 dependency.

## Pending real dimensions

Independent review; composed schema10 real IPC/data copies/cancel/unknown/restart/source immutability/managed-root checks; actual browser screenshots and native OwnerDialog; native WebView/macOS dual architecture/CI remain **not run here**. Parent/Core author separately owns persistence/history budget checks. No mock state is counted as real ready or end-to-end acceptance. No model/private credential/account/paid resource/signing/production action was used, and no complete-product or execution-permission claim is made.

## Independent review F1/F2 — final public-state and recovery correction

Current source `a436be79c5a9099c0b40341db1984629bdb9ba71`. Initial `df0178a`,302 tests, native17 and all earlier hashes/logs above are preserved, not relabeled final. Native/Projects/style and every old259 test/harness/lock are unchanged by this correction.

The author read-only copied the independent controls into owned tests before editing production code:

- F1 actual old-source run: **exit1,1 failed**,22 outside-filter cases not run. Copy revision3 creating → cancel transport Error → remount → same ID's stale Ready revision2 cleared pending. Expected disabled Recover and scope error; old source had removed Recover and displayed Ready. Log `f1/old-stale-remount.log` SHA `0b31e72746093107d38add45b31e109cdef20041d2440447cb459dbda33fd172`.
- F2 actual old-source run: **exit1,5 failed**,21 outside-filter cases not run. `dataCreationShape` accepted observation=null, observation failed, foreign dataset, wrong destination and partial count. Log `f2/old-ready-shape.log` SHA `b8a74df1e69a8b2a359592ac95bbf5bcff59603c2b94491615204a7b63325be0`.

These were targeted controls, not skipped full acceptance. Independent original failed logs also remain under `/workspace/.onboarding/data-workspace-ui-independent/`; author did not modify reviewer's temporary export/link/source.

```sh
npm test -- src/DataWorkspaces.test.tsx -t 'stale terminal copy across remount'
npm test -- src/data-workspace-api.test.ts -t 'refuses contradictory'
npm run typecheck
npm test
npm run build
```

The correction stores only bounded public revision/project-version floors and exact pending operation payload/key/CAS in the existing in-process recovery cache. A successful mutation's valid response raises the floor and retains an acknowledged receipt until a sufficiently new, in-scope read confirms it. Failed/missing/foreign/stale reads cannot recover freshness or drop the unresolved intent. Cancelled-request intent can reconcile to actual newer Ready/failed states without claiming stop. A same-preview copy with a different project_revision cannot satisfy approval. No new mutation, account, API namespace or backend permission is added.

Ready is an internally consistent public receipt, not its state label alone: observation must be real ready, count equals selected files, destination/dataset_revision/selection_digest agree with record/snapshot; errors must be null; public contract and available approved preview must agree. Test fixtures now mirror the formal `DataCreationView` fields and copied-file input observations. Opaque OS identities remain readonly; no private verification token/digest is reconstructed in JavaScript. Historical Ready remains readable when the current Project has changed.

One added cross-preview check correctly rejected an inconsistent pre-existing test fixture (preview project_revision2/copy3 for the same preview). Actual full run at07:21:43 UTC: **324 passed/1 failed**; retained `f1f2-final/ui.log` SHA `bded3874b87a173bf383c65d8cf65843c1fad227c7a3133750e3d1e597b07a39`. Only that fixture's copy.project_revision became2; all its old assertions remain, confirming archived historical reads/cancellation while new approval is disabled. No product guard was relaxed.

Final exact-source run at07:23:05 UTC: **325/325 in18 files PASS**,12.14s, **types/build PASS**. Original43 new cases remain semantically asserted plus23 regressions, original259 files unmodified. Additional coverage includes positive newer/historical remount reconciliation, acknowledged cancel response3→list2 refusal, foreign-project approval, corrupt Ready no callback/no write, linked preview mismatch, record/observation errors and legal source16→inputs/17 path behavior.

| Final changed file | SHA-256 |
| --- | --- |
| `DataWorkspaces.tsx` | `334ae7c8a7c08a16c7c771057953140d0d7b99683759dc0a0356c6af2057de5a` |
| `DataWorkspaces.test.tsx` | `690dafc4cc42f7ad38b00a60158f3d59f5cbed80a277468fdcb72638531f833f` |
| `data-workspace-api.ts` | `0323365017f4cedbebc5350931b8ea5b308eff416776c0f532c5490d4d7424d6` |
| `data-workspace-api.test.ts` | `fa5b9e0575c6197130b7d3c88a306d9295950a915d99e2175cf244284ca1ad9e` |
| `data-workspace-i18n.ts` | `3c52daaf946ef5138a845b2e5f2eea957fa675ddb7fd177b949c17423a2240d1` |
| test-only fixtures | `b4cbc62a7a9f7a6de3a541c3e7310e1f51cb5f3311afa35cd284e4866587b1bf` |
| `f1f2-frozen/typecheck.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
| `f1f2-frozen/ui.log` | `9b9e3cdf97c78be2de41d8a8d874176535a9f7db1600965a84bbd04b12d00a0d` |
| `f1f2-frozen/build.log` | `4908005607d123fe0fd5ff7405300dea962a184a9f8389dfc74de6d11d1dad44` |

Paths resolve under `/workspace/.onboarding/data-workspace-ui/`. Built JS `index-Cj0Bu_O8.js` SHA `700b2d52f14affd5a82f5884b5644fef8a9639bef28ec7e632be8f198ee8f001`. Earlier passing49/323 intermediate logs also remain. No final native rerun is claimed or needed for byte-identical Rust source; native target remains released. Independent F1/F2 re-review and actual Core10/daemon/owner-window/browser/Mac validation remain pending.
