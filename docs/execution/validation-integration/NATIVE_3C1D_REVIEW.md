# Independent PR10 native evidence review — 3c1cd1a

Verdict: **PASS for the defined scripted macOS WebView/private-daemon lifecycle and actual default-release marker audits on Apple Silicon and Intel at exact source `3c1cd1a26b99350f13d14645118bb0a72f87b1de`.** This establishes current schema6 source compiling/testing and running the bounded draft-task native workflow. It does not establish native artifact-policy approval/run UI acceptance, whole-task acceptance or complete Design v0.2.

The reviewer independently fetched GitHub run/job/artifact metadata and full Product job logs, downloaded both ZIPs, checked their digests and safe members, validated the reports with the exact source's `scripts/native-run.py::native_report_error`, and visually inspected both actual PNGs. Downloads are isolated at `/workspace/.onboarding/validation-native-3c1d/`. Only this review document was added in the worktree; no source, locks, index, commits, refs or remote state were changed. Earlier failed CI/artifacts were not altered. No real Provider/account/model call was made.

## Exact checkout and checks

Product [40417144235](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40417144235) and Environment [40417148162](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40417148162), attempt 1, are both completed/success, `workflow_dispatch`, branch `codex/validation-integration`, at the exact 3c1cd1a head. API records show all seven jobs completed/success:

| Job | ID |
| --- | --- |
| Product / Rust | `121902212025` |
| Product / UI | `121902212234` |
| Product / macos-15-intel | `121902212311` |
| Product / macos-15 | `121902212315` |
| Environment / macos-15-intel | `121902233786` |
| Environment / ubuntu-24.04 | `121902234107` |
| Environment / macos-15 | `121902234224` |

All four downloaded Product logs report `git log -1 --format=%H` as `3c1cd1a26b99350f13d14645118bb0a72f87b1de`; the native report commits, artifact names and artifact API run/head associations agree. The three Environment checks remain infrastructure checks; workflow source is unchanged from reviewed 9edf. These dispatch results do not by themselves prove branch-protection requirements or a different merge candidate.

Both actual macOS jobs ran on macOS 15.7.9 (24G830). The ARM image is `macos-15-arm64`, version `20260907.0337.1`; Intel image is `macos-15`, version `20260824.0482.1`. Native reports identify aarch64 and x86_64 respectively.

| Architecture | Artifact | Independently verified ZIP SHA-256 |
| --- | --- | --- |
| ARM / aarch64 | `12696350189` | `eba833b28fb3c879ad52d456fe6393a7db3668831903f167ee77e3bf6df0dad9` |
| Intel / x86_64 | `12696114453` | `2139a46c373c8f5d3be0f4caa2b91a4a58d12cfedd50daa9ff41f45ed5030783` |

Both SHA values match the supplied expectations and independently retrieved API digest metadata. Each ZIP has exactly five regular, safe relative members, with no absolute/traversal/symlink entries: `lane.log`, `native-run.log`, `native-runner.json`, `native-screen.png`, `native-smoke.json`. All ten files were inspected/fingerprinted; both native-run.log files are empty, not missing.

## What the native runs establish

Both native reports are `passed`, `os=macos`, with all required first-window DOM/composer/render/idempotency/invalid-input/journal checks true. Both reopened reports confirm DOM mount, the same task ID, persisted restart, cancellation of the draft and `no_attempt_dispatched` stop evidence. Each report contains all nine distinct diagnostic stages through `reopened_report_received`.

| Architecture | Same task in first/reopened reports | Actual first-window PNG |
| --- | --- | --- |
| ARM | `65d8530a-7594-456f-949e-4c01829fc3e2` | 1136×793 |
| Intel | `9c890e76-6f08-4097-84c3-116a4db9ced0` | 1292×912 |

Both visually inspected images show the actual connected AgentBaton task page, matching native goal text, Draft state and explicit absence of execution evidence. They show the Artifacts tab, but do not show it being opened or any artifact policy being approved/run. These are first-window images; reopened behavior is established by the correlated invoke/report checks, not a second screenshot.

Both reports confirm daemon survival after window close, refusal of a second writer, owned daemon restart and cleanup. Both external runners report exit 0, no timeout, owned_group_cleanup true and status passed, with no observation/cleanup/evidence error. SIGTERM and SIGKILL both report `observed_quiescent`, using the original unreaped group-leader proof on actual Darwin. Each lane log records all six native-bin fixture tests passing and the actual-macOS WebView/IPC/persistence/cleanup PASS line.

The Mac workspace test logs also show both schema6 migration tests passing, including old-record preservation on failed upgrade, and the real authenticated Unix-IPC/server-policy regression passing. Those are native-host Rust tests. The separate WebView script still exercises draft creation, idempotency, invalid input, history, restart and cancellation; it never invokes artifact-policy approve/run or an owner dialog. Current-source schema6 compilation/tests and a real window must not be relabeled as automatic end-to-end native artifact-validation UI acceptance.

The downloaded Ubuntu Rust log separately confirms the authenticated policy IPC regression. The UI job reports 79 tests passing and its real-daemon Chromium lane reports 26 checks/12 screenshots. That browser lane's Artifacts coverage is read-only listing and honest absence of fabricated validation; it is separate from native WebView evidence. This review did not download/re-review that browser artifact or replace the existing Linux policy/browser evidence with a macOS UI claim.

## Actual default release audit

Both job APIs mark the default application build and `Audit the actual default release application for test controls` successful. Both full Mac logs show the command on the actual default release-bundle executable:

`bash scripts/native-audit-release.sh apps/desktop/src-tauri/target/release/bundle/macos/AgentBaton.app/Contents/MacOS/agentbaton-desktop`

ARM prints `PASS no native harness identifiers in default application executable` at 23:13:18.869745 UTC; Intel prints it at 23:16:48.002575 UTC on 2026-10-26. This is the actual default release application audit, not the debug smoke binary. The reviewer inspected its real CI results; the release executable/DMG was not independently downloaded or inspected in this review (the primary reviews DMGs separately). Marker exclusion does not prove signing, notarization or general binary security.

## Fingerprints

All paths below are relative to `/workspace/.onboarding/validation-native-3c1d/`.

| Evidence | SHA-256 |
| --- | --- |
| `arm/verified/native-smoke.json` | `72369909870cfc0c06823961d929b9f95ebd510a9d16c6ced2f616338d200ab2` |
| `arm/verified/native-runner.json` | `420778039d52fa4b7d743caaa832b6d26957a1b5578e61c7c8345fb65130a2b0` |
| `arm/verified/native-screen.png` | `be2b65f47eaa64e62aa884b3add24217bda4923d3614b0c532dfc5223a7eb123` |
| `arm/verified/lane.log` | `160732cce47374a693c4cdf0470d28adf455964ed007dfeac2822711c8001600` |
| `arm/job.log` | `4eac7d9a266277061286a988c9cb75f6533bd0914318c193f67aa6b024692b00` |
| `intel/verified/native-smoke.json` | `f476c8adafa887ce451fe42d592ac7eb5f6299e8cc188fc0b035b667c8274eb3` |
| `intel/verified/native-runner.json` | `420778039d52fa4b7d743caaa832b6d26957a1b5578e61c7c8345fb65130a2b0` |
| `intel/verified/native-screen.png` | `ffc7efc90641418b52ecc09bed4c24747ce2976ebebfc2ee399712e17399240e` |
| `intel/verified/lane.log` | `62ac42e066668666bfe41dd39dabed4d0def1d19833208d129911eb52c064d94` |
| `intel/job.log` | `b9b4a412cb79883777bd49565c0c543f7e805d86c00f66f6e1b3f86ccb486792` |

The complete 22-file download fingerprint manifest, including the two empty native-run.log hashes, metadata, ZIPs and Ubuntu logs, is `verification.json`, SHA-256 `c503854fba3b3d0f056e4314c09487dadf2c015cb11888d23f3e70390ad7aaf7`. It also records these exact-source hashes:

| Source at 3c1cd1a | SHA-256 |
| --- | --- |
| `apps/desktop/src-tauri/src/bin/native_smoke.rs` | `8eb1d8114d60a86eaabf4d316a9f4d7931b1c463a13b1624dee7ed65af19d31f` |
| `scripts/native-run.py` | `b1aa421c9b98f3efe449a64b317566e131385f26f65b4c07dca01203c6f0d46e` |
| `scripts/native-macos-smoke.sh` | `d2228f8a325a386bf83171d41d5616c625fdfd5c16fa17494488e10329fc0886` |
| `scripts/native-audit-release.sh` | `dd60a45803ebc66c8dc5a19bba24da35b6e20716589b332314c7d1ecc4fa52d0` |
| `.github/workflows/product.yml` | `dae8b93309c42038015946557454fe45e1709ea3248cb766f5731c2decb56220` |
| `crates/core/src/storage.rs` | `1dba48a47b42bbb58498d7fd746f1fe8fa8d10d1e31efe703cac11053ac61b7b` |
| `crates/core/src/validation.rs` | `b1e507f8c23c8e66c1bed99669bdc2a14578e1e943ebaf179edbf2ba796c3c51` |
| `crates/core/tests/migration_v6.rs` | `91fc8a18ba5be7895f48aed64bd25111b34fa0ac37edf10e0ce871e8a55e093c` |
| `Cargo.lock` | `d07f4bc79c5653a3bb641d68dff0cad94bd9ade97ca443ff7d1f993541064a02` |
| `apps/desktop/src-tauri/Cargo.lock` | `8bfdfd48228f3a26a860747d3d08aea8814f794f9218d721142a5e3686442aad` |

Native dialog evidence explicitly remains `not_run_manual_acceptance_required`, and Provider/model evidence remains `not_run_not_authorized`. Ordinary installation, owner dialogs/artifact-policy native UX, active Provider work across quit/sleep/login, real vendor compatibility, minimum supported macOS, OS sandbox enforcement, signing/notarization and production release remain separate gates. All earlier failures remain historical failures; these successful current-head runs do not retrospectively prove their cause or complete Design v0.2.
