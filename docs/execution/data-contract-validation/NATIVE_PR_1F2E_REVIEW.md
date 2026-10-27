# Independent Data PR attempt-2 native review — 1f2ed51

Verdict: **PASS for the recovered exact PR11 native/build candidate**, API head `06e7fa9a9f87c5e69564bc2a1afd97418d762d8d`, actual checkout/report commit **`1f2ed5136a6b362c07f5a031cfc15aaadfec1f7b`**. No native-evidence blocker remains. The primary must still use its final expected-head/base/check protection guard and separate DMG verification; this report neither merges nor bypasses protection.

The reviewer independently fetched attempt/run/job/artifact/PR/commit records, both ZIPs and all seven complete logs, validated all ten native files and viewed both actual PNGs. Only this new document was written; no source, locks, index, stage, commit, push, merge, Cargo/shared-target or Provider/model/account action occurred. Downloads are isolated at `/workspace/.onboarding/data-native-pr-1f2e/`.

## Attempt and source binding

Product [40432923340](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40432923340) and Environment [40432923530](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40432923530) are **pull_request, attempt 2, completed/success**, API head 06e7. The independent attempt-1 API records remain **cancelled**. These are restored original PR workflows, distinct from the successful manual Product40432924233/Environment40432928403 previously reviewed at raw head 06e7. Neither cancellations nor the earlier blocked/unstable observations were erased or retroactively labeled successful.

All seven current job logs actually check out 1f2ed51, and both native report commits/artifact names agree. Commit API metadata confirms parents base `febe7c74f62aad1f767aa60a0eab5e3fc0529dcb` and head 06e7. Its tree **`80574899985cc962cf293bc8518dd58a4f5232dc`** equals the committed head tree. The PR snapshot independently read during this review is open/unmerged, head/base unchanged and **mergeable_state=clean**. This is a point-in-time observation, not permission to skip the primary's final race/protection checks.

| Successful attempt-2 job | ID |
| --- | --- |
| Product / macos-15 | `121966210750` |
| Product / macos-15-intel | `121966210641` |
| Product / UI | `121966212199` |
| Product / Rust | `121966241263` |
| Environment / macos-15-intel | `121963775998` |
| Environment / macos-15 | `121963777213` |
| Environment / ubuntu-24.04 | `121963777896` |

Environment remains infrastructure-only. Native proof comes from the separate Product Mac lanes.

| Architecture | Artifact | Verified ZIP SHA-256 |
| --- | --- | --- |
| ARM/aarch64 | `12704846461` | `d102ee79c7378a29d90e788e81c6fd71bf59ce67847d5b7f660161aac2674188` |
| Intel/x86_64 | `12705150346` | `b5b11a03f74068105e6ac53be95b8c51c408824925109368d9c6dd6a0b53285e` |

Both hashes match supplied expectations and fetched GitHub digest metadata. Both archives have exactly five safe regular relative members, no absolute/traversal/symlink entries: lane.log, native-run.log, native-runner.json, native-screen.png and native-smoke.json. All ten files were checked; both native-run.log files are present and empty.

## Fresh native/host/build results

Both reports pass the committed common runner validator plus strict checks for **dataContractLists=true**, **dataContractEmptyControls=true**, **readonlyDataNoMutation=true**. The pinned native code/shell is the same source traced in the prior Data reviews: actual fixed native lists and actual folded/expanded Data DOM controls/read readiness/no alert/disabled approval/unchanged daemon sequence must succeed before those flags are emitted. No preset Data acceptance or Command checks are inferred.

Both first windows confirm DOM, actual composer/task rendering, idempotency, invalid input and journal. Both reopened reports bind the same task and confirm DOM, persisted restart, draft cancellation and no-attempt-dispatched evidence. All nine stages appear once in order through reopened_report_received. Window-close daemon survival, second-writer refusal, owned restart and cleanup are true. Runner records report exit0/no timeout/owned_group_cleanup=true/passed, no observation/cleanup/evidence errors, and both signals observed_quiescent on actual Darwin.

| Architecture | Same first/reopened task | Viewed first-window PNG |
| --- | --- | --- |
| ARM | `48526d49-bd6f-41d0-a846-d5dfb0ad399b` | 1136×793 |
| Intel | `bd7e8ddf-136c-44d6-9321-ddde87a2a821` | 1292×912 |

The actual ARM image shows connected Conversation/Draft; Intel shows the actual Artifacts layout and Data scope warning. Neither is an approved policy, data-run result or reopened image. Both complete Mac logs confirm Core111 (including Data13, busy1, schema7 migrations3 and approval outcomes3), native host8, harness6 and UI129. The nested FIFO child is not counted twice. These current PR results are independently observed, not inherited manual-test claims.

Both logs run the actual default-release audit command on `apps/desktop/src-tauri/target/release/bundle/macos/AgentBaton.app/Contents/MacOS/agentbaton-desktop`. ARM prints `PASS no native harness identifiers in default application executable` at **2026-10-27 02:16:00.928528 UTC**, Intel at **02:26:02.711549 UTC**. Build/audit/packaging jobs are completed/success. This is the real default executable audit, not a debug-harness scan. DMG inspection is the primary's separate evidence; no DMG/release executable was downloaded/reinspected by this reviewer. Marker exclusion is not signing/notarization or general binary-security proof.

## Fingerprints and limits

Paths below are relative to `/workspace/.onboarding/data-native-pr-1f2e/`.

| Evidence | SHA-256 |
| --- | --- |
| `arm/verified/native-smoke.json` | `0adc6fe66e789bbf7c1d8a149e53bf2536f0252f50a808482e93a45212a97346` |
| `arm/verified/native-runner.json` | `420778039d52fa4b7d743caaa832b6d26957a1b5578e61c7c8345fb65130a2b0` |
| `arm/verified/native-screen.png` | `c37d6c0f1fe02edb8d7f6966a94d771e427473af98b9655a8185bbe997dea195` |
| `arm/verified/lane.log` | `74c056f2b8ef5a820919913d564c4a921bfd81b22f65a809b5045d4b0c5a7beb` |
| `arm/job.log` | `1e19958d740846db3fb9a236e5750aa6955e23b394623582a0a08ff6ed2ef079` |
| `intel/verified/native-smoke.json` | `926df85e4f79045d327d5ad3730bda75b1e1be4ad25cd7e1b929db881d0f7cdf` |
| `intel/verified/native-runner.json` | `420778039d52fa4b7d743caaa832b6d26957a1b5578e61c7c8345fb65130a2b0` |
| `intel/verified/native-screen.png` | `028c080a979cf430f5f5f714a44121c4e6b24cff7e24f20e02495cc6da492d5d` |
| `intel/verified/lane.log` | `e4967a5fec0ab44928094eb72099908fd157271b5b932447f7b9c930f57e70b9` |
| `intel/job.log` | `e265d9201831fbbf844b7f537ceb5d74d6db3796dcb12b7e584651c4f035ac1f` |

The complete 29-file manifest (including attempt1 cancellation and actual-merge metadata), seven checkout mappings and 16 exact-source/lock fingerprints is `verification.json`, SHA-256 **40a9cfc4399d3a6bed35c3b1ebf04d291fbc2df56dbaa65079152c391951cf1f**. Source hashes were read from exact 06e7 Git objects; actual 1f2ed51 has the same tree. Native smoke `81dacf1b3c39d1fcde3b3012809720cf1d35c0d6caf63b0d99138cd165eff5a1`, shell `b1ae9e6f083baf152a44b09f69278e9d2ed12bf6e680ba9d8f33d659b204120b`, Core Data `5c5ee6842f1342e95f9e19d39591c25c0ec5716d89f30e3384b0967ce670b5ce` remain pinned.

Native dialogs and Provider/models explicitly remain not_run/manual_acceptance_required and not_run/not_authorized. No data-policy approval/run through native UI, scientific/algorithm correctness, TaskAccepted/Git adoption, OS sandbox, ordinary sleep/login/active Provider recovery, minimum-OS qualification or production release is established. Command library/Core/UI and later source trees require their own review/CI; these accurate restored PR results do not accept them or complete Design v0.2.
