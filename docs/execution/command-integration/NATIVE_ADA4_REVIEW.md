# Independent CI review — ada4 failed on both macOS architectures

**Verdict: ada4 is not accepted; Product CI failed.** The Mac serial-fixture setting was actually applied, but it did not make the unchanged 1000 ms lifecycle path pass. No merge, artifact substitution or threshold relaxation is justified. Reviewer `/root/review` used actual GitHub API results and completed job logs only; no Cargo, source/lock/index edits, push, GitHub comment or merge.

## Exact provenance and completed boundary

PR12 head `ada4c81213fcd3e9fed80c86cd33581415b12890`. Both runs are pull_request events with exactly that API head_sha. Final API read at **2026-10-27 04:55:15 UTC**:

| Check | ID | Actual result |
| --- | --- | --- |
| Product run | `40451447529` | completed / failure |
| Environment run | `40451447566` | completed / success |
| Product / Rust | `122014868031` | success |
| Product / UI | `122014868333` | success |
| Product / macos-15-intel | `122014868271` | failure, Core and native host tests |
| Product / macos-15 | `122014868483` | failure, Core and native host tests |
| Environment / macos-15 | `122014868253` | success |
| Environment / ubuntu-24.04 | `122014868499` | success |
| Environment / macos-15-intel | `122014868580` | success |

**Both completed Mac logs record actual checkout `6758d6d0c3b19ed6323a4ed412771fb4d269b088`**, merging ada4 into protected main `66061439c79fa44b66a33782febc1fb0f31c73df`. This differs from PR head as expected for GitHub's pull-request merge ref. Product artifacts also use this actual merge SHA. Read-only `git diff 201fba9..ada4c81 -- crates apps scripts` is empty: product/test source is unchanged from the independently reviewed fixture. The only workflow change is the previously reviewed Mac-step `RUST_TEST_THREADS: '1'`; other changes preserve evidence/documents.

## Both failures: precise phase, original thresholds retained

| Observation | Apple Silicon / macos-15 | Intel / macos-15-intel |
| --- | --- | --- |
| Actual serial environment | log explicitly `RUST_TEST_THREADS: 1` | log explicitly `RUST_TEST_THREADS: 1` |
| Core command integration suite | **13 passed / 1 failed**, 128.12 s | **13 passed / 1 failed**, 121.76 s |
| Fresh grant remaining at response | 999 ms | 997 ms |
| Grant-response → admission request | 0 ms | 0 ms |
| `sessions.create` RPC itself | **1331 ms** | **1199 ms** |
| Real expired negative control | rejected after 1124 ms, PERMISSION_DENIED/not_started | rejected after 1141 ms, PERMISSION_DENIED/not_started |
| Failing assertion | command_validation.rs:1096: task cancelled vs expected outcome_unknown | same exact case/line/state mismatch |

The failing case on both is `actual_reaped_failed_acp_lifecycle_can_approve_independent_command_while_task_result_stays_unknown`. It gets a fresh unchanged **1000 ms** grant and immediately calls admission, but the admission RPC itself exceeds that window. The assertion fails at task state before the subsequent last_error, initialize/session-new marker, authoritative reaped-receipt and independent-command assertions are reached. Those later assertions are not counted as observed success.

`committed_revoke_and_task_cancel_never_become_not_started_errors_from_owned_record_corruption` passes on both in this run. The earlier 201f second-iteration marker timeout is not a current failure and is not misreported as cleanup hanging. All 1 s / 2 s / 3 s assertions and 1000 ms grant remain unchanged. The intervention demonstrates that serializing unrelated fixtures is **not sufficient** to resolve this lifecycle path; it does not identify the internal admission hotspot or prove a sole contention cause. Next work requires real phase profiling and a safety-preserving production optimization, not changing budgets or accepting a different lifecycle.

## Downstream evidence was not produced

Both jobs stop during workspace tests, before the subsequent native-host **11**, feature-harness **6**, actual WebView/daemon acceptance, Command3+Data3 read-only flags, PNG capture, default release executable audit and development packaging stages. No actual native report/PNG or default-release audit result from ada4 was verified.

The completed run's artifacts API contains only `frontend-real-ipc-6758d6d0c3b19ed6323a4ed412771fb4d269b088` and `provider-contract-6758d6d0c3b19ed6323a4ed412771fb4d269b088`. It contains **zero native-webview or development artifacts**. Consequently no paired native ZIP/API-digest/safe-extraction audit or dual-DMG verification was attempted; the prepared ignored audit helper was not run on fabricated/borrowed data. Earlier 201f, 7ba or other native/package results were not substituted. Browser/schema evidence is separate from macOS native evidence.

## Preserved independent evidence

Files under ignored `/workspace/.onboarding/command-native-ada4/`; SHA-256 recomputed locally:

| File | SHA-256 |
| --- | --- |
| `product-run-final.json` | `6b9bcc2a53e4da2ef6e52ef048dbf412e05b226ca3782c9164462ed2906d3fd1` |
| `env-run-final.json` | `7bb09fc577252f542649270c2e58f3af38653c9fc1119d25327a6420c5dfee27` |
| `product-jobs-boundary2.json` | `dff316a1172cb84a9059869c3384b741ee6cb9576fd64467ac19145462b1bb9e` |
| `env-jobs-boundary1.json` | `042d8673df7ac52cea67593e9157364de51629eda78efe21294f156cfd5f4290` |
| `artifacts-final.json` | `75e90d23b14613295d0e094746aa7981b6d1e135d28a2eea2b0ca1746e2f9732` |
| `arm-job-122014868483.log` | `86134be1b6f736a84a0eae3ac194abafc46de4efbb40196cf953a36e673ff4a6` |
| `intel-job-122014868271.log` | `16f999b823055fd7167550032fcd0a23f7ff0ffb59695ba0575e2286e2b70503` |

Actual read commands: `gh api repos/wanderbot-auto/AgentBaton/actions/runs/<run>`; `/actions/runs/<run>/jobs?per_page=100`; `/actions/jobs/<job>/logs`; `/actions/runs/40451447529/artifacts?per_page=100`; read-only Git source diff and Python JSON/hash inspection. Polling used bounded completion observations, not frequent retry of unchanged failures. All raw failures remain. No private Provider/model/account, signing, production or added-resource operation was performed. Admission optimization and its later exact-head CI require a separate independent source review and report; this failure report closes no product requirement.
