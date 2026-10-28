# Independent PR13 native evidence review — exact 0A0B subject

Status: **FAILED on both macOS jobs at different stages**. Product run completed failure; no new native acceptance or merge recommendation. Reviewer `/root/review`; no source/locks/index/targets/CI/remote changes.

Subject head `0a0beac38260cc426b4a0887b2bf59937ec9ff90`, Product PR run `40475194913`, Environment PR run `40475194909`. Actual checkout `3d958a30235090aa7c3e86c75fde6b14df6575e5` has parents `bcf10bb802258645c554f37581237a6c0cb3cbe6` and the subject head. Both checkout and head tree are `d23258b0a838fff5e822309dbd3d8d308cbf867e`, independently read from Git API and completed-job logs.

All three Environment jobs completed success: Apple Silicon `122091592091`, Intel `122091592262`, Ubuntu `122091592277`. Product Rust `122091591761` and UI `122091591672` also completed success. Their same-checkout raw logs record the 14-command suite, Goal contracts 48/store 34, runner contracts 17, real SIGKILL/restart smoke, frontend 259, browser/Unix-IPC 38 checks/24 images, Command 5, Goal default 9 and structured editor 10. This does not substitute for native execution.

The initial checkpoint had both macOS jobs pending; their final, distinct failures are recorded below. Raw responses, source identities, completed-job logs and the prepared bounded native artifact audit are retained under `/workspace/.onboarding/goal-native-0a0b/`.

The earlier AEFF dual-Mac harness failures and Linux source correction evidence remain separate. No report, package, screenshot or passing count from the earlier subject is counted as this subject's result. Manual OS owner dialogs, live Provider/model/account calls, minimum macOS compatibility, user installation, signing, populated Goal plans, dispatch and whole-product completion are outside this evidence scope.

## Actual Apple Silicon failure, independently read

Product Apple Silicon job `122091591692` completed failure at checkout `3d958a30235090aa7c3e86c75fde6b14df6575e5`. The failure was inside the workspace tests in `Core and native host tests`, before native host/harness or real window execution. `agentbaton-providers --test native_codex` produced **18 passed / 1 failed**:

- Test: `native_faults_reap_child_without_replay_or_secret_error_text`.
- Panic: `crates/providers/tests/native_codex.rs:119`, the shared `start(mode)` helper's unwrap of `ManagedProviderProcess::start(ProtocolRoute::CodexAppServer, ..., Duration::from_millis(500))`.
- Actual error: `TransportFailure { kind: Timeout, side_effect_state: Unknown, protocol_error: None }`.

The fault test iterates generated modes malformed/duplicate/oversized/error/eof. The current panic records neither the selected mode nor initialization timing details, so neither is invented here. This is a startup failure before the case's later prompt fault/reap/no-replay assertions. It is not the earlier alias failure, not the command 1000 ms grant-admission test, not a Goal Store failure, and not a real vendor/model call. The separate original 4 MiB/500 ms native stderr test actually passed in this suite. The Core 14-command suite also passed before this later failure.

Native host 13, feature harness 12 (including the new alias correction), real first/reopened WebView, Goal3 plus DB-invariance evidence, Command3/Data3 native checks, PNG capture, release application audit and development packages were not reached successfully on ARM. Upload-artifact explicitly reported no files under `artifacts/native-smoke/`. The actual run artifact API snapshot contains only provider-contract and frontend artifacts at this boundary, no ARM native ZIP; no nonexistent pair was downloaded and no old pair substituted.

ARM full raw log SHA-256: `dcd64d5e1475e0acbf3f9042f76db918840d31f735b92d6aa04822d53dd6da0f`, path `/workspace/.onboarding/goal-native-0a0b/product-job-122091591692.log`. Source/timeouts, pipeline, runner concurrency and product guards were not changed by the reviewer. Intel was allowed to finish independently; no cancellation or speculative rerun was requested.

## Final Intel result: a different failure

Intel job `122091591770` completed **failure** at the same checkout. Its complete workspace tests (including the 19-case generated native protocol suite) and all **13 native host tests passed**. In the actual macOS WebView step, the feature harness produced **11 passed / 1 failed**:

- Test: `owned_runtime_alias_is_rejected_but_its_canonical_private_database_binds`.
- Panic: `apps/desktop/src-tauri/src/bin/native_smoke.rs:1065`, the positive canonical `Daemon::bind(&runtime)` assertion.
- Actual error: `INTERNAL_ERROR`, `Cannot bind private daemon socket`, side-effect state `not_started`.

The original generated-project canonical test **actually passed on Intel**. The new test's alias-denial and unchanged-database assertions precede the failing positive bind. This is neither ARM's protocol startup timeout nor AEFF's earlier symlink-path rejection. Core maps the Unix socket bind error to a generic message, and this log prints neither the runtime path nor OS errno. The added nested path under macOS TMPDIR makes Unix socket pathname capacity a plausible hypothesis; it is not proven to be the historical cause merely from that generic error.

An independent bounded Linux control used the already frozen fixed-helper/Core9 probe, with mode-0600 databases in generated canonical private paths: a 132-byte socket path failed actual Core bind with the same generic error (exit 101), while a 41-byte socket path bound successfully (exit 0). Direct Linux Unix socket bind on the long path reported `AF_UNIX path too long`. `controlled-socket-length.json` records the exact ELF identity, path byte lengths and logs. This proves a relevant Linux mechanism, **not** the actual Mac path/errno or a fresh current-Core build. No production guard or fixture was changed by this reviewer.

Intel raw log SHA-256: `acf186178c4b94ad3ae41d6febc0282fccdb8f929d753d90971fdd0b94759d50`. Its actual native ZIP is artifact `12721123165`, API/local SHA-256 `2b3b58800512a728887d472ad49f8ed745d74778f45fbd15df6d168cfcb9d343`. Bounded size (1213 bytes), CRC and exact entry-list checks find **only `lane.log`**. No native report, runner JSON or PNG exists in it. ARM has no native artifact at all. A reviewer artifact-name absence check initially used an overly broad ARM prefix which also matched Intel; it was corrected to the exact ARM name before download, and the audit correction is retained separately. That reviewer script error is not a product failure or evidence of an ARM artifact.

## Final required checks and evidence boundary

| Required job | Run / job | Final result |
| --- | --- | --- |
| Environment Apple Silicon | `40475194909` / `122091592091` | success |
| Environment Intel | `40475194909` / `122091592262` | success |
| Environment Ubuntu | `40475194909` / `122091592277` | success |
| Product Rust | `40475194913` / `122091591761` | success |
| Product UI | `40475194913` / `122091591672` | success |
| Product Apple Silicon | `40475194913` / `122091591692` | failure in generated native protocol startup; 18/19 |
| Product Intel | `40475194913` / `122091591770` | failure in feature harness; 11/12 after host 13 passed |

The Product run completed failure. There is no valid new native pair or package pair to accept. Neither Mac completed actual first/reopened WebView, new Goal3 strict booleans plus independent unchanged counts/sequence, original Command3/Data3 in the page, same-task/project native report, screenshot capture, real window/daemon lifecycle acceptance, default release binary audit or development packaging. Linux/source-leaf checks and browser/UI success do not fill these missing native dimensions. No new thresholds, test skips, permission relaxations, old screenshots or prior packages were introduced to claim success.

All raw API snapshots, completed-job logs, source hashes, the Intel log-only ZIP/audit, the initial pending report and clearly labelled mechanism controls are retained under `/workspace/.onboarding/goal-native-0a0b/`. Verification manifest SHA-256: `b77ae1486864c4c2310800c88665d0c3184c2641fba03d4b5cebe695aa685cfe`. Completed Linux UI raw log SHA-256 is `32641aaa4bde4415c1238e6c7930e12a10b6f295e2937abc6af23cb614ea422c`; Rust log is `9e1191bc199b639fc34e639463c7eb50659216a61b7681c48064ad0b85e37345`. Reviewed native harness source remains `775f355327660a3d21c51ddc317c9ac958c870b58dd7c1ee8c633efba8c12309` at this CI head; workflow and relevant provider source hashes are bound in the manifest's `source-hashes.json`.

No Cargo, shared/native target changes, repository source/lock/index modifications, CI triggers/cancellations/reruns, GitHub messages, pushes or merges were performed. The only repository change is this untracked report. Both new failures require diagnosis and an independently reviewed correction followed by fresh exact-head CI; this report does not authorize treating failed CI as acceptable.
