# Goal planning frontend/native bridge independent review

Initial frozen source `8ee7c105da36d5a1168cd70b5ec4fa3ec1b0a806`, base `322461e`; documentation checkpoint during review `ac0c400a845d8243432139a87d88827bbd8930ef`. All nine initial source hashes matched before and after independent verification. Read current gate/approval and OWNER-001/002 from local protected-main snapshot `66061439c79fa44b66a33782febc1fb0f31c73df`; no new owner authorization inferred. Scope is UI and native bridge only; GoalStore/schema9 remains separate ongoing implementation, not accepted by these UI mocks/native units.

## F1 — malformed projection freshness and preview rendering, closed on98a38d4

Source review found scopeInScope did not validate expires_at_ms. Deleting that field from an otherwise valid returned scope left fresh=true; undefined<=Date.now() is false, so new plan/preview controls could remain enabled. Preview checks accepted an object-valued decision.blocked reason, which JSX would attempt to render as an object; nonstring unknown_dimensions also passed shape checks. Reported to primary/author; Core remains authoritative, and this is a defensive read/projection defect rather than a demonstrated grant escalation. Author-owned correction frozen `98a38d4cd486afa2a4fe476d0f464f1763d0805a` changes only GoalPlanning.tsx and its tests: signed safe expiry is mandatory for scope/grant reads, blocked reasons must be strings keyed by actual current nodes, and unknown/blocked reason arrays must contain strings. An actual latest malformed read now clears preview/freshness and blocks writes before rendering. The preserved pre-fix control has four failures, including an actual React object-render error (author execution, independently read and hash-verified). Five regression cases cover missing/unsafe scope expiry, object blocked reason, object unknown dimension, and safe exact-key/CAS recovery for a valid signed past expiry while new actions remain refused. This reviewer independently reran all229 UI tests/typecheck/build on the corrected snapshot; all pass. No unresolved blocking frontend/native-bridge finding remains in this reviewed scope. Reviewer did not edit source/tests.

## Authority, binding and recovery audit

Goal approval is a separate native kind and cannot reuse execution grant, integrity/data/command approval or renderer authority. The native dialog displays the complete server-resolved snapshot and explicitly calls out exact original root grant revision, grant_before/grant_after, only child/depth ceilings changing, retained shared consumed/active counters, unknown measurements, future owner-review criteria and no reservation/dispatch/acceptance. No verified/pass/identity or consumed counters are supplied by renderer selection. The same pre-finalize classification boundary from earlier reviews is retained; native denial does not finalize.

Selection pins task revision and actual root grant revision, existing local project/root grant and immutable execution context. Only live nonrevoked local roots with exactly one workspace/connection/host are selectable. Form preflight bounds safe decimal integers, cumulative root/project limits, output caps, fixed SHA inputs, unique artifact paths/IDs, writable prefixes, owner criteria and validator references; scope is proposal-only. Requested attempts/concurrency/duration cannot exceed the original grant. Token/cost limits remain null instead of zero or implicit spending permission. Child/depth increments must be explicitly shown and approved; consumed/active budgets are read from backend observations, never reset by the UI. A form ceiling is not available remaining budget.

The nine new native handlers are fixed approve/list/get/revoke scope and propose/revise/get/list/readiness preview operations, with current-project grants_list reused. Compared names and DTO shapes against ongoing goal-store goal_store.rs/goal_plan.rs/router: raw plan_json string and outer revision/key match the intended contract. This is source-level wire comparison, not proof those evolving Core9 methods ran. No dispatch/reserve/accept, generic invoke/shell or renderer finalize method was added. Native/root/Node locks, library/Core source and capability permissions have zero delta from base.

Raw plan text is never normalized on ordinary submit or recovery. The strict bounded parser detects duplicate keys (including Unicode-escaped duplicates), unsafe/inexact/nondecimal integer spellings, depth/value count/256KiB UTF-8 bounds, malformed/trailing JSON and nested unknown authority fields before transport. Schema, scope authority revision, prior plan ID, goal/execution binding, exact local program hash, CAS progression, rule/contract IDs, output/input shapes and supported side-effect classes are checked. External/production/remote actions and unsupported token/cost dimensions are refused. The Core still validates the complete DAG/dependencies/resources/budgets; frontend parsing is not a validation receipt. Insert-header/next-revision helpers visibly create an empty node list or explicit revision draft, not a planner result.

The Rust raw frame helper preserves text, whitespace, duplicate fields and UTF-8 bytes inside plan_json instead of deserializing to Value first. Raw text>256KiB yields known not_started refusal; Core enforces its complete serialized-frame and typed-plan limits separately. Original raw text, selected scope ID, key and CAS are preserved in unresolved intents across remount. Cache is task-keyed, with captured exact scope ID; pending selection controls prohibit switching the active request to another scope. No automatic write/outbox, no new key/CAS or retry normalization. Cache32 saturation is source-reviewed, not independently exercised.

Successful latest scope-valid reads are required before explicit exact-key recovery. Later failed/foreign history/scope reads clear freshness. Known stale/input-change errors require a new read before new writes. Revoked/stale authority may be used only to retrieve the exact already-committed historical result by its original payload/key/CAS; backend replay was source-read before current-authority checks, while a cache miss still requires live authority. UI ordinary new plan/preview paths remain gated by revoked scope, changed task/root grant/spec revisions, expiry, local host and validated latest plan state. Approval recovery sets fresh=false; it cannot make a cached old unrevoked scope active. Exact plan recovery updates history without replacing current scope or promoting its authority. Revocation retains readonly history and does not claim existing workers stopped. No budget reservation or attempt dispatch can be initiated by these preview routes.

## Read-only preview and UX

Preview requires exact scope/plan/revision/digest and strict read_only=true, reservation_made=false, dispatched=false. Candidate node IDs must belong to the current plan; they are labeled prospective candidates requiring a later atomic store decision. Actual root/project attempts/active/children counters are displayed, while null runtime/output/token/cost remain Unknown / not measured, never fabricated zeros. Unknown history/blocked observations do not become readiness or task acceptance. No receipt, natural-language completion, Git integration or production release is inferred.

Goal is mounted once under the existing task Details view, default folded, with no Goal read until explicit first expansion. Closed-panel alert/status roles are suppressed without hiding the actual state when reopened. Original App and all13 prior test files are byte-identical; the earlier closed-panel alert collision was fixed in product mounting behavior, not by weakening old assertions. Namespaced CSS preserves Quiet Canvas/composer/shared tokens and44px mobile controls; English/Chinese messages clearly distinguish planning-only, future acceptance and unknown metering. All IDs/JSON/history and hostile text render as text, not HTML or runnable output. No actual visual/browser or native dialog acceptance is claimed here.

## Initial independent execution

Cwd `/workspace/.worktrees/goal-ui/apps/desktop`: `npm run typecheck`, `npm test` and `npm run build`, all exit0. Full224 tests/15 files: prior182 retained plus42 Goal cases. Actual assertions include multi-node raw transport, nested unknown/duplicate/unsafe JSON rejection, per-task/grant scope, exact same-key recovery after history advances/remount/revocation, latest-read failure blocking, expired/foreign readiness rejection, stale CAS behavior, unknown nullable counters, native refusal and lazy folded/disconnected behavior.

Independent Linux native command `cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml`: exit0,13 library tests;0 main/doctests. Existing native target only after primary released it. Environment: CARGO_HOME=/workspace/.toolchains/cargo, RUSTUP_HOME=/workspace/.toolchains/rustup, PATH prefixed with toolchain bin, CARGO_INCREMENTAL=0, CARGO_BUILD_JOBS=2, CARGO_TARGET_DIR=/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target, PKG_CONFIG_SYSROOT_DIR=/workspace/.toolchains/linux-ui/sysroot, PKG_CONFIG_LIBDIR=sysroot/usr/lib/x86_64-linux-gnu/pkgconfig:sysroot/usr/share/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig and LD_LIBRARY_PATH/LIBRARY_PATH prefixed with sysroot/usr/lib/x86_64-linux-gnu. No new target/dependency build; workspace GoalStore target was untouched. Native units exercise separate Goal kind and exact raw-frame behavior, not a real owner click or Goal daemon.

All9 source and12 worker log hashes matched initially, including retained failed controls. Author Clippy/fmt/check were read and hash-verified, not independently repeated. Actual schema8 debug sidecars are compile resources only, with daemon749e995f and CLI1e4a49d fingerprints from the recorded manifest; they cannot serve as Core9/Goal execution evidence. Earlier Command browser5 checks are not Goal evidence.

## Limits and integration

F1 is closed on the independently verified98a38d4 follow-up. No actual Core9/schema migration/Goal IPC, native owner dialog or WebView/browser interaction, macOS/dual-architecture package, NL planner, reservation/dispatch/scheduler/node execution or task acceptance was run here. Exact-head protected CI and actual integrated-source testing remain needed. No Provider/model/private-account call, new spending, production release/signing or native Goal platform state inferred. No reviewer source/test/lock edit, staging, commit, push or subagent. Only this review document, ignored logs and a temporary exact borrowed dependency symlink were created. The worker first removed their own link; reviewer subsequently created a new exact Root link and removed only that link after verification. Actual dependencies and locks were not modified.

## Final corrected snapshot and evidence

Final immutable source98a38d4 retains all seven other reviewed source files byte-identically to8ee7c10. All nine current files were compared against exact98a38d4 git objects after the final run and match. The initial independent native13 therefore applies to the identical final native source without a redundant rebuild; it still proves no Core9/Goal IPC or GUI execution.

Final commands `npm run typecheck`, `npm test`, `npm run build`: all exit0. Full229 tests/15 files retain every original224 case plus the five focused read-shape/recovery regressions. No original App/other13 test file, timeout or acceptance assertion was changed. Bad-scope/preview cases start with a previously valid preview and prove it is removed, scope error shown and write controls disabled; signed past expiry remains readable history and preserves exact original raw/key/CAS only on explicit recovery, with server denial leaving every new plan action disabled.

| Final corrected file | SHA-256 |
| --- | --- |
| `apps/desktop/src/GoalPlanning.tsx` | `90fcd894bef5c0e867ffb1667bb25ac5823b895ed2b0574a66d2d1c122aa592e` |
| `apps/desktop/src/GoalPlanning.test.tsx` | `26ce0dc387e8d736d6f967679ab7b0f6677c603884aea9caa089d56f64196b7d` |

## Initial reviewed source SHA-256

| File | SHA-256 |
| --- | --- |
| `apps/desktop/src/GoalPlanning.tsx` | `1d530af0006f942a9f343f699bbac553bdaf97be8d53c06eab55a5eaa774a812` |
| `apps/desktop/src/GoalPlanning.test.tsx` | `4dfc0ed7cdf0d64a47d1cba530f80cfe8ec26a79722e61dd6de5e9fe7084d15b` |
| `apps/desktop/src/goal-api.ts` | `24dcf1d3af72666da1b902ffb3689f47b6fdd1040389b8decd7949b68664d4be` |
| `apps/desktop/src/goal-api.test.ts` | `155913e9b5a3c968682c91717f9c68039ae2fd57a347352ddf5378c35e52fa1f` |
| `apps/desktop/src/goal-i18n.ts` | `f93e2109599583857812edc1aee90e2a1e17c4fcb745ea5883d77332a563c246` |
| `apps/desktop/src/App.tsx` | `92f94ef85ab9e76e7b0cdb6041336c8e46acad95d2ec6e443ef12e1fb8fe7612` |
| `apps/desktop/src/style.css` | `852558d2536173c8727388b9f6a93c29b6740855c2de64ff6f17a15c35474a2e` |
| `apps/desktop/src-tauri/src/lib.rs` | `06c8e8fe9b7302a550b3d55673ef0974215aed0fb3d14b10c2598a235425042f` |
| `apps/desktop/src-tauri/src/approval.rs` | `50365599c6bdfc7abd8b29e994cb2578862feb40159e69ccf1613943748a35b2` |

## Independent logs SHA-256

Relative to `/workspace/.onboarding/goal-ui-independent/`. Initial224 and final229 executions remain separate.

| Artifact | SHA-256 |
| --- | --- |
| `build-final.log` | `a0b15ae539bb9ecbd07693bee8211b4c8cb568e91985b2e973ea8ea674498900` |
| `build.log` | `6df2949a0a16e8a4e958f175bcec030b81689704fc996bda1c1e6228787c9e66` |
| `native-tests.log` | `cd95e7f0f4804ba628a9b3d8f131b4564e6eb00a54717ea3fe5dc124ea38202c` |
| `source-before.json` | `00ad561d669ae5c4513cd8d0bf66a682f087ce785b7b06b85ba8ed13e31f959d` |
| `source-final.json` | `a9db41bbca8782b9b52ad8374affc673cb6c29d6183fa6c32e66f4da99bf0923` |
| `tests-final229.log` | `058e619b42073c0c1843842b634d2aedd08857a2acb93639e768269f185d138a` |
| `tests.log` | `4cf7903fb96bee79e487f9c697150b9bb4622ec2c57976f7f9f490caa6efc791` |
| `typecheck-final.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
| `typecheck.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
| `/workspace/.onboarding/goal-ui-shape-failing-control.log` (author pre-fix negative control) | `172cce990e218ee8fd4b1774293404dfa04641238c06d277555496bc59f5799a` |
