# Independent schema7/Data native evidence review — 2640e6c

Verdict: **PASS for the defined dual-Mac native draft/reopen lifecycle plus the new read-only Data list/empty-control checks at exact `2640e6c9af2568c2731019cab9d16423fd45ceae`, including both actual default-release audits.** Both Mac host test suites also passed the required schema7/Data/approval regressions. This is standalone branch evidence: PR11 remains stacked/draft and needs the intended main ancestry, exact target update and fresh final candidate CI before integration. It does not establish native owner-dialog approval, data-policy approve/run or scientific correctness.

The reviewer independently queried API records, downloaded both ZIPs and all seven complete job logs, checked ZIP safety/digests/reports/source bindings, traced the new native commands and DOM checks, and visually inspected both PNGs. Evidence is isolated at `/workspace/.onboarding/data-native-2640/`. Only this document was added in the worktree; no source, lock, index, stage, commit, push, merge or remote mutation was performed. No Cargo run competed for a shared target, and no real Provider/account/model call was made. All earlier failures remain preserved.

## Exact CI state and source

Product [40430134077](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40430134077) and Environment [40430137922](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40430137922), attempt 1, are **workflow_dispatch**, head `2640e6c9af2568c2731019cab9d16423fd45ceae`. All seven complete raw logs actually check out that exact source; both native report commits and artifact names/metadata agree. There is no synthetic PR merge checkout in these runs.

The first independent Product API snapshot showed Intel building the application with release audit still pending. That snapshot and the intermediate in-progress records were retained. Only after separately fetching `product-completion-run.json`/`product-completion-jobs.json`, observing completed/success and reading Intel's complete job log was the full Product/build/audit lane accepted. The final APIs establish all seven jobs completed/success:

| Job | ID |
| --- | --- |
| Product / macos-15-intel | `121945512949` |
| Product / macos-15 | `121945513129` |
| Product / Rust | `121945513140` |
| Product / UI | `121945513216` |
| Environment / ubuntu-24.04 | `121945526393` |
| Environment / macos-15 | `121945526667` |
| Environment / macos-15-intel | `121945526695` |

The Environment checks remain infrastructure-only. Actual Mac Product logs report macOS 15.7.9/24G830; ARM image macos-15-arm64 `20260907.0337.1` and Intel image macos-15 `20260824.0482.1`. Native reports identify aarch64/x86_64. This proves neither earlier minimum macOS support nor user-machine installation.

| Architecture | Artifact | Verified ZIP SHA-256 |
| --- | --- | --- |
| ARM | `12701228835` | `1fb6997c51b4344a0a32952694bb5097f56f710dff6246355f0f6a5e278a4bda` |
| Intel | `12702086063` | `fdd12ffb0dbb3c3372f628ea095a314664aba82fa5ba72b7d213119d30189bbd` |

Each ZIP matches both the supplied value and independently retrieved GitHub digest metadata, with matching run/head/name and no expiry. Exactly five regular safe relative files per ZIP were checked before extraction: lane.log, native-run.log, native-runner.json, native-screen.png, native-smoke.json. No absolute/traversal/symlink entries were accepted; all ten native files were fingerprinted. Both native-run.log files exist and are empty.

## New read-only Data checks are actual execution

Both reports contain strict boolean true for `dataContractLists`, `dataContractEmptyControls` and `readonlyDataNoMutation`. Source tracing establishes these are emitted only after the following actual operations succeed, rather than being prefilled receipt claims:

- FIRST invokes production native `data_validation_policies_list` and `data_validations_list` for the actual UI-created task. The fixed Tauri handlers map them to authenticated Core `data_validation_policies.list`/`data_validations.list`; both observed results must be empty arrays for the fresh task.
- FIRST clicks the real Artifacts tab, finds the real Data panel, requires its details to start folded, expands them, and waits until the actual form fieldset is enabled. `DataContractValidation` keeps that fieldset disabled until both scoped list reads succeed; absence of a loading label alone cannot satisfy this condition. The script also refuses any alert and requires the actual approval button to stay disabled without an eligible stopped session.
- It compares the real daemon sequence before and after those reads/DOM operations, and only then emits the three true fields. It never clicks approval or invokes data run/revoke. The macOS shell postcheck independently requires all three fields; the generic runner's common report checks alone were not treated as sufficient Data evidence.

This establishes actual native read-only lists and honest empty controls for a draft task. It establishes neither a current stopped-session data approval, an OS owner-dialog click, a native data-contract run/receipt nor reproduction-script execution. Exact metadata/schema/range checks remain distinct from algorithm/scientific correctness, representative sampling, whole TaskAccepted and integration authority.

## Native lifecycle, host regressions and screenshots

Both reports pass the actual runner's common report validator plus the explicit strict Data-field checks. First-window DOM/composer/rendering/idempotency/invalid-input/journal checks are true. The reopened phase confirms DOM, the same task, persisted restart, draft cancellation and no-attempt-dispatched stop evidence. All nine expected diagnostic stages appear once in order through reopened_report_received.

| Architecture | Same first/reopened task | Actual first-window PNG |
| --- | --- | --- |
| ARM | `839580ba-3cb6-42f9-8c9a-33165afe23f1` | 1136×793 |
| Intel | `58f02d72-7e3b-4134-b749-c000e7ea441c` | 1292×912 |

Both PNGs were actually viewed. ARM captures the connected Conversation/Draft page; Intel captures the real Artifacts layout with the Data contract heading and scope warning visible. Neither image is claimed to show all controls, an approved policy, a completed data run or the reopened window. DOM/report assertions supply the additional read-only/control/reopen evidence; screenshot content is not invented to match a desired tab.

Both records confirm window-close daemon survival, second-writer refusal, owned restart and owned cleanup. Both runner JSONs show exit 0, timeout false, owned_group_cleanup true and passed, without observation/cleanup/evidence errors. SIGTERM and SIGKILL both report observed_quiescent on actual Darwin. Both lane logs show six native-bin units passing and the final actual macOS WebView/IPC/persistence/cleanup PASS line.

Both complete Mac logs independently establish these current-source test results:

| Actual execution on each Mac architecture | Result |
| --- | --- |
| Core unit/integration suite | 111 top-level pass: 36 library + 3 approval outcomes + 7 execution + 5 IPC + 2 migration_v3 + 2 migration_v5 + 2 migration_v6 + 3 migration_v7 + 13 policy + 3 project IPC + 10 registry + 17 sessions IPC + 8 storage; nested FIFO child excluded |
| Data library integration cases within Core | all 13 named `data_validation::tests` pass, including actual authenticated IPC, failed receipts, policy/root/spec/writer drift, raw/count byte limits and journal atomicity |
| Owned-worker Data busy guard | 1 pass before any file validation |
| schema7 migrations | 3 pass: raw document/blob private backup preservation, failed-upgrade rollback/unknown-data preservation, future-v8 byte preservation |
| Approval-outcome guard tests | 3 pass; existing unknown controls and confirmed probe-refusal same-proposal recovery |
| Native host library | 8 pass, including Data-vs-integrity approval distinction/denial and absence of renderer plugin privileges |
| Native smoke binary | 6 pass |
| Frontend Vitest | 129 pass, including 21 DataContractValidation cases |

These are real Mac CI executions read from logs, not a new local Linux Cargo run by this reviewer. Host tests/Vitest are not themselves native owner-dialog tests; the separate real WebView read-only workflow is described above.

## Actual default-release audits

Both Mac APIs mark the native lane, application build, actual release audit and packaging successful. Full logs show:

`bash scripts/native-audit-release.sh apps/desktop/src-tauri/target/release/bundle/macos/AgentBaton.app/Contents/MacOS/agentbaton-desktop`

ARM prints `PASS no native harness identifiers in default application executable` at 2026-10-27 01:10:49.477389 UTC; Intel at 01:19:01.517465 UTC. These audits inspect the actual default release-bundle executable, not the debug harness. The reviewer read the real CI commands/results; DMGs/release executables were not independently downloaded/reinspected here. The primary reviews DMGs separately. Marker exclusion is not signing/notarization, general binary-security proof or production approval.

## Fingerprints and remaining gates

Paths below are relative to `/workspace/.onboarding/data-native-2640/`.

| Evidence | SHA-256 |
| --- | --- |
| `arm/verified/native-smoke.json` | `c25534acf3011631cd0be182aa9a5c4cc8c953e9db56fe6acd3761d5ef408e5d` |
| `arm/verified/native-runner.json` | `420778039d52fa4b7d743caaa832b6d26957a1b5578e61c7c8345fb65130a2b0` |
| `arm/verified/native-screen.png` | `c81d3578d41f2753210b522b6287b02ec7bd017aec7312c59e72b4d1e556fd2e` |
| `arm/verified/lane.log` | `5ce9f8bac657239fafc0f37913146f01a0341a0fdb744e04dd661beb1ecdfd53` |
| `arm/job.log` | `ca5cc5434f0a0ec7afd0d974a058d99135ce48a06face599b9324f2109ba8cdb` |
| `intel/verified/native-smoke.json` | `aa12346a5f9cd4b2b6ecb277e4a2967ca17dbe9521bfcb06686cbac703f41570` |
| `intel/verified/native-runner.json` | `420778039d52fa4b7d743caaa832b6d26957a1b5578e61c7c8345fb65130a2b0` |
| `intel/verified/native-screen.png` | `629df90f1995c2356e4c31313b59a9e709c4b6af1e2d40dcdee3a02eb59d8cf0` |
| `intel/verified/lane.log` | `165952d087038e9502d9c66b3ab31a6544f6b2ec86c0ddad9675040cb972aeb2` |
| `intel/job.log` | `659431bcd1fb68605449ab7dcdc29e2c5483ab38c691822473e2fece78f11cb2` |

The complete 29-file download manifest, all seven actual checkout mappings and 16 source/lock SHA-256 bindings are saved in `verification.json`, SHA-256 `c9c5f76498d84d09edae0a9b86b7bf52babba15d4204a7fde3ab4b3540e14dd4`. Each source was matched byte-for-byte to committed 2640, including these important native/Data files:

| Source | SHA-256 |
| --- | --- |
| `apps/desktop/src-tauri/src/bin/native_smoke.rs` | `81dacf1b3c39d1fcde3b3012809720cf1d35c0d6caf63b0d99138cd165eff5a1` |
| `apps/desktop/src-tauri/src/lib.rs` | `bae39101810b40ceae53b5d82001825f5dd59725c42aed7e883486a9676f4293` |
| `apps/desktop/src-tauri/src/approval.rs` | `c4111d08229607f2c5b33962f79af537f1723f865d39e15b1a2fe8116321a1f8` |
| `scripts/native-macos-smoke.sh` | `b1ae9e6f083baf152a44b09f69278e9d2ed12bf6e680ba9d8f33d659b204120b` |
| `crates/core/src/data_validation.rs` | `5c5ee6842f1342e95f9e19d39591c25c0ec5716d89f30e3384b0967ce670b5ce` |
| `crates/core/tests/migration_v7.rs` | `6afbd809d0320b4950475da9720edbec840eb2c7a7ce69989ac6952a78d3c4c9` |
| `apps/desktop/src/DataContractValidation.tsx` | `6baa6fab529327597da6467462c9732b3a7c679cb3251f5924af6208b6b73721` |

Both native reports retain `native_dialog=not_run_manual_acceptance_required` and `provider_models=not_run_not_authorized`. No scientific/algorithm validation, actual native data approval/run, real vendor/model/account, ordinary install/sleep/login/active Provider recovery, OS sandbox enforcement, minimum-OS qualification or production delivery is established. The separate command-validator F1 work is not this source or evidence. PR11 must still follow PR10's actual main integration and its intended target update with fresh exact-candidate CI/review; this standalone success cannot be inherited by a different later tree or authorize direct merge.
