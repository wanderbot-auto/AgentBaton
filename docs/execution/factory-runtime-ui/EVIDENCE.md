# Candidate evidence

Actually executed on this source candidate:

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
