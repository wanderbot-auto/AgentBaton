# Independent PR8 native candidate review — 2a8092e / 18d05cd

Verdict: **PASS for the fresh dual-Mac scripted native workflow, six harness units, three approval-outcome regressions, App test suite and actual default-release marker audits on this exact PR candidate.** No native-evidence blocker remains for this candidate. This report does not itself perform a merge or replace the primary's DMG/protection/head/base checks, and it does not transfer acceptance to later PR10/Data changes or complete Design v0.2.

The reviewer independently queried GitHub API records, downloaded both native ZIPs and all seven job logs, inspected all ten native files, checked the exact report validator and Git trees, and viewed both PNGs. Downloads are isolated at `/workspace/.onboarding/provider-native-2a809/`. Only this review document was added in the repository. No source, lock, index, commit, push, merge or remote mutation was performed; the earlier 2da7/130f and other failed evidence was left intact. No Provider/account/model call was made.

## Exact PR head, checkout and source

Product [40425134648](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40425134648) and Environment [40425134840](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40425134840), attempt 1, are completed/success, both **pull_request** events with API head `2a8092ec7031a467ea4c02e86893f3ebdbab0dbe`. All seven independently downloaded logs actually check out **`18d05cd75404d108b2c5fdabfca699034445a43f`**. Both native reports correctly record that synthetic merge commit, not the PR head.

The independently fetched commit/tree API records identify the merge parents as base `c4e72c641e3edc3f3c0f665ffde5217933c2895f` and head `2a8092ec7031a467ea4c02e86893f3ebdbab0dbe`. Its tree is `e6ca68afa93a0bb1446a7744b8953dfdb86e5a7e`, identical to the local exact head's tree. Eleven relevant source/lock files were additionally matched to actual-merge Git blob IDs and SHA-256 fingerprints; no local ref fetch/update was needed.

| Successful job | ID |
| --- | --- |
| Product / Rust | `121928876786` |
| Product / macos-15 | `121928877091` |
| Product / UI | `121928877098` |
| Product / macos-15-intel | `121928877165` |
| Environment / macos-15 | `121928877243` |
| Environment / ubuntu-24.04 | `121928877533` |
| Environment / macos-15-intel | `121928877763` |

The three Environment jobs remain infrastructure checks, not product-native proof. The actual Mac Product lanes supply that separate evidence. Native reports identify aarch64 and x86_64. The two runner images are macos-15-arm64 and macos-15; these results do not establish an earlier minimum supported macOS or installation on user hardware.

| Architecture | Native artifact | Verified ZIP SHA-256 |
| --- | --- | --- |
| ARM / aarch64 | `12699965560` | `2c81ded1a6f2f3ec8b731b9b92f690baeca7791ba52044e2c2cb7a0a64399a0b` |
| Intel / x86_64 | `12699552493` | `ea7f7810140bd6f20094b87ba07df0cde7f8c9084e47f4a8c565c6f37a12a510` |

Both ZIP hashes match the supplied values and independently retrieved GitHub digest metadata. Artifact API records associate the correct Product run and PR head; names/report commits refer to actual checkout 18d05cd. Before extraction, each archive was checked to contain exactly five regular, safe relative files with no symlink/absolute/traversal entries: lane.log, native-run.log, native-runner.json, native-screen.png and native-smoke.json. Both native-run.log files exist and are empty.

## Fresh native behavior and regressions

Both new reports pass the exact source's `scripts/native-run.py::native_report_error`, have status passed/os macos/commit 18d05cd, and confirm first-window DOM mount, real composer submission/rendering, idempotency, invalid-input refusal and journal presence. Reopened reports bind the same task and confirm DOM mount, restart persistence, draft cancellation and no-attempt-dispatched stop evidence. All nine expected stages are present once in the correct order through reopened_report_received.

| Architecture | Same first/reopened task | Visually inspected first-window PNG |
| --- | --- | --- |
| ARM | `40fcead5-145a-4be9-ac1d-7f5bf2710f10` | 1136×793 |
| Intel | `7daf5b58-2746-41fa-93cf-52ef96115294` | 1292×912 |

Both actual PNGs show connected AgentBaton native task pages, the expected goal, Draft state and an honest absence of execution evidence. There is one first-window screenshot per architecture; no reopened image is invented. Reopened behavior is supported by the correlated native reports and invoke checks.

Both runs establish daemon survival after closing the window, second-writer refusal, owned daemon restart and cleanup. Both runner records report exit 0, timeout false, owned_group_cleanup true and passed, with no observation/cleanup/evidence error. SIGTERM and SIGKILL report observed_quiescent on actual Darwin. Both lane logs record all six native-bin tests passing, including the private-runtime and terminal-race regressions, with no failures/skips/filtered tests, followed by the actual WebView/IPC/persistence/cleanup PASS line.

The newly corrected tests were verified in these fresh Mac logs, not inherited from b994 or Linux:

| Actual Mac execution | ARM | Intel |
| --- | --- | --- |
| `approval_outcomes.rs`: existing connection-journal unknown control, confirmed probe-refusal IPC regression, original grant/same-proposal control | 3 passed, 00:15:27 UTC | 3 passed, 00:16:17 UTC |
| Vitest `src/App.test.tsx` | 12 tests passed, 00:14:55 UTC | 12 tests passed, 00:15:18 UTC |
| Whole frontend Vitest suite | 49 passed | 49 passed |
| Native-bin fixture units | 6 passed | 6 passed |

The three outcome tests execute real local Store/Unix-IPC fixtures and confirm same-proposal recovery semantics without executing the generated executable. The App tests execute Vitest on macOS runners; they are not an actual native owner-dialog test. The WebView harness remains the bounded real draft workflow. This distinction is essential: host regression success and a real window do not imply automated native approval, artifact-policy approve/run, Provider execution or new Data UI acceptance.

## Actual default-release audits

Both job APIs mark the actual native lane, default application build and release audit steps successful. Full logs show the actual command on the shipping default bundle path:

`bash scripts/native-audit-release.sh apps/desktop/src-tauri/target/release/bundle/macos/AgentBaton.app/Contents/MacOS/agentbaton-desktop`

ARM prints `PASS no native harness identifiers in default application executable` at 2026-10-27 00:21:23.079067 UTC; Intel prints it at 00:29:32.191661 UTC. These are actual default-release executable audits, not debug smoke marker checks. The reviewer read those real CI results; release executable/DMG download and inspection belong to the primary's separate review. Marker exclusion does not establish signing/notarization, general binary security or production approval.

## Fingerprints and preserved limits

Paths below are relative to `/workspace/.onboarding/provider-native-2a809/`.

| Evidence | SHA-256 |
| --- | --- |
| `arm/verified/native-smoke.json` | `1bb3c80e9785c9a15498037521b7a52880b12633dbd4eb6d717b5a8af83e95b7` |
| `arm/verified/native-runner.json` | `420778039d52fa4b7d743caaa832b6d26957a1b5578e61c7c8345fb65130a2b0` |
| `arm/verified/native-screen.png` | `fd26e2e5cbd86631a8c2e963e1003b2f3879a894c4589e0575669059554e0d34` |
| `arm/verified/lane.log` | `e88db0f2f47bf942ed290895400fe9bec9802f523b23f895df5b9f3fafb5919d` |
| `arm/job.log` | `117ad339225f1bbc94b1e4b086175c272e2f5b616dae0de82ad88f8005241fdf` |
| `intel/verified/native-smoke.json` | `a7b2a2d3c133ed26d2309715079ff692030b2b42633322dfdd24a24aaa3c67b5` |
| `intel/verified/native-runner.json` | `420778039d52fa4b7d743caaa832b6d26957a1b5578e61c7c8345fb65130a2b0` |
| `intel/verified/native-screen.png` | `50494dcf895da9bf3d2c12d456cf0e624950ea80a1a10b6a1384e9c99e52ac30` |
| `intel/verified/lane.log` | `a1992f2923cff41566800ea8686464a4eaf6e31561c175a2e3ce4f5aeac03582` |
| `intel/job.log` | `72e07ea53c2fbf37e7edf48897e8543912be83c3eba211ec589f162acddefa41` |

The complete 27-file download manifest, all seven actual checkout mappings and eleven source SHA-256/Git-blob bindings are in `verification.json`, SHA-256 `9e59ea4d377f5566570cbb2eed9592b0c7a33641e09d6a11850beadd70c9bdd1`.

| Relevant exact-tree source | SHA-256 |
| --- | --- |
| `apps/desktop/src-tauri/src/bin/native_smoke.rs` | `8eb1d8114d60a86eaabf4d316a9f4d7931b1c463a13b1624dee7ed65af19d31f` |
| `scripts/native-run.py` | `b1aa421c9b98f3efe449a64b317566e131385f26f65b4c07dca01203c6f0d46e` |
| `scripts/native-macos-smoke.sh` | `d2228f8a325a386bf83171d41d5616c625fdfd5c16fa17494488e10329fc0886` |
| `scripts/native-audit-release.sh` | `dd60a45803ebc66c8dc5a19bba24da35b6e20716589b332314c7d1ecc4fa52d0` |
| `.github/workflows/product.yml` | `dae8b93309c42038015946557454fe45e1709ea3248cb766f5731c2decb56220` |
| `crates/core/src/ipc.rs` | `8d12f952c123d209a7d70e5380bea9aa212908aea65981bbead67dce86ef9025` |
| `crates/core/tests/approval_outcomes.rs` | `dbc6adc0de7e2c086e31a022f52df374cf7fc3c7c6d3f71e1ccc53e3acf96a67` |
| `apps/desktop/src/App.test.tsx` | `194c6c8cc63ed1ab120bde3cdb9283de9d5d263e6198ab674f440d1dc9002e44` |

Reports explicitly retain `native_dialog=not_run_manual_acceptance_required` and `provider_models=not_run_not_authorized`. No manual owner dialog, real vendor model/account, active Provider work across quit/sleep/login, minimum-OS qualification, OS sandbox enforcement, signed/notarized delivery or production release is proven here. Earlier 2da7/130f failures remain failed historical records; the current fresh results do not rewrite them or prove every past failure's cause. Any later PR10/11/Data composition needs its own actual target/main candidate CI and review, rather than inheriting this result.
