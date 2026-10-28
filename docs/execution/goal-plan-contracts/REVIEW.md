# Independent review — Goal/plan contracts

Reviewer: native independent `/root/review` context. Initial frozen source: `4564c5cdadd9909240be9b2d87a250905621b5ea`, baseline `2640e6c9af2568c2731019cab9d16423fd45ceae`. Authorization: AB-START-001, OWNER-001/002; no authority changes. Reviewed AGENTS/gate/approval/Goal/runbook, requirements chapters 06/07/08 and all 27 confirmed decisions, module, tests and four unit records. This review writes only this document; source, locks, index and commits remain author/primary owned.

**Final verdict: the three findings are closed on `9acf86c837217298a29bbd6db6f6c411c3637c47`; no remaining blocking issue found within this pure-contract scope.** Independently executed final 48 integration tests, four separately written control cases, Linux Core all-target Clippy, rustfmt and whitespace checks pass. The original 35 green tests did not cover the initial defects; all original failure evidence remains below. This is a pure validated-contract/admission algorithm, not a Store scheduler, actual model planner, platform Goal, dispatch, TaskAccepted or OS sandbox.

## Final correction review and independent replay

Author source correction `9acf86c837217298a29bbd6db6f6c411c3637c47` is frozen and clean apart from this reviewer-owned document. No reviewer source/lock/index modification or commit. The original 35 tests retain their assertions; receipt and action fixtures were mapped to the new typed API, and 13 new cases were added. Source diffs and author evidence were inspected independently.

- **F1 closed:** ExternalWrite must select a host-approved canonical action with exact Executor, structured operation/recipient/resource set/payload and recomputed digest; it cannot submit another approved flag or arbitrary action name. Host scope checks all connection/program/config/identity/workspace/input/policy bindings, request size and recipients/targets. Duplicate action references and equivalent canonical alias requests are refused. The complete project/root-bound durable action projection includes older known effects and unresolved actions. Process-stopped alone is insufficient: only actual NoEffect reconciliation or a matching never-started fact permits safe retry; confirmed effects cannot redispatch, and unknown/in-flight actions block new admission. The external key uses project/root/action/request digest and remains unchanged across attempt, node/key rename and plan revision. Admission returns the exact approved request to be checked by the future dispatch adapter. This does not prove a receiver implements idempotency or a Provider is physically constrained to that request.
- **F2 closed with the legitimate path retained:** ValidationReceipt carries an exact ResolvedInput source (node producer/attempt/hash, or external artifact/contract/scope/hash), plus current plan/spec/input evidence and approved rule digest. External→Validate, External→Owner and ordinary external consumption are supported. A validator can check the raw approved external input; ordinary consumers require its contract checks. The independent positive path now reaches `required_checks_satisfied: true`, and incorrect source kind, hash, scope or contract is rejected. External/node artifact IDs cannot collide. No early unsupported rejection or change to InternalCompute substitutes for this completion test.
- **F3 closed:** Every actual consumer attempt carries exact non-deserializable consumed-input receipts with consumer node/attempt and current evidence. Admission returns fixed resolved inputs for future atomic persistence. Constructor rejects wrong consumer ownership; historical old producer refs remain representable but freshness comparison makes completed/still-running consumers stale when producer attempt/hash changes. Freshness propagates through dependent consumers, candidates and parents; stale results do not satisfy required checks or authorize downstream work. The independent original A1→B1 then A2/new-hash control now returns false and `ConsumedInputsChanged`, retaining B1's actual old input rather than rebinding it to A2.

The following independent commands used the same coordinated existing Cargo target, under umask 022. Target was explicitly released to Core afterward; binaries/logs are copied under separate `/tmp` names so subsequent worktree builds cannot overwrite the negative-control evidence.

- `cargo test --locked -p agentbaton-core --test goal_plan --offline`: **48 passed**, zero failed/ignored/filtered.
- `rustc --edition 2021 --test /tmp/ab-review-goal-fixed-controls.rs -L dependency=/workspace/.worktrees/validation-integration/target/debug/deps --extern agentbaton_core=/workspace/.worktrees/validation-integration/target/debug/deps/libagentbaton_core-27618ff7f24e49db.rlib --extern agentbaton_providers=/workspace/.worktrees/validation-integration/target/debug/deps/libagentbaton_providers-90ac3a54a35a178c.rlib --extern agentbaton_workspaces=/workspace/.worktrees/validation-integration/target/debug/deps/libagentbaton_workspaces-f26714b52330f5aa.rlib --extern serde_json=/workspace/.worktrees/validation-integration/target/debug/deps/libserde_json-906cb9383affae14.rlib -o /tmp/ab-review-goal-fixed-controls`, then `/tmp/ab-review-goal-fixed-controls --nocapture`: **4 passed**. These retain the original three failure semantics, add external Owner/Work completion, safe no-effect retry/stable key/revision continuity, and wrong typed receipt/consumer identity negatives. Only shared fixture setup is reused; the safety assertions are reviewer-written and are not repository source edits.
- `cargo clippy --locked -p agentbaton-core --all-targets --offline -- -D warnings`: **passed**.
- `rustfmt --edition 2021 --check crates/core/src/goal_plan.rs crates/core/tests/goal_plan.rs` and `git diff --check`: **both passed**.

Final reviewed source fingerprints:

| File | SHA-256 |
| --- | --- |
| `crates/core/src/goal_plan.rs` | `7a6cbb6e56bc565aeae7b790f73200a9756c4fd706bbf8dca4ca6fff40b0cb82` |
| `crates/core/src/lib.rs` | `b1dce56802800f45893405f590f229104bb38e5c3e01efba9049fc2a4ab0d71c` |
| `crates/core/tests/goal_plan.rs` | `8430aa22a3cf76c5d027a0465d1ef4760192e04b482fc4a5a0d88820e037c0e8` |

Final independent evidence:

| Path | SHA-256 |
| --- | --- |
| `/tmp/ab-review-goal-fixed-targeted.log` | `32a88e74438147aa3ad08b2f0e0310f66945a0da851912ff907236ba2e0138da` |
| `/tmp/ab-review-goal-fixed-clippy.log` | `59639588a6f1826f129cbcd121edc07279dab94786f5fa5bde6b3439b2e845cc` |
| `/tmp/ab-review-goal-fixed-fmt.log` | `dd6a032ce3fc9f9078864e2434e9327f89931477d7b7aa6b0898dba07cca3418` |
| `/tmp/ab-review-goal-fixed-controls.rs` | `fa6c6bc5da3cc5d060879fed13fa1adc643713add9ad8a62884359db303b0b96` |
| `/tmp/ab-review-goal-fixed-controls-build.log` | `8361d1e54cea5c0b824662d729ce7f73c3aeb698bc868ed47b3d83de6485fdfb` |
| `/tmp/ab-review-goal-fixed-controls.log` | `bf0cbada985b1549e845044d06eb2703317248cc1e4ed2c097e2252c417c84a7` |
| `/tmp/ab-review-goal-fixed-controls` | `4d9d1d39514b0dc3b61b902037a74c2b81e07c7ca17401955f01bd5d5984c613` |

Both author final correction log hashes (48 tests and all-target Clippy) match EVIDENCE. They remain separate from the independent executions above. No normal or negative-control failure was deleted or rewritten.

## Findings on the initial freeze

1. **F1, P2 — external write rework can repeat a completed effect under a fresh key.** `goal_plan.rs:1558` accepts `Failed { ValidationFailed, ConfirmedStopped }` for an `ExternalWrite`; `:1752` derives a new key from plan digest and next attempt. A stopped process can already have delivered the write. There is no never-started/no-effect, native idempotency or reconciliation proof for this path. Independent control first obtains initial admission, then records exactly this stopped validation failure: a second admission is produced with attempt 2, rework true and a different key. The safety assertion fails. Preserve known before-dispatch/no-effect retry, but block this ambiguous external path pending reconciliation. The key also changes with plan revision, so completed/unknown external action continuity must not be reset by editing a plan or inventing another action key.
2. **F2, P2 — admitted external-input validation cannot represent completion.** The structural validator permits a `Validate` node consuming `ApprovedExternal`; the ready algorithm admits it. After it completes, `TrustedPlanSnapshot::from_verified_host` requires every validation to find a node-produced `ArtifactReceipt` (`:1175`), while artifact receipts themselves must name a declared node output (`:1161`). Therefore no receipt for the approved external input is representable; the legitimate completed receipt is rejected as `StaleEvidence`. The initial review offered either exact typed external receipts or an explicit unsupported guard. Primary subsequently confirmed this is a required legitimate path: retain `ApprovedExternal` → Validate/Owner completion with exact external source/hash/contract/scope receipts; merely rejecting that path does not close this finding. Node-output IDs must not collide with external IDs.
3. **F3, P2 — upstream rework does not invalidate a downstream attempt's old input.** `NodeObservation` and receipts bind the plan/global input revision and producer output attempt, but not each consumer attempt's resolved input producer/attempt/SHA. Independent control completes A and B (B consuming A), then changes A to attempt 2 with a different hash and correct new validation receipts while retaining B's factual old attempt-1 result. All ledger minima are recomputed. The snapshot is accepted and `required_checks_satisfied` remains true, although B never consumed new A. Actual consumed-input evidence must be fixed at admission/attempt creation and checked both for readiness and finished/required checks; otherwise the exact-version guarantee cannot be enforced by this contract layer.

These are local algorithm controls with synthetic trusted-host fixtures. They execute no Provider, network or external write and make no claim about historical CI causes. Constructors intentionally rely on authenticated trusted host records; their public Rust inputs are not signatures. The findings use factual historical host records and identify missing algorithmic checks/representations, not forged Agent authority.

## Independent commands and preserved initial evidence

Environment: Linux, umask 022, existing toolchain; `CARGO_HOME=/workspace/.toolchains/cargo`, `RUSTUP_HOME=/workspace/.toolchains/rustup`, `CARGO_TARGET_DIR=/workspace/.worktrees/validation-integration/target`, build jobs 2. Cargo window coordinated with Core author; released after checks. No new target or dependencies.

- `cargo test --locked -p agentbaton-core --test goal_plan --offline`: **35 passed**, zero failed/ignored; `/tmp/ab-review-goal-targeted.log`, SHA-256 `8a75393084ca6b46c197bbd9ca6399a85705be2e29db1dd130c374967a81f651`.
- `cargo clippy --locked -p agentbaton-core --all-targets --offline -- -D warnings`: **passed**; `/tmp/ab-review-goal-clippy.log`, SHA-256 `bca6666a17f55b5e86d2bfefd9467aa523b2fd99e51cf449e269ac429891a0c3`.
- Independent `/tmp/ab-review-goal-controls.rs` reuses only fixture helpers from the public integration test, with separately written negative assertions; compiled with `rustc --edition 2021 --test` against the just-built exact initial Core/provider/workspace rlibs and existing dependency directory. No repository test/source edits. Initial F1/F2: **0 passed / 2 failed**, exit 101; `/tmp/ab-review-goal-controls-old.log`, SHA-256 `53cd19419f742ab84e66ade70288bccd7bd795695be69217ec4457bacff7838b`.
- Added F3 control, exact filter `reviewer_upstream_rework_must_invalidate_downstream_old_input`: **0 passed / 1 failed**, exit 101, returned `required_checks_satisfied: true`; `/tmp/ab-review-goal-input-version-old.log`, SHA-256 `9f9decaee20f1763237e1d4d39cb0c8ad30e094010ee15fa7ef260ba775721d7`.
- Independent control source SHA-256 after all three controls: `8218bcd86a7643de1c392af05bccd9ad93841f15ceee6a33001b6d30baed2486`. Initial statically linked three-control binary `/tmp/ab-review-goal-controls`, SHA-256 `fef8d34099b12e8d3fb4c4fe69bb49df289f39093381cec7fd6b93df51713a0f`, is preserved even after the shared Cargo target is reused by another worktree. Its later rebuild must use another output path.
- Rehashed all three author log fingerprints listed in initial EVIDENCE (35 targeted, initial Clippy failure, final Clippy): all match. Those are author evidence, separate from independent commands above.

Initial exact source fingerprints:

| File | SHA-256 |
| --- | --- |
| `crates/core/src/goal_plan.rs` | `48d39b9c9b9679f9d412407ffef3bfb146a3343bd39e84d3d9a9504bffe47a3d` |
| `crates/core/src/lib.rs` | `b1dce56802800f45893405f590f229104bb38e5c3e01efba9049fc2a4ab0d71c` |
| `crates/core/tests/goal_plan.rs` | `8f9622d5c342f120dc86d845a27b66fa26e847687c070c04cc1b3dc66800c646` |

## Remaining integration limits

The immutable scope/validated plan/trusted snapshot wrappers do not deserialize Agent authority. The trusted caller must authenticate current grants/rules/receipts and actual physical isolation, and build one consistent journal snapshot. ReadyDecision is tagged with expected plan and ledger revisions and explicitly requires a future atomic Store comparison/reservation/recheck; it reserves nothing. Root/project/subtree quotas use checked arithmetic and unknown token/cost remains unknown under hard caps. Unknown/cancelling current or old-plan root actions block new dispatch. Same workspace serializes even if labeled read-only; distinct roots need exact plan-bound isolation/write proofs, with alias/case/Unicode ambiguity conservative. Worktrees and caller labels are not OS enforcement.

No Store/IPC/UI/scheduler/native Goal/model invocation, actual Provider identity/metering, remote host, macOS native execution, development package or exact-head CI is verified by this review. The author records these limits rather than claiming the full goal complete. Integration must retain atomicity, authority/expiry checks, actual consumed inputs and durable external-action reconciliation, and must not reinterpret partial Integrity/Data validators as whole-task correctness.

Future integration must persist admission, attempt-bound consumed inputs, all cumulative reservations and canonical action intent in one actual compare-and-reserve transaction before dispatch. Host/Provider capability and enforcement checks, recipient/request revalidation, revocation and current time must be rechecked there. Structured payloads are host-private request data, not safe diagnostic/renderer output merely because they serialize. Pure admission results neither stop running workers nor prove actual artifacts/OS containment; their trusted snapshot must be derived from real journals/receipts, not reconstructed from Agent messages.
