# Independent approval outcome review

Verdict: **no blocking finding in the narrow fix at source commit `32b2dc58d23205350aa54878463b8f824997dc02`, based on `b9942d319b70dab22bf723d9f03230ebae08689a`.** The later `83f0d88` adds implementation documentation only; reviewed source hashes remain unchanged. The reviewer changed only this document and ignored independent logs; no source, lock, index, commit, push or merge was performed.

The confirmed bug is specific: an authenticated `approvals.finalize` can durably commit the connection, consumed proposal/result and confirmation journal, then return a synchronous probe-scheduling error with the default `side_effect_state=not_started`. The preserved original run actually failed that assertion, 2 tests passed/1 failed, exit 101. It records PERMISSION_DENIED/not_started while the original connection/key and one confirmation already exist and the generated executable has not run. This is sufficient evidence for the error-semantics correction; no historical CI cause is inferred.

The two confirmation-journal trigger controls are a different case. Their baseline already returns INTERNAL_ERROR/unknown through `db_error`. They passed before the fix and remain unchanged; this review does not describe them as an old not_started bug. After removing each trigger, retrying the same proposal preserves the original connection/grant and idempotency key, creates no new owner proposal/authorization, and writes confirmation exactly once.

The production diff is one IPC hunk after `Store::finalize_proposal` succeeds. It preserves the original code/retryable/message reason, sets confirmed for the already recorded approval, identifies the existing connection in `details_ref`, and requires reconciliation with the same proposal. It adds no retry, grant, DB write, permission, schema or process launch. Errors returned by finalization itself and the pre-effect NOT_FOUND/not_started case retain their original semantics.

Every current synchronous `Supervisor::probe` error exit was checked: connection lookup/decode, four-probe limit, private environment preparation/check, binary-digest parsing and worker-thread creation failure. All occur before a successful worker spawn. Following successful spawn, only insertion of the handle and `Ok(())` remain. Existing active/completed probes return Ok. Errors produced asynchronously inside `probe_version` are recorded later by tick; they do not enter this mapping. Thus “connection approval confirmed, probe not scheduled” does not misrepresent an already running probe. Environment preparation may have created private directories before refusal; the message makes no claim of globally absent filesystem effects.

The new actual Unix-IPC test creates a private non-directory at the environment namespace to refuse scheduling before execution. It asserts durable connection/consumed proposal/confirmation, pending probe, unchanged namespace, unchanged original error code, confirmed state and connection reference, identical same-proposal retry error, one connection/proposal/confirmation and a never-run executable marker. Direct Store reconciliation after daemon shutdown returns the original ID. It does not authenticate an actual native owner click or prove the environment repaired/probe subsequently ran. Resource-exhaustion/thread-spawn failures were source-reviewed, not separately fault-forced.

| Reviewed file | SHA-256 |
| --- | --- |
| `crates/core/src/ipc.rs` | `8d12f952c123d209a7d70e5380bea9aa212908aea65981bbead67dce86ef9025` |
| `crates/core/tests/approval_outcomes.rs` | `01e8bec8066dfed63d2f113709d01dc295682396d39851f33410910020da05e8` |

The evidence JSON source/log references were independently rehashed. `sessions.rs`, `storage.rs`, `policy.rs`, root Cargo.lock and native Cargo.lock are byte-identical to b994. The whole source diff is 13 added/1 removed IPC lines and the new 338-line test file. Implementation EVIDENCE.json SHA-256 at inspection: `b387318625e27ea8f5d6d13d1e081866b9c425acd676a5b6d5377473e90430c5`.

Independent commands ran under Linux, `umask 022`, with `CARGO_HOME=/workspace/.toolchains/cargo`, `RUSTUP_HOME=/workspace/.toolchains/rustup`, its Cargo bin on PATH, `CARGO_BUILD_JOBS=2` and `CARGO_TARGET_DIR=/workspace/.toolchains/quiet-canvas-target`. The active DataCore target was not used.

| Actual independent command | Result | Log SHA-256 |
| --- | --- | --- |
| `cargo test --locked -p agentbaton-core --test approval_outcomes -- --nocapture` | PASS, 3 tests; zero failed/ignored/filtered | `8b0d4f2ec3e4d7afb96fe30fe303beefcdb4f41a95e51ead5b13474c463efa19` |
| `cargo fmt --all -- --check` | PASS, exit 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo clippy --locked -p agentbaton-core --all-targets -- -D warnings` | PASS, exit 0 | `59639588a6f1826f129cbcd121edc07279dab94786f5fa5bde6b3439b2e845cc` |

Logs are `/workspace/.onboarding/approval-outcome-guard/independent-{tests,fmt,clippy}.log`. Source hashes were checked again after the commands and matched. `git diff --check` passed. The implementer's full-Core log was read and its actual 83 top-level tests counted, excluding the extra nested FIFO child; that full run was not repeated or claimed as an independent full run here.

| Preserved implementer evidence | SHA-256 |
| --- | --- |
| `ab-approval-outcomes-before.log` — actual baseline failure and two already-safe controls | `90465795bc3d5f2608813b2616c013a12f14dbaa08bd86c854e5f3020c4d6299` |
| `ab-approval-outcomes-after.log` — 3 pass | `b18d1a20a61cea0f75a61e139608f008df08863c77d831bc043ecb116c467fb8` |
| `ab-approval-outcomes-core.log` — 83 top-level pass | `52f65709578b15fe7f0ed6fb056a6c38e9a9815ede991eded95aab39d8b7f843` |

No real Provider/model/account, native dialog, macOS app, installer or production action was tested. The generated executable never ran in these three tests. The earlier b994 seven successful CI jobs/native artifacts do not validate this new source or a later Root/PR10/Data composition. Integrate with exact-source regression/review and fresh accurate CI before claiming those candidates accepted; full Design v0.2 remains incomplete.

## Independent CI fixture follow-up — 8441fe6

Reviewed `8441fe6e509283c904e70ffc0a65767e3ffe5412` against the documented `83f0d88` source. No blocking finding in this test-only portability correction. The only changed source file is `crates/core/tests/approval_outcomes.rs`: the generated executable helper returns `path.canonicalize().unwrap()` after creating/chmodding the same file, and the existing connection control now supplies a controlled selected-root symlink (with an assertion that it differs from its canonical path). This exercises aliases on Linux as well as Darwin's usual `/var` alias. No production canonical-path guard, permission, outcome assertion, test count, timeout or other threshold was relaxed. IPC remains SHA-256 `8d12f952c123d209a7d70e5380bea9aa212908aea65981bbead67dce86ef9025`; the new test SHA-256 is `dbc6adc0de7e2c086e31a022f52df374cf7fc3c7c6d3f71e1ccc53e3acf96a67`.

The reviewer read the actual ARM failure log for Product run `40421813961`, job `121917839562`, at `/workspace/.onboarding/provider-2da7-job-121917839562.log`. Its actual checkout is synthetic PR8 merge `ed0dda05d9813f5db8b7cb180aa2e9bb4c51cbef` (2da7da2 into c4e72c6), not the raw head checkout. All three new tests failed before their intended outcomes, with INVALID_REQUEST/canonical executable rejection, and the job exited 101. This failure is retained; it is not evidence that the production outcome mapping regressed. The separately saved Linux `alias-before.log` adds the controlled alias while retaining the old helper and actually fails 1 test/exit 101 with the same canonical-selection rejection. It supports the path-alias mechanism; it does not by itself prove the precise Darwin path selected in that earlier CI.

Independent reruns used the same Linux `umask 022`/toolchain/quiet-canvas-target setup as above, with separate new logs so earlier evidence remains unchanged:

| Actual command | Result | Log under `/workspace/.onboarding/approval-outcome-guard/` | SHA-256 |
| --- | --- | --- | --- |
| `cargo test --locked -p agentbaton-core --test approval_outcomes -- --nocapture` | 3 pass, zero failed/ignored/filtered | `independent-alias-tests.log` | `cdfb548d4ee7c3a476eafab08633c66a21aaf24ecf4c9e4a6d5f71a794394ef5` |
| `cargo fmt --all -- --check` | exit 0 | `independent-alias-fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo clippy --locked -p agentbaton-core --all-targets -- -D warnings` | exit 0 | `independent-alias-clippy.log` | `8c233a54790c4800d3dd585436af679aa838da259bfdb129412e2d51cdb75cdc` |

The three tests still verify the two original unknown controls, the post-approval confirmed probe refusal, original IDs/keys and single confirmation, pre-effect refusal and no generated-executable execution. Final source hashes matched after testing and `git diff --check` passed. The implementer's alias-before log SHA-256 is `1f29a82cf941858aaca30d9e29e353413fa2d0e8de7e4c557ae9edc11376de24`; alias-after (3 pass) is `44ffa4db2b000059a3513567320b2481a613e1a02b0e7a136af546d2bbf5e575`. The original ARM job log SHA-256 is `a0d6729503530ef816413cb9ebe0614c6bcb2533a8c52b35bebc14e93914cc9b`.

No macOS rerun result, Intel failure log, new native screenshot or fresh seven-check success was inspected/claimed in this follow-up. The primary must run accurate CI on the actual integrated candidate. The earlier b994 native success and this Linux alias regression cannot be inherited as a passing Mac result for the new fixture/source. The reviewer appended only this section, without staging or modifying source/locks.
