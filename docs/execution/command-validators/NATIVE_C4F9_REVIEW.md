# Independent historical command-library/native evidence — c4f9ccd

Verdict: **PASS for the historical command-library platform tests and existing Data read-only native/build lane at exact `c4f9ccd353f2ea752781057026dfa33328be94e7`.** This does not approve direct PR12 merge or the subsequent Command Core/UI/native integration. The native window in this run did not invoke a command-validation UI or command-validation RPC.

The reviewer independently fetched API records, both ZIPs and all seven complete job logs; checked digests/safety/reports, read exact committed sources and viewed both real PNGs. Only this document was added. No source, lock, stage, commit, push, merge, Cargo/shared-target work or real Provider/model/account call occurred. Downloads are isolated at `/workspace/.onboarding/command-native-c4f9/`; historical failures remain preserved.

**Source isolation:** at review start, this worktree contained uncommitted native_smoke/browser/shell changes preparing new Command checks; the primary committed later integration work while this historical review ran. Every source fingerprint, FIRST/dispatcher inspection and the report-validator code used here came from `git show c4f9ccd353f2ea752781057026dfa33328be94e7:<path>`, never those mutable files. The committed FIRST has the three old Data checks and no Command check. Committed native lib/Core IPC expose no command-validation route; command_validation is a workspaces library module. New Core8/UI182/native11/Command flags therefore are not part of this evidence.

## Exact CI and artifacts

Product [40432398334](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40432398334) and Environment [40432402606](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40432402606), attempt 1, are completed/success, workflow_dispatch, head c4f9ccd. All seven independently downloaded logs actually check out that same full commit; report commits, artifact names and API metadata agree. Environment remains infrastructure-only.

| Successful job | ID |
| --- | --- |
| Product / Rust | `121952983882` |
| Product / macos-15 | `121952984381` |
| Product / macos-15-intel | `121952984990` |
| Product / UI | `121952986166` |
| Environment / macos-15-intel | `121952996252` |
| Environment / macos-15 | `121952996328` |
| Environment / ubuntu-24.04 | `121952996523` |

| Architecture | Artifact | Verified ZIP SHA-256 |
| --- | --- | --- |
| ARM/aarch64 | `12702752500` | `569b1786fe688f8acc39f02ff1f0005d6d6c8e74d6a549cd509f47902ba8016d` |
| Intel/x86_64 | `12702673789` | `9ef30bf2365c006bb415223251259f8862ef92ef51df3d958f0e7a33fe125c37` |

Each ZIP matches the supplied digest and independently fetched GitHub digest. Five regular safe relative members per archive were checked before extraction: lane.log, native-run.log, native-runner.json, native-screen.png and native-smoke.json; no absolute/traversal/symlink entries. All ten native files were inspected/fingerprinted; native-run.log is present and empty in both.

## Actual command-library tests, separate from the GUI

Both complete Mac logs record **13 command_validation tests passing**, including actual fixed native system program/expected-exit execution, literal-argument approved-script handling, script drift, cancellation/timeout/capture bounds, output/subject binding, regular-file safety and refusal of unsupported sealing/writable program chains. These are actual ARM/Intel program executions, not Linux cross-compilation evidence. They prove the tested supported paths and fail-closed negatives, not unrestricted arbitrary-program support or an OS filesystem/network/descendant sandbox.

Both Mac logs record **4 compile-fail doctests passing**: two for command authority/command-only acceptance and two existing Data authority/acceptance examples. Four does not mean four new command doctests. Linux Product Rust separately records **27 top-level command tests** and the same four doctests passing. This includes `linux::delayed_exit_cleanup_and_cancel_observations_cannot_be_accepted`, the actual fixed-source F1 regression. Mac13 is not relabeled as the Linux27 suite, and Linux-only delayed-observation controls are not invented on Darwin. No tests were rerun locally by this reviewer.

## Existing native workflow and build evidence

Both fresh reports pass the validator loaded from the exact c4f committed scripts/native-run.py. Reports have status passed/os macos/commit c4f9ccd and architectures aarch64/x86_64. Both first-window reports establish real DOM/composer/rendering/idempotency/invalid-input/journal checks and strict true for the existing `dataContractLists`, `dataContractEmptyControls`, `readonlyDataNoMutation`. They have **no Command report fields**. The pinned FIRST and shell sources match the earlier reviewed Data read-only implementation, not the later Command FIRST.

Both reopened reports bind the same task and establish DOM, restart persistence, draft cancellation and no-attempt-dispatched evidence. All nine expected stages appear once and in order. Daemon survival after window close, second-writer refusal, owned restart and cleanup are true. Both runners show exit 0, timeout false, owned_group_cleanup true and passed, with no observation/cleanup/evidence error; SIGTERM/SIGKILL both report observed_quiescent on actual Darwin.

| Architecture | Same first/reopened task | Actual first-window PNG |
| --- | --- | --- |
| ARM | `bb2c5626-0471-4ddf-8d4b-806263a7641a` | 1136×793 |
| Intel | `78de7be2-f564-4d6a-b524-892c7fa4543b` | 1292×912 |

Both PNGs were actually viewed and show connected native Conversation/Draft pages with no fabricated execution evidence. Neither is a Command controls screenshot or a reopened image. Data DOM checks/reopen are established through the correlated reports and actual committed script, not invented screenshot content.

Both Mac lane logs have **6 harness units** passing. Complete host/frontend logs retain the old **8 native host / 129 UI** counts. They do not establish the later Command Core8, UI182 or native11 integration. Command execution in the platform unit tests is separate from the real WebView's draft/Data read-only workflow; the latter neither approved nor ran a command policy.

Both actual default application build/release-audit/packaging steps succeeded. Each raw Mac log executes `bash scripts/native-audit-release.sh apps/desktop/src-tauri/target/release/bundle/macos/AgentBaton.app/Contents/MacOS/agentbaton-desktop`. ARM prints `PASS no native harness identifiers in default application executable` at **2026-10-27 01:33:42.619512 UTC**; Intel at **01:43:23.089658 UTC**. These inspect actual default-release executables, not debug harnesses. The primary independently reviews DMGs; this reviewer did not download/reinspect release executables or DMGs. Marker exclusion is not signing/notarization or general binary-security proof.

## Fingerprints and limits

Paths below are relative to `/workspace/.onboarding/command-native-c4f9/`.

| Evidence | SHA-256 |
| --- | --- |
| `arm/verified/native-smoke.json` | `decf47655b803049520a63ec66618bf1312e493e626c7076c0bb7297602edda6` |
| `arm/verified/native-runner.json` | `420778039d52fa4b7d743caaa832b6d26957a1b5578e61c7c8345fb65130a2b0` |
| `arm/verified/native-screen.png` | `1a83bb13f6f3c8bf2a712eceafbce6662b46762bfa009679f5a271cf272fdee8` |
| `arm/verified/lane.log` | `e78efd7a2c7b3a0318358313980e3ddb6716f021670dbd65799b125bfe46546e` |
| `arm/job.log` | `ebc4d9a383d4bec1cb5140c8a5de306992c1467775fe752c19da6e58d40ee708` |
| `intel/verified/native-smoke.json` | `2f5f912f55f9a8528e2b8208ff90d73526afe83395c4acc3ae15d3274dafe677` |
| `intel/verified/native-runner.json` | `420778039d52fa4b7d743caaa832b6d26957a1b5578e61c7c8345fb65130a2b0` |
| `intel/verified/native-screen.png` | `9369a0f6182cebd36f3fb588837d518c7d7d39827177049c1a9acfd3d5412109` |
| `intel/verified/lane.log` | `75b154c8b4a411853987bf7d6748c756daeb3738c7ad581eb3929394435d1fae` |
| `intel/job.log` | `cca63c5c2783947425535f356dc782df25b6ee363c5a650264959f9f38aa90e5` |
| `product-job-121952983882.log` | `769c0efc645bffc802a914bca88826b10ca5f14a8511a3f8fc95768469c5c57a` |

The full 25-file download manifest, all seven actual checkout mappings and 17 **committed-object-only** source/lock SHA-256 fingerprints are `verification.json`, SHA-256 **1e7f7b68a567fb708548ab56e6736a2db27fc8a1c70adf8702bef19e44e4523c**. Key immutable source bindings:

| Committed c4f source | SHA-256 |
| --- | --- |
| `crates/workspaces/src/command_validation.rs` | `d5fa7b28d6a694c4fa8f767a021499fb5c984a68937cfbf939fcf553cbd4e870` |
| `crates/workspaces/tests/command_validation.rs` | `1ec8708817bb72197be4fb44afec25155810e70dc4c0fc6e3973f86fe912cfd5` |
| `apps/desktop/src-tauri/src/bin/native_smoke.rs` | `81dacf1b3c39d1fcde3b3012809720cf1d35c0d6caf63b0d99138cd165eff5a1` |
| `apps/desktop/src-tauri/src/lib.rs` | `bae39101810b40ceae53b5d82001825f5dd59725c42aed7e883486a9676f4293` |
| `scripts/native-macos-smoke.sh` | `b1ae9e6f083baf152a44b09f69278e9d2ed12bf6e680ba9d8f33d659b204120b` |
| `apps/desktop/tests/quiet-canvas-real-ipc.py` | `c13b6fc6323ed68398f3348f64f702fd898cfb22f617afc4a6b5ab2ae15063c3` |
| `crates/core/src/ipc.rs` | `8084d7e193e84e425d6d36518bcb617ab8513aa19b6bfe4a066a95f0dfbbcd64` |

Reports retain native_dialog=not_run_manual_acceptance_required and provider_models=not_run_not_authorized. This historical CI does not establish Command owner confirmation, Core policy persistence/IPC, Command native/read-only controls, command execution initiated through the app, OS isolation, whole-task/code/scientific acceptance, Git adoption or production release. The later Core/UI/native composition and its target/main candidate require independent source review and fresh accurate CI; this successful library evidence is retained as history and is not permission to merge PR12 directly.
