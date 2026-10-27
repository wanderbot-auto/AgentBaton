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
