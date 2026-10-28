# Candidate evidence

Previously executed on frozen37c source candidate:

- `npm test --prefix apps/desktop`:611 passed/40 files,0 failures/errors/skips;21.22s. Final log `.onboarding/factory-runtime-ui/frozen-candidate/node-full.log`.
- `npm run typecheck --prefix apps/desktop`:exit0.
- `npm run build --prefix apps/desktop`:exit0 (real Vite/TypeScript production build, not browser/Native verification).
- Direct `rustfmt --edition2021 --check` on approval.rs/lib.rs:exit0, syntax/format only.
- `git diff --check`:exit0.
- Source comparison:37 original Node test files, original Native approval source body/test module and original lib test module preserved. Cargo/package locks, production native_parent, globalstyle, original native_smoke and readonly runner/mac script/Python test files unchanged. `.onboarding/factory-runtime-ui/source-preservation.json`.

Final commands/exit codes/timing are `.onboarding/factory-runtime-ui/frozen-candidate/results.json`; original606 fullsuite and interim checks remain separate. Failed new two-test run is `.onboarding/factory-runtime-ui/new-floor-poll-before-fix.log` (2 failures, higher-scope diagnostic and unchanged-read polling). Both corrected without dropping assertions; new historical-failed-recovery control retains original unknown after current Native Cancel/not_started. No Native test failure was fabricated: eight new Rust bodies have not run.

Mocked Node controls cover exact seven wrappers, denied/unknown errors, path/integer/contract bounds, independent Grant versus Factory splitting limits, opaque big u64 metadata, altered root/grant/companion/start/unsupported fields, scoped history, no automatic start, unknown exact-key recovery/remount, opposite-operation block, stale/higher-fork floors, historical acknowledgement without floor rollback, cancelled-request versus task stop, terminal/future state, disconnected state, revoke-not-stop, synchronous duplicate-click guard and bounded readonly polling. No mock establishes Core runtime, actual files, Vendor, native dialog or macOS behavior.

Source SHA256:

- `apps/desktop/src-tauri/src/approval.rs` `706b671631f1d8ff7e5c223a6f0179c733092034dc19998f88c7e3fd40a49ac9`
- `apps/desktop/src-tauri/src/lib.rs` `55850dd385a176a786c5a760e8fe388cc13514fa3c733dcac205ecce9b757490`
- `apps/desktop/src/App.tsx` `3103f406bf5b6dc7de7ae8e7105e9229b81fcd653cd104341c927614f47d042f`
- `apps/desktop/src/FactoryRuntime.test.tsx` `a278585ff26f83fe85e2785f7c3988e3214157a89ebf998136f0ec12ca6a5651`
- `apps/desktop/src/FactoryRuntime.tsx` `54245368c34fbce52afeec5d4b1eb5ee9db9309d47e3498d194c2a9d2fd9accd`
- `apps/desktop/src/factory-api.test.ts` `793ff1f2742a1a4126b774a3876101908dc81bfc05be39ace53f3d803d351277`
- `apps/desktop/src/factory-api.ts` `ad7d1e87878688f0d83ac46bbe0409a10b49813b5ed6873406c3f7a7a2be0adc`
- `apps/desktop/src/factory-form.ts` `c6bc7342186ce293bf2b906dc3a9f5a63bc782a80a1d4a17bb49ce4223ebe49e`
- `apps/desktop/src/factory-guards.test.ts` `73228e2b96e739c6d56f3c215b5fac98470cf9941aeed669b949341adca670c0`
- `apps/desktop/src/factory-guards.ts` `e5a14a451e00554a304729c9a4f81073bb69e5430d031b482b743e68913f2ccd`
- `apps/desktop/src/factory-i18n.ts` `0672e5f275ad304851467fb57029ea4bd53cde8d8fd8927922d36cb9bdaa13f6`
- `apps/desktop/src/factory-runtime.css` `da3ed3215db4fe7fc5e0fec8b290061b2c6ec9d4e7ddc3f7bd269fecbe7ac25f`
- `apps/desktop/src/factory-test-fixtures.ts` `3cc39fd637256b7235f39b7a246fb029177cb1ee4f4eeef1f6bcbc152a1310db`
- `apps/desktop/src/factory-types.ts` `c84243c8788fee69ea4be3efa75ecf349ae10ac25dc6dde7d711c19e1d16e9cf`

Core contract source read directly; still no actual Factory JSON/runtime integration fixture at this freeze. Core confirmed Draft-only initial scopes, lineage-derived task budget roots and full-root historical unknown-run gate; do not infer these from renderer flags. Filesystem metadata is not reconstructed/digested in JS. New Native handler/source tests use original serde values and preserve high-u64 literals but are not yet compiled.

NOT_RUN: new8 Rust tests/Clippy, composed Core13/storage/realMCP/shared-ledger runtime tests, browser and real IPC, actual Owner popup, macOS ARM/Intel Factory behavior, real Provider/account/model calls. Root will assign independent review and target lease. No paid resource, credentials, OS permission, signing, production deployment or license changes.

## Post-review actual revision evidence

- `npm test --prefix apps/desktop`:626 passed/41 files; final Vitest duration23.35s. `revision-two/node-full.log`.
- `npm run typecheck --prefix apps/desktop`, `npm run build --prefix apps/desktop`:exit0; exact commands/codes/timing in `revision-two/results.json`.
- Standalone rustfmt on Native approval.rs/lib.rs and `git diff --check`:exit0; not Cargo/type compilation.
- Targeted guards/component/actual wire tests:59 passed before the final extra Project-floor regression; full626 includes it.
- Prior independent37c FR01–03 controls failed as preserved in `.onboarding/factory-runtime-ui-independent-review/independent-negative-probes.log` SHA9ef44cf09ce4603033398b2e563216b887c690f64867d19f931e2564f38a8de0; FR04 was static Native recipient omission. These are Renderer/Native defensive evidence defects, not an actual Core authority bypass.
- Initial new test typecheck failure: `revision-two/typecheck-initial.log`; no dependency installed. Final source uses existing Node WebCrypto test environment for immutable fixture SHA, not browser/Mac proof.
- Copied actual fixture source `/workspace/.worktrees/factory-runtime-core/artifacts/factory-runtime-core/wire-bundle-0c6fbd065df4a49997acca4fc898a3f952721bfbc60b57229ca36571db0c0c64.json`; destination `apps/desktop/src-tauri/tests/fixtures/factory-wire-bundle-0c6fbd065df4a49997acca4fc898a3f952721bfbc60b57229ca36571db0c0c64.json`; original SHA2560c6fbd065df4a49997acca4fc898a3f952721bfbc60b57229ca36571db0c0c64 unchanged. Generated actual Store propose→trusted Owner-finalizer fixture→scope→durable queuedRun, generated Git/input and synthetic connection; no actual Provider, OS dialog, runtime parent/child or shared-ledger success inferred.
- New10 Rust bodies SOURCE ONLY (original8 plus actual wire/private-frame binding and negative host/recipient/Project controls). No Cargo/target use. Await independent review and composed compile.
- Source preservation `revision-two/source-preservation.json`; current source/log/fixture SHA in `revision-two/manifest.json`. Prior37c manifest and logs unchanged.

## FR05 follow-up current source evidence

- Full actual Node:640 passed/41 files,26.27s Vitest duration; typecheck/build exit0. Exact commands/codes/timing `revision-three/results.json`, logs in the same directory. Standalone rustfmt approval.rs/lib.rs and diff exit0, no Cargo.
- Independent original4279 seven grant-pin drift negatives failed: `.onboarding/factory-runtime-ui-independent-review-r2/grant-pin-probes.log` SHA1a736a7f5ffa47ba9fcabdcf331da3a57ab643c71b5eef425080adc1fece46f2. Old626/41 pass and FR01–04 resolution remain unchanged; this is a defensive binding gap, not proof of actual Core authority bypass.
- New actual0c6f derivatives:8 connection business-field drift cases,4 missing selected pin/map cases, opaque filesystem structural checks and one component execution gate case. The original immutable fixture bytes/SHA remain unchanged; derivatives do not establish a new actual Core execution.
- First new640 Node run passed but test fixture typecheck failed (optional shared NamedConnection.protocol_route); logs preserved `revision-three/initial-attempt`. Final fixture uses its already-selected custom_acp string, with no shared type/API/lock/dependency/config change. Final640/type/build all pass.
- Native12 new source-only tests; extra two cover original connection pins, full exact workspace identity, artifact contract root device/inode and u64=9223372036854775809 positive versus +1 drift. Not compiled or executed. Root still owns compile/integration scheduling.
- JS never compares filesystem numeric identity values or reserializes them for authority. Renderer requires pin structure only; Native retains original serde Value equality and complete review bytes. Server freshness checks remain independent and required.
- Source preservation and source/log SHA: `revision-three/source-preservation.json` and `revision-three/manifest.json`. Prior manifests, original failures and fixture bytes unchanged. Still no Provider/OSdialog/Mac/shared-ledger/parent-child runtime proof from this leaf.

## Final protocol-type refinement

- Exact final current source:643 passed/41 Node files,21.72s Vitest duration; typecheck/build exit0, standalone rustfmt/diff exit0. `revision-four/results.json` and logs hold actual commands/codes. Native13 source-only, no Cargo/target used.
- Preserved independent d128 protocol-shape old3FAIL: `.onboarding/factory-runtime-ui-independent-review-r3/protocol-shape.log` SHAc88f9de0bb60282aade7d45e0def1131331cca4346ca0a62607a3eb2f0af95b6. Independent required-field matrix24 cases had21PASS/onlyroute3FAIL: `required-field-matrix.log` SHAcdbae2ee11b311cca0b3464b83f03589583a07632f48e6eac0290aafd0c15be8. Existing11 controls and640 Node results remain historical, not reused as current proof.
- Narrow code addition only: protocol_route belongs in the existing required nonempty string check in JS and Native; exact Grant pin equality still applies. No enum change, field widening, identity-number comparison or new authority. New3 permanent Node regressions plus1 Native source loop retain missing/null/empty rejection.
- Source hashes/preservation and current logs are `revision-four/manifest.json` and `source-preservation.json`. No new Core run/IPC/Provider/OSdialog/macOS evidence.
