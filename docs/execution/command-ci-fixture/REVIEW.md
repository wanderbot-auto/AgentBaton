# Independent review — PR12 command lifecycle fixture

Reviewer: independent native `/root/review`; read-only product/source/locks/index. Base: `7ba60e0822129d474be1ade845640d1d72e79b21`. Authorization remains AB-START-001 and OWNER-001/002; no Provider/model/account, production or paid-resource activity. Scope is a fixture-only correction retaining actual generated-process stop → separately approved command validation, while preserving grant expiry and task-result uncertainty.

**Final verdict: independent fixture review passes on `b1438a9c6e97cb45e155c443a084ece81453cb8f`; no remaining blocking issue found in this narrow change.** Old-order failure was independently reproduced; final 14-case integration suite, Core all-target Clippy, format and whitespace checks pass. New exact-head macOS CI remains required, and no historical Intel phase cause or fresh native/package success is claimed.

## Observed original CI evidence

Primary supplied the actual completed Intel Product job `121999784372` in run `40446762573`. The raw log records checkout of actual merge `56ff4227f2bae71fdbf377fbd07c3af7bcd6e403` (PR head `7ba60e0822129d474be1ade845640d1d72e79b21` into `66061439c79fa44b66a33782febc1fb0f31c73df`). The Core command integration binary reports 13 passed / 1 failed, and the failing case is `actual_reaped_failed_acp_lifecycle_can_approve_independent_command_while_task_result_stays_unknown`. Its generic success-call assertion at old line 955 receives `PERMISSION_DENIED`, “Execution grant or an ancestor is revoked or expired”, `side_effect_state=not_started`, revision 17. This is an authority rejection, not a command acceptance assertion failure.

Raw log: `/workspace/.onboarding/command-7ba-intel-job-121999784372.log`, SHA-256 `7bb109695acb3fbf3018eb4f793dad9fd806a23294ea6c5f4d57152bc99b76e3`.

The initial exact test file SHA-256 is `272a2dfa2b6a038809ef4d6f23fe422c7a4b6a78c2861280ece8eb4d9b1945cc`. Static inspection confirms the fixture finalizes a 1000 ms execution grant, then creates the task, then requests session admission. Production computes expiry when creating the grant, and session admission checks its current lineage before starting an attempt. The original generic assertion did not identify the RPC method or elapsed phase, so the old log alone does not establish how much time each phase took, or prove that reordering removes all possible macOS latency. Controlled delayed preparation and fresh exact-head CI are required.

## Review requirements

Retain the original 1000 ms grant, all three-second waits and all lifecycle assertions. Move unrelated task/preparation work before final grant approval, not into the grant's active window. Diagnostic method/phase timing must identify whether any future failure occurs before admission or within admission itself. A real 1100 ms preparation delay should demonstrate the old ordering expires and the fresh ordering preserves the original path. Separately wait out a real 1000 ms grant and prove admission refuses it with no session/attempt/dispatch/lease side effect. Production expiry, clocks, permissions and timeouts must remain unchanged. Neither larger budgets nor ready/stopped-only assertions can substitute for real generated process ownership/stop evidence and the independent command result remaining distinct from TaskAccepted.

Cargo execution was deferred until Primary explicitly granted a short exclusive window after GoalStore released it; final independent evidence is below. The target was explicitly released back to Primary afterward.

## Frozen change and independent static verdict

Exact freeze: `b1438a9c6e97cb45e155c443a084ece81453cb8f`. Final `crates/core/tests/command_validation.rs` SHA-256: `5525e892f99ece9fba21df7f9b1ff05b8cc996f5f83bfd880ff7732a05a6e4db`. `git diff --name-only 7ba60e0..HEAD` contains only that test and author's PLAN/PROGRESS/CHECKPOINT/EVIDENCE documents. All production libraries, schemas, permissions, locks, workflow checks, expiry and runtime code are byte-unchanged. No source/index edits by this reviewer.

The same real 1100 ms task-preparation delay now runs before grant proposal/final approval. An explicitly expired 1000 ms grant is tested first after 1100 ms real waiting; refusal must be `PERMISSION_DENIED`/`not_started`, with zero session and attempt rows for this task, unchanged lease holder/fencing epoch, and no ACP startup marker. Then a separately proposed/finalized fresh 1000 ms grant is used immediately for the original session. Neither budget is 10000 ms and none of the original three-second waits is raised or skipped.

Coverage is strengthened, rather than changed to merely observe ready/stopped: the generated account-free executable writes only actual `initialize` and `session/new` requests, the final marker must be exactly those two lines, session last_error must be `session_creation_failed`, and its trusted journal receipt must say `process_reaped=true`, `no_process_started=false`, `reason=session_creation_failed`, `task_accepted=false`. Original stopped intent, cleared lease, task `outcome_unknown`, separately owner-approved actual validator script `command_passed`, `task_acceptance=not_established`, and final task still unknown all remain. This distinguishes a malformed ACP process actually started/reaped from budget expiry without a process or acceptance inferred from a generic stopped label.

The fresh SQLite schema8 assertion protects local cross-worktree cache correctness; deliberate integration into a later schema must update that fixture sanity expectation with the actual reviewed migration. It changes no production schema. Per-method prep/RPC/total timing and grant remaining TTL are diagnostics; they do not excuse slow admission or reinterpret failures as success.

All eight hashes in author EVIDENCE were independently recomputed and match, including preserved compile-type and journal-column fixture mistakes, original old-order failure, saved old test binary, final positive/negative run, 14-case run, Clippy and format logs. Those are author executions, not this reviewer's tests.

## Independent old-order replay

Before execution, the saved Linux harness `artifacts/command-ci-fixture/old-order-control` matched SHA-256 `c0ecfd248792f6e81a06affa2dd9acf51409881fc49565f7ae0987984070a144`. Under umask 022 this reviewer ran:

```sh
artifacts/command-ci-fixture/old-order-control --exact actual_reaped_failed_acp_lifecycle_can_approve_independent_command_while_task_result_stays_unknown --nocapture
```

**Expected failure reproduced: exit 101, 0 passed / 1 failed, 13 filtered, 2.40 s.** Task preparation 1100 ms plus task RPC 8 ms consumed 1108 ms after approval; `sessions.create` itself returned in 10 ms with expired-grant `PERMISSION_DENIED`/`not_started`, revision 17. This directly proves the delayed old fixture ordering can consume the short grant. It does not establish the exact uninstrumented historical Intel phase latency.

Independent raw replay `/tmp/ab-review-command-ci-old-control.log`, SHA-256 `d18b0c09d7efaee7959b30bee47e4bc333f11435e307c70f27b38a2291846ed1`. No source reversal, new Cargo target or live Provider/account/model was used. The binary is a Linux regression test harness, not a desktop or macOS delivery artifact.

## Independent final execution

Linux managed environment, umask 022, existing toolchain/cache only:

```sh
export CARGO_HOME=/workspace/.toolchains/cargo
export RUSTUP_HOME=/workspace/.toolchains/rustup
export CARGO_TARGET_DIR=/workspace/.worktrees/validation-integration/target
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2
export PATH="$CARGO_HOME/bin:$PATH"
cargo test --locked -p agentbaton-core --test command_validation --offline -- --nocapture
cargo clippy --locked -p agentbaton-core --all-targets --offline -- -D warnings
cargo fmt --all -- --check
git diff --check 7ba60e0..HEAD
```

Actual final integration result: **14 passed / 0 failed / 0 ignored / 0 filtered, 9.18 s**. This includes the independently rerun fixed lifecycle, its real expired negative and all existing command security/recovery cases. The fixed lifecycle recorded:

- Task preparation 1100 ms and task RPC 7 ms occur **before** final fresh approval.
- Expired control waits 1100 ms after its unchanged 1000 ms grant; admission rejects in 7 ms with `PERMISSION_DENIED`/`not_started`. No session/attempt row, lease change or generated ACP startup occurs (all actual assertions pass).
- Fresh final approval returns with **998 ms** remaining; grant-return→admission-request is **0 ms**; actual session admission is **223 ms**. This local admission is within the original 1000 ms budget.
- Actual generated initialize→session/new, malformed-start failure, authoritative owned-process reap, stopped intent, released lease, still-unknown task, independently approved real script command_passed, and no TaskAccepted all pass the strengthened assertions.

Core all-target Clippy with `-D warnings`: **passed**, 2.43 s. `cargo fmt --all -- --check` and baseline-to-freeze `git diff --check`: **passed**. No whole-Core repeat, new target, source reversal or Provider/model/native GUI call was made by this reviewer.

At Primary's explicit instruction, only this worktree's `crates/core/src/lib.rs` mtime was touched before Cargo to force the exact schema8 library rebuild rather than reuse schema9 metadata from the shared target. File bytes before/after both hash `b944881ec6353c80eca87994a2bb5e5edc569ebbc51fc2a2071330a01043a547`; runtime schema8 fixture assertions and final test-source fingerprint pass. Git status after tests contains only this untracked reviewer document. No production content, lock, index or commit was modified, and the shared Cargo target was explicitly released.

Independent logs:

| Path | SHA-256 |
| --- | --- |
| `/tmp/ab-review-command-ci-old-control.log` | `d18b0c09d7efaee7959b30bee47e4bc333f11435e307c70f27b38a2291846ed1` |
| `/tmp/ab-review-command-ci-final14.log` | `5aa839957823e50da08c71d860fcf3e30ce1e8a3601b4ac91069c7f18a7ba4f2` |
| `/tmp/ab-review-command-ci-final-clippy.log` | `83b24d2e5d683d690c820c28e5934b3034bd849d58f411da32218a3ab57d4c13` |
| `/tmp/ab-review-command-ci-final-fmt.log` | `88492d9c5afc4b945db9939ef8248d402dd40fe8872003553c56a17ae29d4dbb` |

## Remaining limits

The controlled Linux old/new comparison proves the fixture's preparation-order mechanism and preserves actual expiry rejection. The old Intel log had no per-method timing, so neither these controls nor a Linux pass establish that this was the entire historical Mac latency cause. In particular, a future fresh finalization or admission consuming the original 1000 ms remains a real failure; it must be diagnosed from the new phase/remaining-TTL evidence, never hidden by extending timeout or accepting a different lifecycle reason.

This review supports preparing the next exact-head PR12 CI run. Intel and Apple Silicon required jobs, actual native GUI/owner approval, real vendor/model/accounts and traceable new development artifacts require their own evidence. The generated ACP fixture, Rust Unix IPC and Linux command execution do not establish those dimensions or complete the overall product goal. Existing product/Environment checks and release/spend/credential gates are unchanged.
