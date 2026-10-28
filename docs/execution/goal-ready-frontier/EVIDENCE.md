# Evidence

Frozen source `04ba1807af1abaedcc5d6cbbce8f57683f7a0843`; baseline `78b317a4dc42f088f752852bf04028eb0bb51d9c`. Runtime SHA-256 `c98b87ba442d9be0b954a44cfff78f4781caae18f8e16d858dc2d473ce384167`; IPC `0cb42b280291ee31bbd1e7673b82f0215979e2c91a5d7fd6df2ed9293146c1a4`; sessions tests `7b0749bce7336f7676202038b9afe2d9235127260b74557a4cfd774a6f3694e0`. Linux x86_64 generated local ACP and actual daemon/session/SQLite/owned process proof; no real Provider/model/account/paid call. Native Owner finalizer fixture is not a macOS dialog.

Logs are `/workspace/.worktrees/goal-ready-frontier/artifacts/goal-ready-frontier/`. CARGO_HOME=/workspace/.toolchains/cargo, RUSTUP_HOME=/workspace/.toolchains/rustup, CARGO_TARGET_DIR=/workspace/.worktrees/validation-integration/target, CARGO_INCREMENTAL=0,CARGO_BUILD_JOBS=2,RUST_TEST_THREADS=1. Actual tests append `-- --test-threads=1 --nocapture`; compile/format checks retain original bounds. No Native target write.

| Actual command/control | Exact log | Result |
| --- | --- | --- |
| cargo test -p agentbaton-core --test sessions_ipc actual_goal_same_plan_two_independent_nodes (unchanged9f) | old-source-two-node-count-fail.log |FAIL with firstcandidate/secondnot_started/SQLite1attempt/runawaiting_acceptance; required old negative |
| cargo test -p agentbaton-core --test sessions_ipc actual_goal_same_plan_ | frontier-positive-and-guards-2.log |2PASS11.52s; Git+Data positives and6 guard scenarios |
| cargo test -p agentbaton-core --lib goal_store::runtime::tests::newer_validated_head | new-head-pause-1.log |1PASS0.96s actualStore no Provider |
| initial concurrent setup attempts | old-temporary-slot-fail.log; old-temporary-slot-2-fail.log |WORKSPACE_BUSY before slot; never relabelled as resource proof |
| actual_goal_temporary_global_slot after fixed Owner route but before slot fix | old-slot-resource-limit-count-fail.log |FAIL: real approvedRoot2 blocked/not_started,total1attempt,actualjournal RESOURCE_LIMIT |
| actual_goal_temporary_global_slot after slot fix | new-slot-and-version-2.log |1PASS5.59s, two real scenarios: quiet queue/ownedcompletion→resume and savednewhead/quietpause/explicitadopt/replay |
| actual_goal_fixed_concurrent_review_keeps_other_kinds_setup_and_corrupt_authority_refused | fixed-kind-guards-2.log |1PASS2.45s: legacy grant/setup/sameworkspace + unknownid/rendererflag/oversize/foreignid/wrongkind/snapshottamper/forgedlegacykind all refuse zero neweffects |
| cargo test -p agentbaton-core | full-core-1.log; full-core-counts.json |279unique outer tests PASS, including53 library/38sessions/oldsingle+replan/migration/storage; nested FIFO child not double counted |
| cargo clippy --workspace --all-targets -- -D warnings | final-clippy.log |PASS |
| cargo fmt --all -- --check | final-format.log |PASS |
| cargo build -p batond -p baton | final-build.log |PASS |
| existing smoke-local-product.py with immutable BINARIES, plus fresh DB schema probe | frozen-binaries-smoke.log |PASS Unicode/idempotency/evidencedcancel/SIGKILL restart and real schema11/0Goalrows |

Final full-core source covers every preceding actual control, including the newest latest-node distinct-attempt assertion and individual denial-stage output. Targeted intermediate logs retain their original embedded runtime hashes; they are not relabelled final-source proof. Other retained failures: old-source-two-node-fail, new guards usizeFromSql compile error (frontier-positive-and-guards-1), and fixed-kind-guards-1 strictpredecode INVALID_REQUEST vs incorrect Busy fixture expectation. old-slot-with-finalizer-count-fail is a repeated earlier partial diagnostic after a matcher failed, not distinct success. No assertion, timeout, hard budget or global ceiling was relaxed.

## Decisions and evidence boundaries

Conservative global single slot remains1, including command-process occupancy already counted by approved host snapshots. Read-only eligibility waits before an advance intent; actual session transaction rechecks the unchanged cap and all root/project reservations. Temporary occupancy uses existing WORKSPACE_BUSY, hard cumulative limits remain RESOURCE_LIMIT. Queued is readiness, never authority or a dispatched receipt.

Only trusted Store existing id-matching/resultNull/!consumed/kind-exact goal_execution may pass the generic idle classifier into its unchanged full private finalizer. Renderer labels cannot select the kind. Other kinds/setup remain idle-only; no_writers, command fences, complete immutable snapshot, native Owner confirmation, exact ledger/run/plan CAS, lineage and fresh current input/witness remain required. New validated head pauses once with cause until explicit adoption; no new Owner for an authorized same-scope revision and no automatic adoption. Candidate/known effects/ownedstop/TaskAccepted remain distinct.

Whole source/binary candidate `/workspace/.onboarding/goal-ready-frontier-schema11-04ba180/manifest.json`, manifest SHA-256 `2488e2a01f80c537284d34ad2e06006149cdd25737850daadfb0ff7da93865ac`.94source map SHA-256 `d6082cf6bee17dc6a12ee28b1519e3faeb3da1cf1b281b9d3daab5ba272509c0`, dirtyfalse at source freeze. Same-directory batond SHA-256 `0a94e3da9f59975b1e7ce6955cdeda80dd9137428f9cca8ffad171514e1d5fdb`,96027128bytes; baton `4e71bc65efaffdbc21c62c88bbc25a292323c55aa8d7b166d5bda4a5085c1a2e`,71029344bytes. Unsigned Linux debug validation executables, not macOS installers; old9f candidates untouched. Known test control strings absent from production executables.

Full graph dependency/validator/Owner acceptance, actual child/factory authority/workspaces and NL planning remain unimplemented parent scope; TaskAcceptedfalse throughout. Real Provider/model/native dialog/macOS/CI/package results are separate and not claimed. Prior full549+7/independent274+27 results remain historical evidence; old no-blocking review was withdrawn for G19, new independent verdict is pending. Main target lease now exclusively belongs to independent reviewer `/root/data_core_review_resume`; author performs no more Cargo writes without a new explicit handoff.
