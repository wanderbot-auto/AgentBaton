# Independent e373 review — macOS native evidence passes; Linux CI remains failed

**e373 is not accepted for merge: Product CI failed on Linux.** Both macOS jobs and their exact-checkout native artifacts were independently verified. The proposed two-line Linux Rust-step fixture isolation is reasonable without reducing per-test acceptance; it still requires a new exact-head full CI run. Reviewer `/root/review` performed read-only API/source/log/artifact inspection, with no Cargo, source/lock/index change, CI trigger, push, GitHub comment, artifact execution or merge.

## Exact completed run

PR12 head `e3737ccdd9798e3246ebd2083d4c2bdecf74e569`; Product `40456226726` completed **failure**; Environment `40456226711` completed **success**. API event/head values match. Both Mac checkout logs and native reports bind actual pull-request merge **`ba437c0dc7e3e55adc4134dd80a0e1543cfe2901`**, merging e373 into protected main `66061439c79fa44b66a33782febc1fb0f31c73df`.

| Check | Job | Actual conclusion |
| --- | --- | --- |
| Product / Rust | 122030062175 | failure: workspace tests |
| Product / UI | 122030062312 | success |
| Product / macos-15-intel | 122030062437 | success |
| Product / macos-15 | 122030062560 | success |
| Environment / macos-15 | 122030061982 | success |
| Environment / ubuntu-24.04 | 122030062352 | success |
| Environment / macos-15-intel | 122030062431 | success |

The 05:55:19 UTC completion observation records both Mac jobs completed; the final Product API still reports failure. Old ada4/201f/7ba results were not substituted.

## Real Linux failure and proposed resource isolation

Actual Linux default-parallel Core command suite is **13 passed / 1 failed, 8.99 s**. `actual_reaped_failed_acp_lifecycle_can_approve_independent_command_while_task_result_stays_unknown` fails at exact line1096: task `cancelled`, expected `outcome_unknown`. Fresh grant has **760 ms remaining**, request follows in **0 ms**, and sessions.create consumes **946 ms**, exceeding the remaining grant. Being below the nominal1000 ms is insufficient. The real expired negative correctly refuses after1100 ms. The later startup-marker/stop/independent-command assertions were not reached in this Linux case. Original1000 ms and1/2/3 s thresholds remain; Linux cross-fixture concurrency pressure did not pass and must remain recorded as unmet, not relabeled by a future serial pass.

Independently inspected the exact proposed `.github/workflows/product.yml` diff: only `env: RUST_TEST_THREADS: '1'` is added to **Workspace tests with actual generated native-wire capture**. A byte comparison confirms deleting those two lines restores the entire e373 workflow; PyYAML verifies Rust25min/Mac30min and the original schema-capture/workspace commands. All tests, explicit test-internal daemon/worker/client concurrency, per-test limits, checks and production behavior remain. **Static approval:** reasonable isolation of independent real Git/SQLite/process fixtures, without skipping cases or relaxing single-test acceptance. It is not proof of CPU contention as the sole cause or of cross-fixture stress performance. No workflow modification or CI trigger by this reviewer.

Proposed workflow SHA-256 `9a18b87ce31aa760792ea294574f5d3535723cc1623131e6b9503c0d23293fb2`; saved diff hash `4cadb998b991847a7fc01a841de3c2009a363414a19608b28a82d200c662e551`. A new head must actually pass every required check; these Mac results cannot replace its future checks.

## Exact macOS test and release-audit evidence

Both raw logs show RUST_TEST_THREADS1 and original lifecycle limits. Exact binary sections establish **Core command14**, **native host11**, and **native feature harness6** all passed on each architecture, with zero failed/ignored/filtered cases. Actual WebView/daemon acceptance and the named-marker scan of the actual default release executable pass on both. The audit establishes absence of the named smoke identifiers in that executable, not a formal guarantee about every possible test control.

| Actual observation | Apple Silicon | Intel |
| --- | --- | --- |
| Core command14 | passed,59.00s | passed,71.49s |
| Fresh grant remaining/request interval |998ms/0ms |998ms/0ms |
| Original sessions.create RPC | **531ms** | **406ms** |
| Profile readonly/reservation/drifted-launch/launch (µs) |106552/179035/149605/289663 |273745/269216/271489/290534 |
| Native host / harness |11 / 6 passed |11 / 6 passed |
| Actual WebView and default-release named-marker audit |passed |passed |

These are observed e373 Mac results, separate from the earlier Linux profile. They support the reviewed batching in these runs but do not establish a latency percentile or a universal1s-start guarantee. Expired-grant rejection remains present. Development packaging steps succeed, but this reviewer did not inspect the dual DMGs; Primary's separate helper must verify those bytes, records/signing state and checksums before delivery claims.

## Downloaded native ZIPs, reports and PNGs

Downloaded only the two native artifacts from **this run** using GitHub API. Each artifact's workflow_run.id/head_sha matches40456226726/e373. Local SHA-256 equals the API digest. Before extraction, the independent helper checks path traversal/absolute/backslash names, normalized duplicates, link/file types, encryption, file/aggregate size bounds and ZIP CRC; no artifact code is executed. Each ZIP contains5 files.

| Artifact | ID / architecture | ZIP SHA-256 (API = local) |
| --- | --- | --- |
| native-webview-macos-15-ba437c0… |12712681660 / aarch64 |4fe9828ad260fa603154ecbbdeb496240c6a98d998c4df9159cbe1d0a13e7b33 |
| native-webview-macos-15-intel-ba437c0… |12712369741 / x86_64 |28ec556a5835c9634348bf54a3e0cc0c31b571bf91a915767fc6adb80f8b78cf |

Both native JSON reports/runner records actually pass: exact commit/OS/architecture, actual product composer and persisted same task ID, idempotency/invalid-input/journal evidence, **Command3 + Data3 read-only flags**, window-close daemon survival, rejected second writer, owned daemon restart/cleanup, reopened persistence/cancel/stop evidence; runner exit0, no timeout/error and confirmed owned-group cleanup. The three flags per category prove real list/empty-control/read-only-no-mutation behavior, not native command/data execution or owner approval. Both explicitly declare **native_dialog not_run_manual_acceptance_required** and **provider_models not_run_not_authorized**.

Both PNGs were decoded and visually inspected with view_image: actual native task windows, no blank/foreign screenshot. ARM1136×793 displays Conversation; Intel1292×912 still captures the Artifacts frame despite the scripted return to Conversation before reporting. This physical-paint timing difference is retained as a screenshot limitation: do not claim both show the same tab or that every below-fold command field is visible. It does not substitute for the exact read-only IPC/DOM report evidence.

| Native file | ARM SHA-256 | Intel SHA-256 |
| --- | --- | --- |
| native-smoke.json |9a5cc75e9e54cef4eafba5ea53b7ef1a92c73ed307c909ade234f23a1fbb6c54 |3955a3dab526ba39fc0f7a7cd0538e250dd16bb275ba4d440ba8dccd23c354b3 |
| native-screen.png |cba4906af48f53766022c2d9b44a1a4750b01603e0c5fc3a6bacb7be7feb4477 |8fde8f2c03560d5dbf98940a650da36373a3ee5c1f5a8cb65d8c6114037e5a56 |

## Preserved evidence

Ignored root: `/workspace/.onboarding/command-native-e373/`. Audit JSON contains all extracted file hashes and exact validation results; raw jobs and the failed Linux log remain unchanged.

| File | SHA-256 |
| --- | --- |
| `product-run-final.json` | `578ec790bc7b0520cef4b69b859bf172512bcc37b4ef5731f365214ce7c3eeb0` |
| `env-run-final.json` | `434bb8207b815ef4c09356d2f0563eaae2a0d50cb9642f16d425f4a908f91633` |
| `product-jobs-boundary2.json` | `324b740e6c9520d29dc8d5bc59fb2214e9f6ce3e518a8940fadce0d2879d55fc` |
| `env-jobs-initial.json` | `12847b3dd8438d01c12cc340784feb8df9682c0310b82f286377efc8100d48f2` |
| `artifacts.json` | `0c01d23d4409ac623586ab2caa6a02a6b7cc7b74c6d31ac48eb0377fc7760632` |
| `rust-job-122030062175.log` | `06014e87b502af9115ba32a4308c98ec73389bf9f0c7ca60a72c6781d20465f5` |
| `arm-job-122030062560.log` | `7200db110ef02454c07a6102a163e5a003ddc9a58cd5e26a6ae24f7f998be9c9` |
| `intel-job-122030062437.log` | `a1518b12d5badb5509a2e93c37bf133baa76f1cf029ed0bd1ebaa96ee8a076f5` |
| `arm-audit.json` | `2b5e1d15df74fb128e0f19aeab024a5c0ee034fdc63221e02202966620a49062` |
| `intel-audit.json` | `61ad402192e5f4c21f7c7b1efdd77d853a858d8e407ab3bd51384886e0acc074` |
| `audit-native-artifact.py` | `0b182023209a33030945aaf2e48643dad7c8d92756d6c265058fe45f47e0b7d7` |
| `proposed-rust-isolation.diff` | `4cadb998b991847a7fc01a841de3c2009a363414a19608b28a82d200c662e551` |

Actual operations: read-only gh api run/jobs/completed-job-logs/artifacts metadata; GET artifact ZIP by exactID; independent Python hash/safe-ZIP/report inspection; view_image on both extracted PNGs; read-only Git/YAML comparisons. No Cargo or private Provider/model/account/signing/production/additional-resource operation. Production optimization's prior cb82 review/hash remains unchanged. The overall failed head, Linux pressure limitation, unrun native owner/model scope and separate installer verification are not silently closed by the successful Mac dimensions.
