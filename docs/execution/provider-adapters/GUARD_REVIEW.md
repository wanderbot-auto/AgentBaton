# Independent shared Git guard integration review

Reviewer: native `/root/integration_review`, independent execution context. Reviewed intentional merge in progress from root `30f466d` with focused guard branch `851fa073` (sources 4d43989 and 8dcb4a3), at 2026-10-26T21:03:51.852223+00:00. Only this review document was written; no source/lock/staging/commit/push or merge operation was changed.

Verdict: no unresolved blocker in the composed guard. The new shared Git restrictions and raw-object semantics retain the root's existing hidden-index/stat-trust protections. This is Linux source/fixture evidence and does not replace exact-commit required CI or native macOS verification.

## Composition and behavior

- Source delta is limited to `crates/workspaces/src/lib.rs` and new `tests/lazy_fetch.rs`; dependency manifests/locks, core authority, native runtime, UI and existing safety tests are unchanged. The sole Display conflict retains root UnverifiedIndex and generalizes UnsafeRepository for both executable filters and external object access.
- The Git command still removes inherited GIT selectors and ignores global/system configuration, hooks, fsmonitor, global attributes and recursive submodules. Root core.ignoreStat=false/core.trustctime=true/core.checkStat=default remain. New GIT_NO_LAZY_FETCH, protocol defaults and explicit partial-clone/promisor metadata refusal run before object/status operations and are repeated before checkout. Unsupported partial clones fail closed without fetching or repairing configuration; the metadata check covers older Git versions that may ignore the no-lazy-fetch selector.
- GIT_NO_REPLACE_OBJECTS=1 and GIT_GRAFT_FILE=/dev/null make approved hashes refer to raw trees/parents. Replacement/graft files, source content and index bytes are left intact. The genuine ordinary-Git controls demonstrate misleading clean/hash behavior and changed ancestry; guarded inspection, adoption checks and isolated checkout read the approved raw objects instead.
- Existing 18 safety tests are byte-identical to 30f466d, including all five hidden-index/stat-trust regressions. The new 3 integration tests exactly match 851fa073, with a fourth new unit regression for legacy graft ancestry. The controlled missing-blob fixture deliberately configures protocol.ext.allow=always and a local generated helper that could only write a synthetic marker; guarded import/checkout refuse before that helper executes. No real network/vendor/account was invoked.
- Protocol defaults and configuration refusals are application-level Git controls, not an OS network sandbox or proof against arbitrary same-user filesystem races. No source files, replacement refs, index flags or external configuration are rewritten to make validation pass.

## Independently executed checks

At `/workspace/AgentBaton` under `umask 022`, configured CARGO_HOME/RUSTUP_HOME/PATH and CARGO_BUILD_JOBS=2, ran `cargo test -p agentbaton-workspaces --locked`: **22 PASS** (existing 18 safety +new 3 integration +new 1 graft unit). Also inspected `git diff 30f466d` and verified no unresolved index conflicts or whitespace errors.

The implementer's actual guarded root Rust log has 251 passing result-line tests including one nested FIFO child, hence **250 top-level tests** (core 78/config 67/providers 81/workspaces 22/daemon 2). Read current guarded build/clippy/restart success logs; fmt is recorded separately by the integrator. These full checks were observed, not independently rerun in full by this reviewer. Older focused-branch 128/17 evidence is historical, not substituted for the composed root 22/250 result.

All 39 fingerprints in the preceding root MERGE_REVIEW.md still match. The two guard files bring that reviewed integration source set to 41; the table below also binds the unchanged existing safety tests used to establish preservation. The composed library fingerprint intentionally differs from the older-main focused branch because root's five additional guards remain.

No native macOS/remote CI success, real vendor/model/account call, private credentials, production release or additional paid resource is claimed. Full Design v0.2 remains incomplete. The previous root CI FIFO failures remain failures until a fresh exact-candidate native run establishes the corrections.

## Exact composed source fingerprints

| File | SHA-256 |
| --- | --- |
| `crates/workspaces/src/lib.rs` | `9fe2caec23d31b599e6d7767235130102e7414efdce17d1bac083ab99f543a98` |
| `crates/workspaces/tests/lazy_fetch.rs` | `e514f8ecd8e1393ad8edd897b74dc11d39dadf2c76627412ab86a873e45e24d0` |
| `crates/workspaces/tests/safety.rs` | `aea419fc8d4f4640d1522170063b034a958c2be2951714cbe1cf2cf8bc990420` |

## Observed guarded integration logs

| Log | SHA-256 |
| --- | --- |
| `/workspace/.onboarding/provider-runtime-guarded-rust.log` | `0fa7c8b6856ac22d49e2e38d29a6e008b43f7cb9dd3c5b28395398ccb367504c` |
| `/workspace/.onboarding/provider-runtime-guarded-clippy.log` | `1a33a2e9fb62c5b2af130a5c2604273119403ee36cdd80bb0a5f22eff092bfcd` |
| `/workspace/.onboarding/provider-runtime-guarded-fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `/workspace/.onboarding/provider-runtime-guarded-build.log` | `0bd524d8529041596176b321f302b78bc21454fcfc6d5a90d212ce9eb3d5855d` |
| `/workspace/.onboarding/provider-runtime-guarded-smoke.log` | `3e4b9b3606e773bb0c39912df2ea993aac9bbbc73b7cfe097c06d2081e5dd805` |
