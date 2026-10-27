# Fixed-command frontend/native bridge independent review

Initial frozen source `6e82390f756dc354f2754bb43b1ff93a72eac8a2`, base `6b19d1c1633c9e52e95f83c1d45c0c40427ee84c`. Read current gate/OWNER-001/002 and latest local protected-main inbox `febe7c74f62aad1f767aa60a0eab5e3fc0529dcb`; no additional authorization inferred. Scope: command component/API/i18n/tests, one App Artifacts mount, namespaced CSS and the seven typed native bridge handlers/approval kind. Core/schema8 and command-library runtime are separate work units. The earlier ff494/e701 narrow reviews remain in PRE_FINALIZE_REVIEW.md and were not overwritten.

## F1 — cancellation cache lock, closed on355d33f

Source review identified unknown cancel -> valid scoped same-run command_failed/evidence:null readback. Core command_failed_before_receipt may legitimately produce this terminal record. The initial refresh resolved only cancel_requested or owned_leader_reaped; the terminal record had neither, and only active-state rows offered a cancellation recovery button. Pending then blocked form/run/revoke with no usable recovery action. Reported promptly to primary/UI author. This is a conservative request-cache UX defect, not permission expansion or proof of process stop. The author fixed only CommandValidation.tsx and its test in frozen `355d33f85b57589338c0bc3d37a159693fd17082`. A fresh scope-valid same-run command_failed read with revision at least the observed cancellation revision now clears the cancellation intent. It does not fabricate stop evidence, release a Core fence or permit a second run of the policy. The retained pre-fix control test fails on the old component (author execution; reviewer read/hash-verified the actual failure log); four new regressions verify terminal/null-evidence unlock and stale/foreign/read-failure refusal until a valid newer terminal read. This reviewer independently reran the complete182-test suite, typecheck and build on the fixed source, all exit0. No unresolved blocking frontend/native-bridge finding remains in the reviewed scope. Reviewer did not modify source/tests.

## Source and wire review

Selection is proposal-only: fixed absolute executable, SHA-bound fixed inputs and optional pinned POSIX stdin script, literal argv, bounded output contracts, safe constant environment and explicit enforcement/time/capture requirements. No renderer program hash/root/cwd/subject/identity/fence/approval/finalize input. Seven native commands match current command-core router/DTO field names: policy list/approve/revoke and run/list/get/cancel. Core shape was inspected while its tests were separately in progress; no new command-Core execution is proved here. Value selection becomes serde-deny-unknown DTO server-side; native owner dialog displays the complete server-derived snapshot and binds one policy run. Existing custom Tauri dispatch and empty renderer plugin permissions remain; no generic shell/terminal/finalize route is added.

Native CommandValidation kind is distinct from integrity/data/grant/connection, displays one-run resources/argv/file/time limits and honest sandbox/descendant/input limitations, rejects cross-kind and oversized review snapshots before confirmation, and finalizes only server proposal ID/revision/key. e701 malformed-before-confirm errors remain not_started; post-finalize unknown/error values remain unchanged. Approve does not start a command; run requires explicit user action under max_runs=1, exact current policy/spec/session and no existing run. Same-key recovery preserves payload/revision, cannot become a second run, and a unique scoped durable run can be reconciled read-only. Revocation affects future use and does not claim cancellation or erase historical records.

Parser rejects missing hashes, traversal/control/oversized/duplicate file roles, unsafe integer syntax/rounding, excessive counts/bytes/time/capture limits, unsupported mode/environment, NUL and oversized UTF-8 arguments. It enforces1..15 fixed inputs,1..16 outputs,1..4MiB per declared I/O file/16MiB I/O total, separately pinned <=64KiB script, <=64 literal arguments/4096 bytes each/16KiB combined, selection <=16KiB, exit codes0..255 unique<=16, wall1..60000ms, cleanup1..1000ms and retained stdout/stderr0..65536 bytes. Empty arguments and literal shell-looking characters are preserved without interpolation. Only LANG/LC_ALL constants C/C.UTF-8/en_US.UTF-8 and optional TZ=UTC are accepted. Each require boolean is preserved rather than silently relaxed; unsupported strict facts remain a server refusal before spawn. Capture limits are explicitly not disk/output quotas and the deadline is not a kernel-interruption guarantee.

History must match task/project/policy/session/workspace/attempt/scope/validator/one-run identities. Duplicate policy runs or malformed/foreign/elevated reads invalidate freshness and cannot authorize writes. Exact known states are labeled; future/prototype enums remain unknown, never acceptance. A passing command label requires actual command evidence, process-started, bounded exit/cleanup, leader reaped and file-integrity pass; it is not scientific correctness, TaskAccepted or Git/adoption/release permission. Unknown workspace outcomes remain fenced; stale task specifications and running/foreign sessions cannot run. Read-only session filtering is not server stop/fence authorization.

Prepared, running and cancel_requested are distinct; cancellation request is never confirmed stop. Owned leader reaped explicitly states descendant containment is not established. Only capture counts/digests/truncation/EOF metadata are displayed; no raw output command channel. Program args and error text use React text/JSON escaping. Process/native configuration/dynamic/transitive limitations remain explicit, not upgraded to OS sandbox claims. The Artifacts panel is folded initially, controls labeled, English/Chinese messages supplied, and CSS changes stay within .command-validation; homepage/composer and existing integrity/data components remain unchanged.

Pending intents retain a cloned form, exact payload/key/revision per task across remount; process-local cache is bounded32 and is not restart-durable. Latest failed/foreign/malformed list reads reset freshness; successful actual read reconciliation may precede an explicit same-request recovery. Effects automatically perform only read-only polling,1-second interval at most70 polls per batch plus initial read; failures/unmount/inactive state halt polling. Manual refresh can restart observation and never auto-runs/cancels. The70-poll epoch regression is exercised; cache32 saturation is source-reviewed, not separately executed.

## Initial independent execution

Cwd apps/desktop, all exit0 on stable initial9-file source snapshot:

- `npm run typecheck`
- `npm test`:178 tests /13 files (129 baseline retained plus49 command cases), jsdom/mocked Tauri transport.
- `npm run build`

Native independent command: `cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml`, exit0,11 tests; zero main/doctests. Existing target `/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target`, CARGO_HOME=/workspace/.toolchains/cargo, RUSTUP_HOME=/workspace/.toolchains/rustup, CARGO_INCREMENTAL=0, CARGO_BUILD_JOBS=2, PKG_CONFIG_SYSROOT_DIR=/workspace/.toolchains/linux-ui/sysroot, PKG_CONFIG_LIBDIR=sysroot/usr/lib/x86_64-linux-gnu/pkgconfig:sysroot/usr/share/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig, LD_LIBRARY_PATH/LIBRARY_PATH prefixed with sysroot/usr/lib/x86_64-linux-gnu, and toolchain bin prefixed to PATH. No new target/dependency install. Native build uses existing older schema7 real sidecars solely as build resources; their hashes are recorded in worker evidence. This is Linux Tauri compilation/pure units, not schema8 command IPC, real owner click or GUI evidence. Worker Clippy/fmt logs were inspected/hash-verified, not independently re-executed.

All9 source and16 worker log fingerprints independently matched. Initial worker two test failures and the preapproval setup failure remain retained. This reviewer did not reduce assertions, change timeouts or skip acceptance checks. No Core/command library/locks/plugin-permission delta in this branch; no real Provider/model/account call, extra cost or production action.

## Limits and pending integration

F1 is closed by the independently verified355d33f follow-up. Actual new Core/native bridge composition, true Unix IPC command lifecycle, native owner dialog, macOS dual architecture/GUI/installation and exact-head protected CI are not proved by these UI mocks/Linux native units. Complete GOAL remains open. No source/lock/test modification, staging, commit, push or subagent by reviewer. Only this REVIEW.md and ignored independent logs are written. The reviewer-created temporary exact symlink to Root node_modules was removed after verifying its target; the earlier worker-owned symlink was never removed by reviewer and actual installed dependencies were untouched.

## Final correction and independent verification

Final frozen source355d33f differs from6e82390 only in the two F1 files. Every current file was checked against that exact git object after the final run; the other seven reviewed source files and all native handlers/kind/permissions/locks remain unchanged. Thus the independently executed native11 result on6e82390 also covers the identical final native source without a redundant native rebuild.

Final commands `npm run typecheck`, `npm test` and `npm run build` all exit0. The complete suite now has182 tests/13 files, retaining all178 prior cases and four F1 regressions. Positive fresh-terminal reconciliation checks editable form, no pending-status label, exactly one cancellation call and no run or leader-reaped claim. Negative stale/foreign/read-failure reads keep the unknown request locked until an actual valid newer read; original run ID/cancel revision remain exact. No timeout, skip or prior assertion was weakened. This is still mocked frontend evidence, not a new Core run.

| Corrected file | SHA-256 |
| --- | --- |
| `apps/desktop/src/CommandValidation.tsx` | `f59204bfa094bb4351904863ae3d161f9fa7038c2844b14b80203fa4623cb1d1` |
| `apps/desktop/src/CommandValidation.test.tsx` | `c70dbf95f41470f09a060968ca8d6db3a4fd6a3f75ea13c125e5114eea0939f9` |

The original16 worker log hashes and9 source hashes were verified on6e82390 before F1. Final correction control/independent logs are separately fingerprinted below; no pre-fix run is mislabeled as final.

## Initial reviewed source SHA-256

| File | SHA-256 |
| --- | --- |
| `apps/desktop/src/CommandValidation.tsx` | `c6f1b9199812b4a6e3a1179636711f4cdeef49fcdfa7fa0e8c3eb8403badf9b3` |
| `apps/desktop/src/CommandValidation.test.tsx` | `d256ed51ae1c2e07f192fbd0d009c08f3714211092514e30871cc17fce7b6f26` |
| `apps/desktop/src/command-validation-api.ts` | `a0174a48ace707b92a0479ff84128c2f8f9cfa4fe3490936242807921e763323` |
| `apps/desktop/src/command-validation-api.test.ts` | `2b42ecda9b7b20f336f241f287ad400696de0077fee75a7d75549c4ca6700171` |
| `apps/desktop/src/command-validation-i18n.ts` | `f33994ffeafc34edc337f8ff754ed5e060ede3890a23f65c6f9db5cec2b53056` |
| `apps/desktop/src/App.tsx` | `8d0b82c93492b7063d63455b46370b0d94c417d5d9f79f3846ae76cdd81bdf08` |
| `apps/desktop/src/style.css` | `2327809663043400ba2ed467bd4108215d480dbc7640f2744887cc377c719aa2` |
| `apps/desktop/src-tauri/src/lib.rs` | `79251d8dff8564b7ec2c14af631b84dbf8562d2b1f30e03e7e242aa83666df7c` |
| `apps/desktop/src-tauri/src/approval.rs` | `73e7ec269bc43fe01e078c618f0dc5c42ead47a023109339ebf6cde48dfe6c16` |

## Independent logs SHA-256

Relative to `/workspace/.onboarding/command-ui-independent/`; initial178 and final182 remain separate.

| Artifact | SHA-256 |
| --- | --- |
| `build-final.log` | `acde005aac8a3680c8897c853e8e91d1398d23da765fb63e0696dacd0532bd7b` |
| `build.log` | `176a305835feec564331e82f55ce203da5c46b03ba6d8935c3ad3c5dd2e664f0` |
| `native-tests.log` | `3f3f6f1401c89d3fe3ca5e1c8f04dbfb1e54585285c1f4a0158596e95a8b95eb` |
| `source-before.json` | `298f2f809a78edbeac2e02c8e6fd5e1fdd75c89fda721b3aa2a9d6d4a55fd5ab` |
| `source-final.json` | `1a04418f947d2199939c82e5dae9541ab3b14a5644a2ba4c57c8b1012269b29e` |
| `tests-final182.log` | `0d1d1ea063c34699676d465cbb1d17d22a287ebc7c7986fc2f101017610fc7dd` |
| `tests.log` | `25f1e87faa4445c49def583382eefeb7c35bf7608ccd66828ced35d5cf98843c` |
| `typecheck-final.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
| `typecheck.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
| `/workspace/.onboarding/command-ui-cancel-terminal-failing-control.log` (author pre-fix failure control) | `2fc2bcd0fec16a0afe1cc65149b28b93df2b643a713f206ea2b1c4c3a97b378f` |
