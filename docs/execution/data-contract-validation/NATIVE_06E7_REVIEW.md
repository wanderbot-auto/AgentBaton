# Independent Data main-target native review — 06e7fa9

Verdict: **PASS for the exact-source native/build evidence gate at `06e7fa9a9f87c5e69564bc2a1afd97418d762d8d`.** Both manual workflows, dual-Mac read-only Data/native lifecycle, required host regressions and actual release audits passed. This report does not perform a protected merge or establish whole-product/native owner-dialog/data-approve/run/scientific acceptance.

The reviewer independently fetched run/job/artifact/PR records, both ZIPs and all seven complete logs, checked their source binding and report validator, and viewed both actual PNGs. Only this document was added in the worktree. No source, lock, index, stage, commit, push, merge, Cargo/shared-target use or real Provider/model/account call occurred. Downloads are isolated at `/workspace/.onboarding/data-native-06e7/`; previous 2640 and failed evidence remains intact.

## Source, CI and protection boundary

Product [40432924233](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40432924233) and Environment [40432928403](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40432928403), attempt 1, are both workflow_dispatch, completed/success, exact head 06e7fa9. All seven raw logs actually check out that same full commit, and both native reports/artifact metadata/names agree. These are fresh current-candidate results, not inherited 2640 test outcomes.

| Successful job | ID |
| --- | --- |
| Product / macos-15 | `121954716755` |
| Product / Rust | `121954716888` |
| Product / UI | `121954716912` |
| Product / macos-15-intel | `121954717012` |
| Environment / ubuntu-24.04 | `121954727198` |
| Environment / macos-15 | `121954727633` |
| Environment / macos-15-intel | `121954727654` |

The independently read PR11 snapshot is open/non-draft, head 06e7fa9, base `febe7c74f62aad1f767aa60a0eab5e3fc0529dcb`. A local read-only ancestry check confirms that base is contained in the head. The same API snapshot reports **mergeable_state=blocked**. This report does not infer its cause or declare branch protection cleared: the primary must resolve/verify the actual required head/base/check conditions through the guarded merge path.

The duplicate pull_request runs `40432923340` and `40432923530` are completed/cancelled, not failures renamed to success. Their cancellation history under `/workspace/.onboarding/pr11-06e7/` was left untouched; fresh independent API snapshots are retained beside these downloads. The completed manual runs above supply the accurate current evidence and do not erase cancelled history. Environment remains infrastructure-only.

| Architecture | Artifact | Independently verified ZIP SHA-256 |
| --- | --- | --- |
| ARM/aarch64 | `12702214543` | `5e2edc02b0df4ad892fce695bc9efe7523c9a93f8282360fb84b0872b95f0bf8` |
| Intel/x86_64 | `12703615153` | `4ec95f7a5b7b3aa8fc8c96da35123d09de5f338d1a28a11c4bfe2d390202f4cb` |

Both ZIP hashes match the supplied values and independently fetched GitHub digests. Both have exactly five regular safe relative members, with no absolute/traversal/symlink entry: lane.log, native-run.log, native-runner.json, native-screen.png and native-smoke.json. All ten files were fingerprinted; the two native-run.log files exist and are empty.

## Actual Data/native checks

Both reports have status passed, os macos and commit 06e7fa9, pass the exact committed common runner validator, and contain strict boolean true for **dataContractLists**, **dataContractEmptyControls** and **readonlyDataNoMutation**. The committed FIRST and shell source hashes match the independently traced 2640 source: the two real fixed Tauri list calls must return empty arrays; the actual Artifacts/Data panel starts folded, is expanded and waits for its fieldset to become enabled after successful scoped reads; no alert is allowed; approval must remain disabled without a stopped session; the real daemon sequence must be unchanged. The three true fields are emitted only after those assertions, and the shell requires them. This is actual read-only native/DOM evidence, not a preset approval or data-run result.

Both first-window reports confirm real DOM/composer/task rendering, idempotency, invalid input and journal. Both reopened reports bind the same task, DOM, persisted restart, draft cancellation and no-attempt-dispatched evidence. All nine expected stages appear once and in order. Window-close daemon survival, second-writer refusal, owned restart and cleanup are true. Both runners report exit 0, timeout false, owned_group_cleanup true and passed with no observation/cleanup/evidence error; both signals report observed_quiescent on actual Darwin.

| Architecture | Same first/reopened task | Actual first-window PNG |
| --- | --- | --- |
| ARM | `1e9e1870-d85f-4082-bab8-028b3268eddf` | 1136×793 |
| Intel | `adee0a64-b66b-432c-ada9-fab4e6669ef4` | 1292×912 |

Both PNGs were viewed. ARM shows the connected Conversation/Draft page; Intel shows the real Artifacts layout and Data contract scope warning. Neither is relabeled as a complete control screenshot, approved policy, successful data run or reopened image.

Each Mac's complete current log confirms **Core 111 top-level tests**, including 13 Data tests, the owned-worker busy guard, three schema7 migrations and three approval-outcome regressions; the additional nested FIFO child is excluded. Native host **8**, harness **6**, and frontend Vitest **129** (including Data panel 21) pass on both architectures. These are real current Mac CI results read from complete logs, not local Linux runs or an OS owner-dialog click. The common native workflow does not execute a Provider or a reproduction script.

Both actual default application build/audit steps succeeded. Each log runs `bash scripts/native-audit-release.sh apps/desktop/src-tauri/target/release/bundle/macos/AgentBaton.app/Contents/MacOS/agentbaton-desktop`. ARM prints `PASS no native harness identifiers in default application executable` at **2026-10-27 01:44:09.007987 UTC**, Intel at **01:49:52.301366 UTC**. These audit the default release executable, not the debug harness. The primary independently reviews DMGs; the reviewer did not download/reinspect release executables or DMGs here. Marker exclusion is not signing/notarization or general binary-security proof.

## Fingerprints and scope

All evidence paths below are relative to `/workspace/.onboarding/data-native-06e7/`.

| Evidence | SHA-256 |
| --- | --- |
| `arm/verified/native-smoke.json` | `7a2fe79e45aa8236421e97ab0cd7d9731fd1c79427af0e3c58e23d4b31017796` |
| `arm/verified/native-runner.json` | `420778039d52fa4b7d743caaa832b6d26957a1b5578e61c7c8345fb65130a2b0` |
| `arm/verified/native-screen.png` | `06f16566d36e529d4cdc1a746c1ab24ca682807def9f2c8f51454f846c90fde3` |
| `arm/verified/lane.log` | `8c77c1f08f5a6d5c007fd183b4692f426f874a9794b1ec9c765903aafcc379e8` |
| `arm/job.log` | `972b98279c0ce5d84509c37cdbbcf8d98229c3d4085939702cd54387cf2fbcdc` |
| `intel/verified/native-smoke.json` | `d324bc0b35df0b5e8eb7a2edcad3166ff040ab44b1dc0135cf96fa36921bafb0` |
| `intel/verified/native-runner.json` | `420778039d52fa4b7d743caaa832b6d26957a1b5578e61c7c8345fb65130a2b0` |
| `intel/verified/native-screen.png` | `9f10b5d730ba92164576d4bd7ac30524edf06e00083734a5c23e66966160a204` |
| `intel/verified/lane.log` | `95ff3a6b1d3c3f7125396a71a79a6c699e5da9832eb9db2161d39263194c77df` |
| `intel/job.log` | `89b4a4ba78d9a8eb0484e54a8f833a5045144b1a61d9fe9ce29d707549c5cda2` |

The complete 28-file download manifest (including both cancelled API snapshots), seven checkout mappings and 16 committed-source/lock SHA-256 bindings is `verification.json`, SHA-256 **dd10e20576f8700b07fa888e8c410d1a6747ae5b823b5a1e052209e09c07aba1**. Every bound source was read from the exact committed object and checked against the unchanged worktree; all 16 bindings also match the previously statically reviewed 2640 sources. Current CI execution was checked anew. Key source fingerprints:

| Source | SHA-256 |
| --- | --- |
| `apps/desktop/src-tauri/src/bin/native_smoke.rs` | `81dacf1b3c39d1fcde3b3012809720cf1d35c0d6caf63b0d99138cd165eff5a1` |
| `apps/desktop/src-tauri/src/lib.rs` | `bae39101810b40ceae53b5d82001825f5dd59725c42aed7e883486a9676f4293` |
| `scripts/native-macos-smoke.sh` | `b1ae9e6f083baf152a44b09f69278e9d2ed12bf6e680ba9d8f33d659b204120b` |
| `crates/core/src/data_validation.rs` | `5c5ee6842f1342e95f9e19d39591c25c0ec5716d89f30e3384b0967ce670b5ce` |
| `crates/core/src/ipc.rs` | `8084d7e193e84e425d6d36518bcb617ab8513aa19b6bfe4a066a95f0dfbbcd64` |
| `crates/core/tests/migration_v7.rs` | `6afbd809d0320b4950475da9720edbec840eb2c7a7ce69989ac6952a78d3c4c9` |

Both reports explicitly retain native_dialog=not_run_manual_acceptance_required and provider_models=not_run_not_authorized. Real native owner approval/data-policy approve/run, algorithm/scientific correctness, whole TaskAccepted, integration authority, active Provider recovery across sleep/login, OS sandbox enforcement, minimum-OS qualification and production release remain separate gates. The command-validator library and later Core/UI/native changes are not accepted by this evidence. The native evidence gate is clear; branch protection and the primary's final head/base/DMG checks remain independently required before PR11 integration.
