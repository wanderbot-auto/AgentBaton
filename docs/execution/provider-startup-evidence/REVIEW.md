# Independent generated-provider startup evidence review

Verdict: **no blocking finding in this test-only evidence change; suitable for integration and fresh exact-head CI**. Reviewed source `f61842cf7d03b3967ceff60a0e6883d0d37dd682`, documentation `fe76c4e157f88a92eb39c3dea23b732ffdd29f15`, independently in `/root/review`. This is not a production performance fix or closure of the original ARM startup timeout.

## Preserved actual failure

PR13 subject `0a0beac38260cc426b4a0887b2bf59937ec9ff90`, Product run `40475194913`, Apple Silicon job `122091591692` failed 18/19 generated native protocol tests. The panic was the old shared startup helper at line 119: `Timeout`, effects `Unknown`, no protocol-error enum. The original full log (SHA-256 `dcd64d5e1475e0acbf3f9042f76db918840d31f735b92d6aa04822d53dd6da0f`) did not identify which of the five fault modes or observed initialization stage caused the failure. The distinct original 4 MiB/500 ms stderr case passed. No historical mode, cold-start explanation, cleanup defect or OS cause is invented by this review.

## Source, safety and semantic review

Mechanical comparison against the subject head proves the only changed source is `crates/providers/tests/native_codex.rs`. The complete generated Python `FIXTURE` bytes remain identical (SHA-256 `62f269de6fdd2e2bd4f163bbfc38b5b5f41513d0eeee3cd18e4977a82c07efb1`). All original 19 test functions and strong assertions remain; one bounded diagnostic regression is added. There are no production Provider/Core/UI/config, dependency/lock, workflow, credential, permission, retry, warm-up, deadline or timeout changes.

The original helper still performs exactly one `ManagedProviderProcess::start` with 500 ms. Success takes the original path. Only an error triggers diagnostic file inspection, after the original start result has returned and its owned process cleanup path has run; no diagnostic result can accept startup, resume it, replay it or extend its protocol deadline. The original fault tests still check unknown effects, secret-error redaction, actual reaping, refusal to replay and the EOF side-effect marker.

Preparation and start are measured separately with monotonic caller clocks. `start_elapsed_us` includes digest verification, spawn, initialize exchange and failure cleanup; it is not initialize-only latency or a claim that an over-budget exchange succeeded. The production initialize deadline is still established later inside `exchange`. Tests do not add a loop, retry or availability assumption to convert failures to success.

The diagnostic opens only the generated fixture directory's existing files and reads at most 4096 wire bytes. It parses only complete newline-terminated records within that prefix. Output consists of fixed local mode literals, typed `FailureKind`/`SideEffectState`/optional real `ProtocolError` enum, booleans, byte sizes, durations and safe counters/stage names. It emits no raw result, stderr, error text, argv, environment value, path, PID value or wire field value. JSON nesting/invalid/incomplete lines remain within the bounded scan; malformed complete lines contribute only a count. The enum types contain no raw payload strings.

The stage names describe **recognized fixture records**. In particular `initialize_response_recorded` proves neither correct protocol content nor stdout flushing/client receipt. Counts are only observations from the bounded prefix, not a complete wire history or authorization/identity proof. The private helper is test-fixture instrumentation, not an API for arbitrary Provider-selected filesystem objects.

The added regression intentionally replaces a local generated script with one that records its PID and sleeps, preserving the original 500 ms start argument. It requires Timeout/Unknown and checks that an observed PID is reaped; an unscheduled child need not have written a PID marker. It then writes recognizable records containing hostile credential/path-like values, malformed lines and a giant incomplete trailing record. Exact counters, 4096 scanned bytes, truncation and raw-secret/path absence are asserted. No real vendor, account, model, network or production operation is involved.

## Independent commands and actual results

Only the author's released private target `/workspace/.worktrees/provider-startup-evidence/target` was used, after native verification had completed and its target had been formally released. Main and native Cargo targets were not accessed here. Incremental was disabled, jobs 2 and umask 022. `RUST_TEST_THREADS=1` matches the current CI fixture condition; all case-local deadlines and explicit concurrency tests remain intact.

| Actual command | Result |
| --- | --- |
| `cargo test --locked --offline -p agentbaton-providers --test native_codex -- --show-output` | 20 passed, 0 failed/ignored; 70.14 seconds |
| `cargo clippy --locked --offline -p agentbaton-providers --all-targets -- -D warnings` | passed |
| `cargo fmt --package agentbaton-providers -- --check` | passed |
| `git diff --check 0a0beac38260cc426b4a0887b2bf59937ec9ff90..f61842cf7d03b3967ceff60a0e6883d0d37dd682` | passed |

The intentional delay test actually recorded its local child PID; reaping and safe diagnostic checks passed. Its observed diagnostic was `fixture_pid_recorded`, initialize counters zero, `Timeout/Unknown`, `start_elapsed_us=1427835`, with no wire file. That delay is an intentional negative control, **not reproduction of ARM's original failure**. The original fault test and strict 4 MiB/500 ms stderr test also actually passed locally. No raw secret fixture text was emitted by the diagnostic.

Reviewed test source SHA-256: `aff6e9cac154f7877b6f0a969d4cadaaa7e32c2a28638fcb9546b088d7b1c8e3`.

Independent evidence directory: `/workspace/.onboarding/provider-startup-evidence-review/`; verification manifest SHA-256 `d5ce107f7d275e07ff22ef0f23bc38468c068d884158a6e9748fd65e1c57eb14`, binding exact commands, actual test/lint logs and mechanical source/fixture preservation proof. The Provider target was formally released after the last check; there are no further Cargo writers from this review. Reviewer changes are confined to this untracked report and ignored evidence, with no source/locks/index/commit/push/CI/merge action.

## Unresolved dimension

The original ARM initialize timeout was **not reproduced and its cause remains unknown**. This change improves failure evidence without fixing or concealing that unknown. A fresh exact-head CI must still pass all required checks; if it fails, its new diagnostic must be read rather than relaxing 500 ms/4 MiB, skipping the case or claiming that local green proves a Mac fix. macOS runtime/GUI, actual model/provider/account compatibility, user installation, minimum OS, signing and full Goal/product completion are not established by these Linux generated-process tests.
