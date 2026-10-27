# Independent macOS native evidence review — b9942d3

Verdict: **PASS for the defined scripted native WebView/private-daemon lifecycle and both actual default-release marker audits at `b9942d319b70dab22bf723d9f03230ebae08689a`.** This is a narrow evidence verdict, not approval to merge PR8. The separately assigned investigation of finalize action commitment followed by confirmation-journal failure remains outside this harness; this success cannot clear an untested fault or replace review/new CI for any resulting fix.

The reviewer independently fetched GitHub run/job/artifact metadata and full Mac logs, downloaded both native ZIPs, checked digest/ZIP safety and all ten contained files, parsed reports with the current `scripts/native-run.py::native_report_error`, and visually inspected both actual PNGs. Downloads are isolated at `/workspace/.onboarding/provider-native-b994/`. This review added only this document in the repository; no source, locks, index, commits, refs or remote state were modified. No Provider/account/model call was made, and old failures were preserved.

## CI provenance and actual checkouts

Product [40418995061](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40418995061), attempt 1, is completed/success, `workflow_dispatch`, branch `codex/provider-runtime-adapters`, head b994. Both downloaded Mac job logs explicitly report checkout `b9942d319b70dab22bf723d9f03230ebae08689a`; native reports and artifact metadata/names bind the same exact source.

Environment [40418995472](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40418995472), attempt 1, is completed/success with API `head_sha=b994`, but its event is **pull_request**. All three independently downloaded Environment logs actually check out synthetic merge **`5fbbf1ec36929e3514e7cc69093c70ed5e9f019a`**, logged as merging b994 into `c4e72c641e3edc3f3c0f665ffde5217933c2895f`. The head association must not be described as all seven jobs executing the same checkout. Environment remains infrastructure-only; its three workflow checks are unchanged from 9edf.

All seven job APIs report completed/success:

| Job | ID |
| --- | --- |
| Product / UI | `121908374642` |
| Product / Rust | `121908374836` |
| Product / macos-15-intel | `121908374930` |
| Product / macos-15 | `121908374932` |
| Environment / macos-15-intel | `121908363170` |
| Environment / macos-15 | `121908363700` |
| Environment / ubuntu-24.04 | `121908363727` |

Both Mac Product jobs ran macOS 15.7.9 (24G830). ARM image `macos-15-arm64` version `20260907.0337.1` reports native architecture aarch64; Intel image `macos-15` version `20260824.0482.1` reports x86_64.

| Architecture | Artifact | Verified ZIP SHA-256 |
| --- | --- | --- |
| ARM | `12697906046` | `75cfe57ac6bf8c2b4d1e27b9262dbb0d72ac35bc91bb587db9068809fefb1eda` |
| Intel | `12697406840` | `1850ee1da6d8422814f0ac3036c78f168701d6ff3416df6562813e53bad609cf` |

Each archive's SHA matches both the supplied value and independently fetched artifact API digest. Both archives have exactly five safe regular files, no symlink/absolute/traversal entries: lane.log, native-run.log, native-runner.json, native-screen.png and native-smoke.json. Both native-run.log files are present and empty.

## Native evidence established

Both reports have status passed, os macos and the exact b994 commit. The first-window checks confirm DOM mount, actual product composer submission/rendering, idempotent retry, invalid-input rejection and journal presence. The reopened checks confirm DOM mount, the same task, persisted restart, draft cancellation and no-attempt-dispatched stop evidence. All nine distinct diagnostic stages are present through reopened_report_received.

| Architecture | Same first/reopened task | Actual first-window PNG |
| --- | --- | --- |
| ARM | `cb0db176-36e2-403d-96f4-99cf56b26545` | 1136×793 |
| Intel | `e2bf9e91-ba2e-4542-917f-938bf16cef8a` | 1292×912 |

Both images were actually viewed and show the connected native AgentBaton task page, matching goal text and Draft state, with explicit absence of execution evidence. They are first-window captures; there is no claimed reopened screenshot. Reopened behavior is established by the correlated typed-invoke/native reports and stages.

Both runs confirm window-close daemon survival, second-writer refusal, owned restart and owned daemon cleanup. Both external runner records report exit 0, timeout false, owned_group_cleanup true and passed, with no observation/cleanup/evidence error. Both SIGTERM/SIGKILL results are observed_quiescent, using the original unreaped group-leader check on actual Darwin. Each lane log records six native-bin fixture tests passing with zero failed/ignored/filtered tests and the final actual-macOS WebView/IPC/persistence/cleanup PASS line.

## Actual default-release audits

Both job APIs mark `Build desktop UI and application` and `Audit the actual default release application for test controls` successful. Both full Mac logs execute:

`bash scripts/native-audit-release.sh apps/desktop/src-tauri/target/release/bundle/macos/AgentBaton.app/Contents/MacOS/agentbaton-desktop`

ARM prints `PASS no native harness identifiers in default application executable` at 2026-10-26 23:29:05.538380 UTC; Intel prints it at 23:33:38.666973 UTC. This is the actual default release-bundle executable, not the debug smoke binary. The reviewer verified the real CI command/result; no release executable or DMG was downloaded/reinspected here. DMG review is assigned separately to the primary. Marker exclusion is not a general security, signing or notarization proof.

## Fingerprints

Paths below are relative to `/workspace/.onboarding/provider-native-b994/`.

| Evidence | SHA-256 |
| --- | --- |
| `arm/verified/native-smoke.json` | `969df8b685a909e6c9c497551be453cd4429453cbd56d3ca7ee45815f8a782e2` |
| `arm/verified/native-runner.json` | `420778039d52fa4b7d743caaa832b6d26957a1b5578e61c7c8345fb65130a2b0` |
| `arm/verified/native-screen.png` | `ebafc1f8e6f32af3022db6025484e4c316cb3164781adde30eeea8a7a0aa4f66` |
| `arm/verified/lane.log` | `549c4117af96dafaf1f49849dd3f45fd7ac7d3a324d2b0f3779f7b0c50955c34` |
| `arm/job.log` | `607951df935a4c7d255521371d0f0e7717bc4347924a4f249930a5c2f599dfdd` |
| `intel/verified/native-smoke.json` | `fc11b4ec73497d66003ae55d6df11350ce8dee8d6038237f334036ad618c7e02` |
| `intel/verified/native-runner.json` | `420778039d52fa4b7d743caaa832b6d26957a1b5578e61c7c8345fb65130a2b0` |
| `intel/verified/native-screen.png` | `05babb19d97c854edb87982976275bcf64c0f8cddd7881ebfb0139281fbe98c3` |
| `intel/verified/lane.log` | `f7ec77c5283f4579c27dc9e4c1465e73fa86af159e747767321badc490d38602` |
| `intel/job.log` | `8d8c39aa086f6448270500426ab1553b7fd5ea57581ee628af65c3823ece614d` |

`verification.json` contains all 23 downloaded-file hashes, actual checkout mappings and exact-source fingerprints; SHA-256 `64d6cee063c68a7e16cbb42122c3df69f1e2a4b0f844928e0ca6e9f86704b826`.

| Source at b994 | SHA-256 |
| --- | --- |
| `apps/desktop/src-tauri/src/bin/native_smoke.rs` | `8eb1d8114d60a86eaabf4d316a9f4d7931b1c463a13b1624dee7ed65af19d31f` |
| `scripts/native-run.py` | `b1aa421c9b98f3efe449a64b317566e131385f26f65b4c07dca01203c6f0d46e` |
| `scripts/native-macos-smoke.sh` | `d2228f8a325a386bf83171d41d5616c625fdfd5c16fa17494488e10329fc0886` |
| `scripts/native-audit-release.sh` | `dd60a45803ebc66c8dc5a19bba24da35b6e20716589b332314c7d1ecc4fa52d0` |
| `.github/workflows/product.yml` | `dae8b93309c42038015946557454fe45e1709ea3248cb766f5731c2decb56220` |
| `.github/workflows/environment-preflight.yml` | `3bcfd3051e2995de0e49bff88861fd9972a3f805a7cdc9f9bd13c43d4d0e668c` |

Both native reports explicitly leave dialogs `not_run_manual_acceptance_required` and Provider/models `not_run_not_authorized`. The harness does not test a committed finalize action followed by confirmation-journal failure, native owner approvals, active Provider work surviving ordinary quit/sleep/login, new data-validation UI, user-machine installation, minimum supported macOS, OS sandbox enforcement or production release. Old failures remain failed historical evidence. PR8 must remain pending the separate approval-outcome investigation and exact-source validation/review of any correction; successful native draft smoke cannot supersede that boundary.
