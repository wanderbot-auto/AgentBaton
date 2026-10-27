# Independent PR10 native review — 62465a5

Verdict: **PASS for the current PR10 native/build evidence gate at `62465a57f35b55838bce2064e96eafef8255156a`; no native-evidence blocker remains for a guarded merge of this unit.** Both Mac native runs, six harness units and actual default-release audits passed. This is not full-product acceptance, a merge operation, production approval or acceptance of later Data/command-validator code. The primary retains the final expected head/base, required-check/protection and separate DMG verification steps.

The reviewer independently fetched run/job/artifact records, both ZIPs, all seven full job logs and PR/base/merge-tree metadata; inspected all ten native files, ran the exact source's report validator and visually viewed both PNGs. Downloads are at `/workspace/.onboarding/validation-native-62465/`. Only this new document was written in the repository. No source, locks, index, refs, stage, commit, push, merge or remote mutation occurred; no Cargo/shared-target work or real Provider/account/model call was made. Old 3c1d and failed evidence was preserved.

## Exact source and CI provenance

Product [40430127911](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40430127911), attempt 1, is completed/success, **workflow_dispatch**, head `62465a57f35b55838bce2064e96eafef8255156a`. All four complete Product logs check out that exact head; both native report commits and artifact metadata/names match.

Environment [40430127252](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40430127252), attempt 1, is completed/success, **pull_request**, API head 62465a5. All three complete Environment logs actually check out synthetic merge **`92394fc7af77f4109d3f15f339f21476341211b9`**. Independently fetched commit metadata identifies parents base `afb52be241f7c4cef261f99c04f56caf21f12083` and head 62465a5. Its tree `8387f9a5c485c0fb2414f2f7ff3a60f9ad670527` equals the exact head's tree. This difference in commit identity is retained rather than describing all seven jobs as the same checkout.

The independently read PR10 snapshot was open/non-draft, head 62465a5, base afb52be, mergeable/clean. A read-only local ancestry check confirmed that base is an ancestor of the reviewed head. This snapshot does not eliminate the primary's final race/protection checks.

| Completed/success job | ID |
| --- | --- |
| Product / macos-15 | `121945493829` |
| Product / Rust | `121945494092` |
| Product / UI | `121945494109` |
| Product / macos-15-intel | `121945494150` |
| Environment / macos-15-intel | `121945486995` |
| Environment / ubuntu-24.04 | `121945487204` |
| Environment / macos-15 | `121945487470` |

The Environment jobs remain infrastructure-only. Actual Mac Product logs identify macOS 15.7.9/24G830; reports identify aarch64/x86_64. These results do not qualify minimum macOS or installation on ordinary user hardware.

| Architecture | Artifact | Verified ZIP SHA-256 |
| --- | --- | --- |
| ARM | `12702060631` | `9c650170c988b270c3d5b3adaacdbcaac3bc3dea48904f72b7eefbfaaf825a60` |
| Intel | `12701659234` | `af8e46593860e11571ad514d5678b52efc64b3288e83b003fe3998ee682339f3` |

Both ZIP hashes match both the supplied expectations and independently fetched API digest metadata. Each archive contains exactly five regular safe relative members, with no symlink/absolute/traversal entries: lane.log, native-run.log, native-runner.json, native-screen.png and native-smoke.json. All ten files were inspected/fingerprinted; both native-run.log files exist and are empty.

## Native behavior and current Mac regressions

Both current reports pass `scripts/native-run.py::native_report_error` with status passed, os macos and the exact 62465a5 commit. First-window DOM/composer submission/task rendering/idempotency/invalid-input/journal checks are true. Reopened reports confirm DOM, the same persisted task, restart, draft cancellation and no-attempt-dispatched stop evidence. All nine expected stages appear once and in order through reopened_report_received.

| Architecture | Same first/reopened task | Actual first-window PNG |
| --- | --- | --- |
| ARM | `0a96c46d-3f08-4915-a064-39dee9a62926` | 1136×793 |
| Intel | `7e0adbe2-1bcd-4dd3-b430-d211d9e2ba2b` | 1292×912 |

Both actual PNGs were viewed: connected AgentBaton native Conversation pages show the expected goal, Draft state and honest absence of execution evidence. There is no claimed reopened screenshot or image of owner approval/artifact validation. Reopened behavior is established by the correlated native invoke/report checks.

Both runs confirm window-close daemon survival, second-writer refusal, owned restart and owned daemon cleanup. Both external runner reports show exit 0, timeout false, owned_group_cleanup true and passed, with no observation/cleanup/evidence errors. SIGTERM/SIGKILL both report observed_quiescent on actual Darwin. Each lane log records six native-bin fixture tests passing, no failed/ignored/filtered tests, and the final actual macOS WebView/IPC/persistence/cleanup PASS line.

Fresh complete Mac logs, independently read for both architectures, additionally establish:

- Core library 22 tests and all Core integration suites pass: 94 top-level total, excluding the nested FIFO child. This includes both schema6 migration tests and the real authenticated Unix-IPC/server-policy regression.
- All three approval-outcome tests pass on each Mac, preserving the original unknown controls, confirmed probe-refusal error and same-proposal recovery. They are not inherited from a Linux or earlier native result.
- Native host library has **7** tests passing on each Mac; the separate smoke binary has **6**. The Data branch's eight-host-test count is not substituted here.
- `App.test.tsx` has 12 passing tests on each Mac, within 79 passing frontend Vitest tests. These are macOS-hosted tests, distinct from the actual WebView lane and from native OS dialog acceptance.

No local Cargo repeat is claimed by this reviewer. The native script still tests the bounded draft workflow. It does not approve/run an artifact policy in the native UI or prove native owner-dialog behavior. Actual policy/Unix-IPC regressions and browser read-only Artifacts coverage remain separate dimensions, not automatic native policy-approval acceptance.

## Actual default-release audits

Both Mac APIs mark application build, actual release audit and packaging successful, including the previously pending Intel job. Full logs execute:

`bash scripts/native-audit-release.sh apps/desktop/src-tauri/target/release/bundle/macos/AgentBaton.app/Contents/MacOS/agentbaton-desktop`

ARM prints `PASS no native harness identifiers in default application executable` at 2026-10-27 01:12:45.747477 UTC; Intel at 01:22:19.620123 UTC. This is the actual default release-bundle executable, not the debug smoke binary. The reviewer read those real commands/results; release executable/DMG inspection belongs to the primary's separate evidence. Marker exclusion is not signing/notarization or a general binary-security proof.

## Fingerprints and remaining scope

Paths below are relative to `/workspace/.onboarding/validation-native-62465/`.

| Evidence | SHA-256 |
| --- | --- |
| `arm/verified/native-smoke.json` | `14d8ee19c36f32a1c1ffc5579f1414285ebd1cefd6b198ee13daeefbb756c0ee` |
| `arm/verified/native-runner.json` | `420778039d52fa4b7d743caaa832b6d26957a1b5578e61c7c8345fb65130a2b0` |
| `arm/verified/native-screen.png` | `f2c06b58fa4df1bd53bcb8a1bf9bfd9d96110b65393165cfd4af031c70631a86` |
| `arm/verified/lane.log` | `d8bc2327461b5f5af2f26d48a407ce31c39fa2e0af215bc62554d8809da96c5b` |
| `arm/job.log` | `38f5873cb2146e50c6eb2de5b3e07987ad8d3a0578daf714b54627904e545927` |
| `intel/verified/native-smoke.json` | `0325c0d58c4bbf879e46cabd8ba7ee3967b3335c67115f2e1884396e3e60967e` |
| `intel/verified/native-runner.json` | `420778039d52fa4b7d743caaa832b6d26957a1b5578e61c7c8345fb65130a2b0` |
| `intel/verified/native-screen.png` | `b7444d58a107446ce2f81c9ee27491e9f0a55cecd288fa02c41cc5a8dbe647fd` |
| `intel/verified/lane.log` | `55d83c12da1820cd3e03adb757d49535197817135e7e883ad2d239172eb381b5` |
| `intel/job.log` | `9be49b924c69c88efd7b1aa9448754ed09f7e8c0d871ba6b36f1060c5378b375` |

The 28-file manifest, seven actual checkout mappings and 16 exact committed source/lock hashes are in `verification.json`, SHA-256 `dcdb22bee02a516719337c3d6712ae5cc15071e08f9c5ede95f95c77500e9226`. Key source bindings:

| Source at 62465a5 | SHA-256 |
| --- | --- |
| `apps/desktop/src-tauri/src/bin/native_smoke.rs` | `8eb1d8114d60a86eaabf4d316a9f4d7931b1c463a13b1624dee7ed65af19d31f` |
| `apps/desktop/src-tauri/src/lib.rs` | `6b424769da00eb674b76af98747b207eb8a93adfc58f6abdbf43ba43c8194288` |
| `scripts/native-run.py` | `b1aa421c9b98f3efe449a64b317566e131385f26f65b4c07dca01203c6f0d46e` |
| `scripts/native-macos-smoke.sh` | `d2228f8a325a386bf83171d41d5616c625fdfd5c16fa17494488e10329fc0886` |
| `scripts/native-audit-release.sh` | `dd60a45803ebc66c8dc5a19bba24da35b6e20716589b332314c7d1ecc4fa52d0` |
| `crates/core/src/ipc.rs` | `8d497aae196929d50bca2284fc19f90272c1226ad661defcd4eb0363560b7eab` |
| `crates/core/tests/approval_outcomes.rs` | `dbc6adc0de7e2c086e31a022f52df374cf7fc3c7c6d3f71e1ccc53e3acf96a67` |
| `apps/desktop/src/App.test.tsx` | `194c6c8cc63ed1ab120bde3cdb9283de9d5d263e6198ab674f440d1dc9002e44` |

Both reports explicitly retain native dialogs not_run/manual_acceptance_required and Provider/models not_run/not_authorized. Real vendor compatibility, approval dialogs, active Provider work across ordinary quit/sleep/login, minimum-OS qualification, OS sandbox enforcement, signing/notarization and production release remain separate gates. Earlier 3c1d and all failed records remain historical evidence, not overwritten success. PR11/Data/command-validator and later main-target compositions must use their own exact-source review and CI; the current unit's native success cannot be inherited as whole-product acceptance.
