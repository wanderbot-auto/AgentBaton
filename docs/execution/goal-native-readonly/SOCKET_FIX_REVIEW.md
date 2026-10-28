# Independent native alias socket-layout review

Verdict: **no blocking finding in this fixture-only correction; suitable for fresh exact-head macOS CI**. Reviewed source `6a798204a9160c2bf86b9c0fd2b951dd912d102d` with documentation `1c2e78134ebfff24bc68646eef12844f0a89f9ac`, independently in `/root/review`. Prior P2 and alias reviews remain unchanged; this does not turn either failed CI subject into a pass.

## Actual failure and correction

PR13 subject `0a0beac38260cc426b4a0887b2bf59937ec9ff90`, Product run `40475194913`, Intel job `122091591770` actually passed workspace tests and 13 native host tests, then failed the new alias fixture's positive canonical socket bind (11/12 harness). Its raw log SHA-256 is `acf186178c4b94ad3ae41d6febc0282fccdb8f929d753d90971fdd0b94759d50`. The error was generic `INTERNAL_ERROR / Cannot bind private daemon socket`; no actual Darwin runtime pathname or OS errno was printed. The original generated-project canonical bind test passed on that Intel run. ARM's distinct generated-provider startup timeout occurred earlier and is not fixed by this change.

The old alias control unnecessarily nested the real runtime under another private runtime, `actual-parent`, and another temporary directory. The correction creates the actual runtime directly through the same `private_fixture_runtime()` helper and canonicalization as the real native main. A separate owned private namespace contains only the denied symlink alias. The alias resolves to the same exact runtime; the original refusal, database bytes, mode 0600, positive real-daemon bind, read-only snapshot and alias linkage assertions all remain. Failure diagnostics add only numeric socket-path byte count and existing structured Core error, without a path or wire dump.

The prior independent Linux 132-byte versus 41-byte actual-bind control demonstrates the relevant socket-length mechanism and remains under `goal-native-0a0b/controlled-socket-length.json`. It was not repeated without benefit and is not evidence that the historical Darwin errno/path was observed. Fresh Mac CI remains necessary.

## Independent source comparison

Only the existing alias test inside `#[cfg(test)]` changes. Mechanical byte comparison against source `8fde8b658bf49d18bd1f88290d16e3adc6278b5c` confirms:

- Everything outside `#[cfg(test)]` is byte-identical.
- The earlier generated-project canonical test, including its actual bind and private-database assertions, is byte-identical.
- All 12 tests and original strong assertions remain; no production Core/no-symlink guard, umask, TMPDIR, timeout, CI, runner, JS, dependency/lock or capability changes are introduced.

The actual runtime remains private and generated; the alias namespace is also private. Only the known generated runtime basename is appended to its symlink, and equality of canonical identities is explicitly asserted. This is no new filesystem/Provider permission or alternate product runtime-selection mechanism. TempDir ownership is retained through cleanup.

## Independent commands and actual results

Commands ran under umask 022 with existing Linux SDK/sysroot, `CARGO_INCREMENTAL=0`, jobs 2 and only the handed-off native target `/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target`. Main target was not touched. Per primary request, own Core/workspaces/native lib entrypoint mtimes were refreshed and their bytes verified unchanged before actual recompilation.

| Command | Result |
| --- | --- |
| `cargo test --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol` | 13 host + 12 harness passed, 0 failed/ignored; generated canonical bind and alias refusal/canonical bind both actually executed |
| `cargo clippy --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol --all-targets -- -D warnings` | passed |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` | passed |
| `git diff --check 8fde8b658bf49d18bd1f88290d16e3adc6278b5c..6a798204a9160c2bf86b9c0fd2b951dd912d102d` | passed |

Native target was formally released before starting the separate Provider review; Rust compilations were not run concurrently. No further native Cargo/target writes follow this verification. Only this report and ignored evidence were written by the reviewer.

Reviewed source SHA-256: `3c9a05ee705f9a049e289244dbfc8c7a04df712493a5f0515664efc574a5b913`.

Evidence directory `/workspace/.onboarding/goal-native-socket-review/`, manifest SHA-256 `ebc2df4c8cffa26f4c7400a57caf8f5281c5951f5b8dc4c6f2f5936d4eaa39e3`, records exact commands, source comparison, unchanged entrypoint hashes, and actual test/lint logs.

## Limits

Independent Linux source/regression review passes; the original Mac socket error remains preserved with its uncertainty. This is not new Mac host/harness/WebView execution, a GUI screenshot, Goal dispatch, owner confirmation, live Provider/model, minimum supported OS, user-install or signing proof. Original Goal3 plus independent DB-invariance, Command3/Data3 and lifecycle/default-release requirements remain unchanged and require fresh exact-head dual-Mac CI. An unusually long externally supplied temporary root could still fail legitimately; the fixture does not change process-wide temp configuration or production path protections to hide that condition.
