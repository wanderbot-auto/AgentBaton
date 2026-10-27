# Independent review: failed 201f candidate and CI resource isolation

**201f is not accepted. The proposed four-line CI resource isolation is reasonable for a new exact-head verification; it does not establish a historical root cause or a new macOS pass.** Independent reviewer `/root/review` read source, actual GitHub APIs and the completed Intel job log. No Cargo, product/source/lock/index changes, GitHub comments, push or merge were performed. Only this report and ignored evidence were written. Original provider-runtime worktree files were preserved.

## Exact observed candidate

PR12 head `201fba91597d475d4ecce0c64f15106e17382d27`; Product run `40450221736`; Environment run `40450221768`. Both API head_sha values match 201f and event is pull_request. Intel checkout log confirms actual merge `76d7503e1b834bca70a5abbb397efd25c5a796f7` of 201f into protected main `66061439c79fa44b66a33782febc1fb0f31c73df`.

Last API observation: **2026-10-27 04:42:17 UTC**.

| Job | Actual observed result |
| --- | --- |
| Product / macos-15-intel, `122010945047` | Failure: Core and native host tests |
| Product / Rust, `122010945186` | Success |
| Product / UI, `122010945218` | Success |
| Product / macos-15, `122010945256` | In progress; core/native-host and actual WebView steps report success; default application build in progress, release audit/package still pending |
| Environment / macos-15, `122010944743` | Success |
| Environment / ubuntu-24.04, `122010944943` | Success |
| Environment / macos-15-intel, `122010944987` | Success |

ARM step success above is API metadata only: this reviewer did not download/inspect its native report or PNG, and does not infer precise test counts from the step label. Intel failed before its native host 11, harness 6, actual WebView/Command3+Data3 read-only report/PNG, default release audit and development packaging stages could complete. Its command integration suite is **12 passed / 2 failed**, not 14 passed. No paired native ZIP/API-digest/safe-extraction validation or dual-DMG verification was performed because the dual-Mac success prerequisite failed. Old 7ba ARM artifacts and old development packages were not substituted. A normal later push may cancel the still-running old ARM job; this bounded observation does not assert its future result.

## Actual Intel failures and correction of preliminary phase report

1. `actual_reaped_failed_acp_lifecycle_can_approve_independent_command_while_task_result_stays_unknown`, line 1096: task was `cancelled`, expected `outcome_unknown`. Preparation had moved before grant as intended. Fresh grant returned with **992 ms** remaining and request followed in **0 ms**, but **sessions.create itself took 1232 ms**, exceeding the unchanged 1000 ms budget. This is no longer the old post-grant task-setup delay. The original expired negative also correctly rejected after 1146 ms. The log does not include the later strong ACP marker/reaped assertions because it fails at task state first; actual malformed-process-start coverage must not be inferred.
2. `committed_revoke_and_task_cancel_never_become_not_started_errors_from_owned_record_corruption`, line 1177: **second operation, revoke, timed out waiting one second for the script startup marker**. The preceding `CONTROL task_cancel committed=true` is printed only after that first iteration joins the actor and passes its earlier assertions. The reviewer initially mapped the line to the wrong old-source phase and reported a two-second cleanup wait; that preliminary report was incorrect and has been explicitly corrected to Primary and the author. The actual two-second thread-finished wait is at lines 1202–1204. This log does not prove cleanup hung.

The original full job log is preserved unchanged at `/workspace/.onboarding/command-native-201f/intel-job-122010945047.log`, SHA-256 `d8db5e19561f2af8c2b82df36eda538b8614dac49944efbde62d3e4847fbfa6d`. Fourteen independent real Git/SQLite/owned-process fixtures competing for runner resources is a plausible hypothesis, not established causation. No 1000 ms / 1 s / 2 s / 3 s threshold may be raised or an assertion weakened to conceal these results.

## Four-line CI-only proposal: static approval for verification

Independently read the exact uncommitted `.github/workflows/product.yml` diff: only the Mac matrix **Core and native host tests** step gains `env`, two explanatory comment lines and `RUST_TEST_THREADS: '1'`. A Python byte comparison verifies deleting precisely those four lines reproduces the complete 201f workflow; PyYAML parsing verifies the value is scoped to that step, the Linux Rust job has no such environment override, and the desktop timeout remains 30 minutes. All Cargo commands, test inventory, per-test assertions/deadlines, four Product jobs and three Environment checks remain unchanged. No product implementation or permissions change.

**Verdict: reasonable independent-fixture resource isolation, not a skipped test or relaxed per-test acceptance standard.** Test-internal real daemon/worker threads, cancellation under slow clients, fences/replay and explicit concurrency cases remain. Linux Rust still runs with default harness parallelism. Mac cross-fixture concurrent stress is not established by a future serialized pass; record that limitation rather than claiming a general concurrency performance result. The next exact-head dual-Mac CI must test this hypothesis; this static review is not its success evidence.

Proposed workflow SHA-256: `5ec67f4e94d169d700315b856e9c9010caeff15f092e55ad926175e282583e6e`. Saved diff `/workspace/.onboarding/command-native-201f/proposed-resource-fix.diff`, SHA-256 `58b993472aec7d56d1dad18229fd1967ad40e6fc3f7a8670ce3b3d0dc6ffb3a9`.

Primary separately ran a frozen **Linux schema9/6c** command test binary with `--test-threads=1`: 14 passed in 28.75 s, fresh TTL999/request0/admission227 ms. Reviewer read its final result and verified log SHA-256 `8a73945dbea15c8dbc95f24a4939d64350e94bea2ca81de2d12d438ebe6f52ee`, path `/workspace/.onboarding/command-serial-frozen-schema9-control.log`. This is Primary's control, not reviewer execution, and not a schema8/201f or macOS proof. Its default-parallel Linux pass likewise does not establish the cause of the Intel failures.

## Goal integration content check

Read-only composition audit of `6c023f0f3af78898a2b43a65162231e7913e80ba`: merge `810856edf720f384bb7494530897bddb5f67ff86` has parents 201f and frozen Goal `b43b396912527906a83698ab710d6a13303d9342`. Core source trees are byte-identical to b43, Git tree ID `b68c318bef7f8771c4adf57a3a20d3e8cae65c8f`; storage and all migration tests retain b43 content. The only Core test difference from b43 is the retained reviewed b143 command fixture. An exact full-file replacement assertion verifies b143 fixture → 6c differs solely by schema assertion **8→9** and its corresponding diagnostic text; all timing and strengthened lifecycle assertions remain.

Reviewed b143 fixture SHA-256 `5525e892f99ece9fba21df7f9b1ff05b8cc996f5f83bfd880ff7732a05a6e4db`; composed schema9 fixture SHA-256 `83aa18cf1b4a639c3c082798dfcac08ac358ff600ee880d2905246faa79c1960`. Later `ae871ca` adds seven evidence/review documents only; no source changes from 6c. **No source loss found in these conflict resolutions.** This is a content audit, not independent execution or full acceptance of GoalStore/history behavior; Primary's remaining empty/stale history work is not silently closed.

## Preserved API observations

All files are under ignored `/workspace/.onboarding/command-native-201f/`:

| File | SHA-256 |
| --- | --- |
| product-run-initial.json | a01ce83369cbe83b9c19f6ad3e3b8e29a9cb5875bfeba3bae076b7e4da41210c |
| env-run-initial.json | 53408bad068a0ef97f73f2ae191ab7ec4b0559bc937adbadeaff75fde99e02b2 |
| product-jobs-final-observation.json | 887f6e19aa559baadb3976fec4e6e994953dbe74728512e5780cd7a8606b6ec2 |
| env-jobs-final-observation.json | 6929a56d5f26c2011e9492c40d18667c3b7aacd52350c5d503019a65b404da7e |

Actual commands: `gh api repos/wanderbot-auto/AgentBaton/actions/runs/<run>`; `gh api repos/wanderbot-auto/AgentBaton/actions/runs/<run>/jobs?per_page=100`; `gh api repos/wanderbot-auto/AgentBaton/actions/jobs/122010945047/logs`; read-only Git blob/diff comparisons and Python hash/YAML checks. No real vendor/model/account/remote host/signing operation, artifact execution, Cargo or merge occurred in this review. New-head results require a separate report, with actual checkout binding and independently verified native/package artifacts.
