# Independent PR13 exact-head native CI review

Status: **FAILED on both macOS architectures**. Product run completed failure. No native acceptance or merge recommendation.

Subject: `aeffd473ec4cf9db5cc48c9aa666ed9811e511e1`, Product PR run `40471245384`, Environment PR run `40471315226`. This report is distinct from the source-leaf review. The original Environment run `40471245438` is retained as cancelled, not combined with the replacement run. The replacement is a normal PR ready-for-review event under the existing concurrency policy; it is not a manual rerun.

The completed replacement Environment run has all three actual jobs successful: Ubuntu `122078865617`, Apple Silicon `122078865925`, Intel `122078866063`. All three raw logs identify checkout `1b990b0853ab837ffb97cc97e0436bae6c09370e`. Git API metadata confirms parents `bcf10bb802258645c554f37581237a6c0cb3cbe6` and the subject head, with tree `541d5e54b89394972d0ffd07bb6010a2b863b2f7`, equal to the head tree.

Product jobs: UI `122078602855`, Rust `122078603051`, Apple Silicon `122078603054`, Intel `122078603267`. Rust has completed successfully; its same-checkout raw log records the real command suite (14 tests), Goal contract/store suites, 17 runner contracts, and actual daemon/CLI SIGKILL/restart smoke. This was the initial checkpoint; final Intel and ARM failure details follow below.

Raw API responses, completed-job logs, source hashes and the prepared bounded native artifact audit are stored under `/workspace/.onboarding/goal-native-aeff/`. No Cargo, target writes, source/lock/index modifications, workflow triggers, GitHub comments, pushes or merges were performed by this reviewer. Gate and OWNER-001/002 were read; private accounts/models, additional spending, production release and signing remain outside this review.

## Actual Apple Silicon failure

Job `122078603054` completed **failure**. Its workspace and default native host step succeeded, including the actual 14-command suite and all 13 native host tests. In the next `Actual macOS WebView and daemon lifecycle acceptance` step, the native smoke script ran its 11 harness tests: **10 passed / 1 failed**. The failing test is `generated_registered_project_setup_creates_no_goal_or_attempt_authority`, `apps/desktop/src-tauri/src/bin/native_smoke.rs:1022`:

> helper-created DB mode=600; actual authority rejection=Some(CoreError { code: "PERMISSION_DENIED", message: "Runtime paths must not contain symbolic links", ... side_effect_state: "not_started" ... })

This is the new fixture's real-daemon-bind assertion, not the 1000 ms grant-admission lifecycle test, a Goal Store failure, the earlier 0644 database issue, or a running GUI failure. The actual window executable, first/reopened WebView, new Goal3/DB invariant, screenshots, default release application audit and package steps did not run successfully afterward.

Source inspection distinguishes the paths: the actual native executable already canonicalizes `temporary.path()` before preparing the project or launching its daemon (`native_smoke.rs:823`); the new unit passes `directory.path()` directly to `Daemon::bind` (`:1021`). Core intentionally rejects any symlink ancestor. A Darwin temporary-directory alias is consistent with the observed guard error, but the CI log does not print the exact rejected path, so this report does not claim that historical path string was observed.

A separate independent Linux mechanism control used the already frozen fixed-helper/Core9 probe from the source-leaf review, under umask 022, with a generated private symlink-parent runtime and a canonical runtime. Both databases were mode 600. The alias case failed exit 101 with exactly the same symlink guard error; the canonical case passed exit 0 with real daemon bind. It changed no repository source or Cargo cache. This supports a fixture-only canonical-path correction while preserving production no-symlink checks; it is not a new Darwin execution or a fresh compilation of the current Core. Exact probe hash, paths, results and log hashes are in `controlled-runtime-alias.json`.

Full ARM job log SHA-256: `cb4d8d7e20483a538ed4d2a697cc7754613d3cce426c92bffcbd2d9b2237e636`. The actual uploaded native artifact is **log-only**, ID `12719437358`, ZIP API/local SHA-256 `69e7092c620d67803fd2a8e1bdc8766949013eb32e2afdc5f5285c2abfb3d852`; bounded ZIP/CRC validation finds only `lane.log`. It contains no `native-smoke.json`, runner report or PNG, and cannot satisfy native acceptance. The ZIP, raw lane and `arm-failure-artifact-audit.json` are retained.

UI job `122078602855` also completed successfully at the exact checkout: 259 frontend tests, actual browser/Unix-IPC 38 checks/24 images, Command 5 checks, Goal default 9 and structured editor 10. Those are browser evidence, not substitutes for the failed macOS lane. Rust raw log SHA-256 is `681c10ed69f6de14191bcd591b233361402e35af06f865b3fa7218515d21c582`.

## Check boundary and limitations

At the final captured API boundary, five distinct required checks succeeded (replacement Environment three, Product Rust, Product UI), and both Product macOS jobs failed. Product run `40471245384` completed failure. The earlier checkpoint with Intel still running is archived separately. `head-check-runs-at-report.json` retains both the cancelled original Environment job and complete replacement jobs; they are not mixed to manufacture a passing run.

| Required job | Run / job | Observed result |
| --- | --- | --- |
| Environment Ubuntu | `40471315226` / `122078865617` | success |
| Environment Apple Silicon | `40471315226` / `122078865925` | success |
| Environment Intel | `40471315226` / `122078866063` | success |
| Product Rust | `40471245384` / `122078603051` | success |
| Product UI | `40471245384` / `122078602855` | success |
| Product Apple Silicon | `40471245384` / `122078603054` | failure, new harness 10/11 |
| Product Intel | `40471245384` / `122078603267` | failure, new harness 10/11 |

There is no valid pair of new native reports/PNGs or development packages to approve from this completed run. Original Command3/Data3, new Goal3 plus unchanged structured DB counts/sequence, same-task/project identity, native window close/reopen/restart/owned cleanup, and default release audit are still source requirements and must pass on a fresh exact CI head after correction. Prior PR12 native reports, prior Linux 13+11, or browser results cannot fill those missing dimensions. OS owner dialogs, populated Goal scope/plan history, dispatch, live Provider/model/account behavior, minimum macOS compatibility, signed production release and whole-product completion were not established. Package verification and any protected merge belong to the primary and were not performed here.

Source identity: `native_smoke.rs` remains `e63ae80dc190b1884e96b55b127ef648325b583585bad939a94f0eae60a842b9`; the other four source-leaf hashes remain exactly those independently reviewed in source-leaf report `6851f0510c0c580045dec63d8301d8e2ddf972a0c918d465d9a74e06187e4bd3`. Full workflow/source identities are recorded in `source-hashes.json`.

Evidence manifest: `/workspace/.onboarding/goal-native-aeff/verification-manifest.json`, SHA-256 `53b1dd2da11f29c1daeef2dae454d34af3257c68ff8a7b18cf7081fbe0e51881`. It binds API snapshots, checkout parent/tree metadata, completed-job raw logs, the actual log-only artifact and audit, source hashes, the independent alias control, and the captured status boundary. This review document is left untracked; it does not mutate the frozen PR head. Both jobs were allowed to finish; no reviewer cancellation or rerun occurred.

## Final Intel outcome and preserved earlier checkpoint

Intel job `122078603267` independently completed **failure** at the same actual checkout. Its full raw log confirms the complete 14-command suite and all 13 native host tests passed. The retained 1000 ms lifecycle control reports fresh TTL 998 ms, request delay 0 ms and admission 340 ms; the deliberately expired negative also remained correctly denied. Thus this failure is not evidence that the earlier admission issue returned.

In `Actual macOS WebView and daemon lifecycle acceptance`, Intel's 11 harness tests produced **10 passed / 1 failed** in 2.24 s. The actual failure is again `generated_registered_project_setup_creates_no_goal_or_attempt_authority` at `native_smoke.rs:1022`: database mode 600, real `Daemon::bind` error `PERMISSION_DENIED`, `Runtime paths must not contain symbolic links`, side-effect state `not_started`. This is confirmed by Intel's raw log, rather than inferred from ARM. Intel's actual window execution, Goal3/DB invariants, Command3/Data3 in the native page, screenshot, reopened-window/daemon acceptance, default release audit and development packaging were not reached successfully.

Intel raw job log SHA-256: `16e9c3b2dd86e0fb3da5868152791f48ebdf2a19a464db352e205220011d75bb`. Its actual native artifact `12720035337` is a 1213-byte **log-only** ZIP; API and local SHA-256 both `30b4cc83a767d0a99f948e1f55cbcb1c27ad33a566a5da2a95ff53fe28c06229`. Bounded size, CRC and exact entry-list checks confirm only `lane.log`, with no native JSON, runner JSON or PNG. The archive, raw lane and audit JSON are retained alongside ARM's equivalent failed artifact.

The initial report has been copied byte-for-byte to `/workspace/.onboarding/goal-native-aeff/report-at-intel-pending.md`, SHA-256 `cb2f2dc47533c0188f1ebefb457d47c6efae53a5f248b2ef5110ff6c8e2306fa`. Its initial manifest is preserved as `verification-manifest-at-intel-pending.json`, SHA-256 `6756e1fce37f0830c476f8173e205ecb7b684b6d17f9e076335275e0e4642706`. Neither the earlier pending observation nor the two actual Mac failures was overwritten. The final manifest additionally binds both completed-job logs, final API metadata and both failed native artifact audits.

The author's later fixture-only source correction is a separate subject for source review and a fresh exact CI run; its existence does not turn this AEFF run into a pass. This update used no Cargo or source/index changes while the author owned the native target.
