# Independent Intel UI CI readiness review

Reviewer: native `/root/integration_review`, separate execution context. Reviewed the UI-test correction above root `f3eb83e` and the concurrent conditional Git guard merge in progress, at 2026-10-26T21:19:39.430352+00:00. Only this review document was written; no source, lock, staging, commit or remote operation was changed. Native-harness files being handled by a different agent were outside this review and remain separately gated.

Verdict: no unresolved blocker in these scoped changes. Independent typecheck and all 49 mocked-transport UI tests pass; the composed Git guard's 24 workspace tests pass. These are exact scoped-source checkpoints, not a claim that the previously failed native CI run is now green.

## UI failure and correction

Read actual Intel Product job 121847819047 from run 40401378109: 48 tests ran with 47 passing and the first SessionWorkbench duration case failing while the expected workspace option was absent and DOM still showed Loading. Its original one-hour validation, recipient acknowledgement and dispatch assertions remain valid; the correction addresses fixture setup ordering rather than changing product IO deadlines.

`showSessions` now awaits React async act around initial render, letting its explicitly mocked initial promise/effect results settle before queries for ready controls. Every caller awaits the helper. No production UI/core/API code, test deadline, duration bound, expected authorization condition or assertion was weakened. The added deferred workspace-response case leaves the actual promise unresolved across setup, confirms the option is absent and grant action unavailable without a backend call, then resolves that exact promise and observes the option. It demonstrates that setup flushing does not manufacture a scope response or bypass readiness.

Production ManagedSessions/App and both Cargo locks are byte-identical to f3eb83e. Existing uncertain-send/replay, grant expiry/scope and explicit acknowledgement checks remain. The actual UI remains transport-mocked; this suite does not execute a vendor or prove native Tauri behavior.

## Conditional Git guard composition

Also inspected the intentional merge of the independently reviewed conditional guard (b112185/32ca76). The root retains UnverifiedIndex, all five hidden-index/stat-trust regressions, existing no-lazy/protocol/raw-object/graft controls and source identity/path checks. New `reject_config_indirection` audits declarations with `git config --no-includes` before even probing the repository and again before checkout; direct or future-worktree conditional includes are refused without following or repairing them. A failed audit cannot permit a later object operation. Malformed/unreadable configuration remains NotRepository, preserving its original rejection assertion.

The new future-child control creates only a no-checkout linked worktree, demonstrates that its conditional filter configuration becomes visible there while invisible in the source context, and verifies no helper marker/content checkout occurred. Guarded inspect/create then refuse before creating the negative destination and leave config, included files, index and source bytes unchanged. The separate direct-include case preserves even a benign included file while explicitly refusing unsupported configuration indirection. This is application-level conservative Git policy, not an OS sandbox. It does not claim an old compiled Baton execution was reproduced.

## Independent commands

| Command | Actual result |
| --- | --- |
| `npm run typecheck && npm test -- --reporter=dot` in apps/desktop | PASS: typecheck + 49 tests /7 files. |
| `cargo test -p agentbaton-workspaces --locked` under 022 and configured toolchain/CARGO_BUILD_JOBS=2 | PASS: 24 (existing 18 safety + 5 security integration + 1 graft unit). |
| `git diff --check` and `git ls-files -u` | PASS; no unresolved index conflict. |

The 18 existing workspace safety tests are byte-identical to f3eb83e. No real vendor/model/account request, private credentials, production action, signing or additional paid resource was used. The Intel failure and ARM native-harness startup/cleanup failure remain failed evidence until fresh exact-candidate CI validates their separately reviewed corrections. No native/macOS GUI or whole-product completion is claimed here.

## Exact scoped fingerprints

These identify only reviewed UI/guard inputs; the concurrently changing native harness is excluded until its separate freeze/review.

| File | SHA-256 |
| --- | --- |
| `apps/desktop/src/SessionWorkbench.test.tsx` | `d3898008b0ccb4a3681bd32eaa1eaf0bb0249a80bbdf1ef6d08609273e3a8070` |
| `crates/workspaces/src/lib.rs` | `796e8cf7b2f92052f01b4140ac7f76fa47631959190138fbaa755ff7b682491e` |
| `crates/workspaces/tests/lazy_fetch.rs` | `8f4de1e700b317e9e100a8bff3ec9f4feb9402b9a86f218992a020b577dcf711` |
| `crates/workspaces/tests/safety.rs` | `aea419fc8d4f4640d1522170063b034a958c2be2951714cbe1cf2cf8bc990420` |

| Preserved failure or accepted local evidence | SHA-256 |
| --- | --- |
| `/workspace/.onboarding/provider-runtime-ci-121847819047.log` | `e7561c7c3fe935a5eef5700a7c081d20c8c05f8454ca61b8efe3dbf0b49e8cef` |
| `/workspace/.onboarding/provider-runtime-intel-ui-readiness-typecheck-accepted.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
| `/workspace/.onboarding/provider-runtime-intel-ui-readiness-tests-accepted.log` | `a3cc741b1c2295999bee8741bcc0ca10fba684d6950cb82006ceb8208e0715b7` |
