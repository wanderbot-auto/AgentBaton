# Independent final session Git-guard merge review

Reviewer: native `/root/integration_review`, independent execution context. Reviewed candidate `334d71e`, tested merge `3bd37e614047c7b3a33616cdfdab0c4c4d895061` of Session `b85a64d` and guard `32ca76e`, at 2026-10-26T21:23:24.121407+00:00. Only this review document was written; no sources, locks, root files, previous reviews, staging, commits or remote operations were changed.

Verdict: no unresolved blocker in this Session composition. Existing actor/schema4/recipient/history behavior is preserved; the new shared Git refusal/raw-object guards are integrated without losing hidden-index/stat-trust protections. This review is conditional on the integrator completing required exact-candidate CI and protected-main ancestry reconciliation; it does not establish native or full-product acceptance.

## Preservation and added behavior

Compared with b85a64d/cf00, core IPC/storage/policy/sessions, both Cargo locks and every tracked desktop file are byte-identical. Existing 18 workspace safety tests, including UnverifiedIndex and all five hidden-index/stat-trust regressions, are unchanged. The single Display conflict preserves UnverifiedIndex while broadening UnsafeRepository to include configuration indirection and external objects. No schema5, native route, adoption/profile modules or new dependency were copied into this branch.

The shared Git helper keeps isolated environment/config, hooks/fsmonitor/submodule restrictions and root stat overrides. It adds no-lazy-fetch/protocol defaults, refuses promisor/partial-clone metadata before object/status operations, and ignores replacement refs/legacy grafts for exact raw tree/ancestry semantics. Immediate `--no-includes` declaration auditing rejects direct or future-conditional includes before repository probing and again before checkout. No source/index/config/refs are repaired or rewritten. The six added regressions include genuine ordinary-Git replacement/graft controls and a no-checkout child context demonstrating conditional configuration visibility without executing its filter.

The resulting library/test bytes are identical to the separately reviewed root conditional-guard composition, but this review ran the Session candidate's own tests. Prior root/native/GUI results are not substituted for Session evidence.

## Independent validation

At the Session worktree under `umask 022`, configured CARGO_HOME/RUSTUP_HOME/PATH and CARGO_BUILD_JOBS=2:

| Command | Actual result |
| --- | --- |
| `cargo test -p agentbaton-workspaces --locked` | PASS: 24 (existing 18 safety + security 5 + graft 1). |
| `python3 scripts/verify_bootstrap_manifest.py` | PASS: 92 files. |
| `git diff --check` | PASS. |

Verified all **68 current source hashes**, all **14 log/evidence hashes**, all 8 explicit preserved-source hashes, and the preserved cf00 EVIDENCE archive digest. Core/desktop/lock comparisons against b85a64d also pass. Read the actual full Rust log: 200 passing result-line tests include one nested FIFO child, hence **199 top-level tests** (core 70/config 45/providers 58/workspaces 24/daemon signals 2). Actual UI log reports 35 tests; current clippy/build/restart/fmt/manifest checks are recorded with exit 0 and their logs hash-match. These full commands were observed, not independently rerun in full here.

Current EVIDENCE.json SHA-256: `bf458e13711a56d8dd5b8300074aac2acfced92f5ebcd167806e21a089df27cf`. Preserved cf00 evidence SHA-256: `d320f704bb6060f3358f2ca0b04c6be7658ce3e9c6ea79b5d0d1c33aa7eb2ae7`.

Only generated local repositories/helpers were used; no vendor/account/model, credentials, production release, signing or extra paid service was invoked. The old 3b94 native CI failure remains historical failure; no current Session macOS/remote result is claimed from the root branch. Full Design v0.2 remains incomplete. After PR9's required checks and authorized merge, the integrator must reconcile Session with the actual protected-main head and validate the resulting exact candidate before guarded integration.

## Exact guard fingerprints

| File | SHA-256 |
| --- | --- |
| `crates/workspaces/src/lib.rs` | `796e8cf7b2f92052f01b4140ac7f76fa47631959190138fbaa755ff7b682491e` |
| `crates/workspaces/tests/lazy_fetch.rs` | `8f4de1e700b317e9e100a8bff3ec9f4feb9402b9a86f218992a020b577dcf711` |
| `crates/workspaces/tests/safety.rs` | `aea419fc8d4f4640d1522170063b034a958c2be2951714cbe1cf2cf8bc990420` |
