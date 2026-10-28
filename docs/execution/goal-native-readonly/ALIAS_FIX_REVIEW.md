# Independent native fixture alias correction review

Verdict: **no blocking finding; suitable for integration and fresh exact-head macOS CI**. Source `8fde8b658bf49d18bd1f88290d16e3adc6278b5c`, baseline `ad30ad0b77dba3a2992ba881d28ce0a6d7e3dc13`, independently reviewed by `/root/review`. This report does not replace or edit the closed P2 source-leaf report (`6851f0510c0c580045dec63d8301d8e2ddf972a0c918d465d9a74e06187e4bd3`).

## Failure and exact correction

Actual AEFF PR Product run `40471245384` failed on both macOS jobs: Apple Silicon `122078603054`, Intel `122078603267`. Each passed its Core command suite and 13 native host tests, then failed the new generated-project real-daemon-bind harness test (10/11). The database was already mode 0600; the runtime path failed Core's intentional no-symlink guard. Both exact raw logs and log-only ZIPs remain independently recorded in `goal-integration/NATIVE_AEFF_REVIEW.md`, final SHA-256 `0e8f1f51a7d8e4563ecbd76b939ef3fe4c5349715ee43f599e10a5e698d2b19f`. The first report and manifest, with Intel pending, are retained separately. Neither real WebView acceptance nor default release/package audit ran successfully after those failures.

The actual native executable already canonicalizes its generated runtime. The correction makes the generated-project unit use that same canonical runtime for setup, Store, read-only snapshots, file metadata and actual `Daemon::bind`. The original mode-0600, project identity, empty task/workspace/grant, counts, actual bind and cleanup assertions remain. It does not bypass an error or remove the real bind operation.

The added test creates an owned private parent and explicit symlink alias, prepares a fresh mode-0600 database through the canonical path, then actually calls `Daemon::bind` on the alias. It requires `PERMISSION_DENIED` with the symlink guard message and unchanged database bytes. The same database's canonical runtime must then bind successfully; Goal snapshot validation, private mode and alias linkage remain checked. This exercises both safety and legitimate use instead of accepting both paths or changing production authorization.

Independent static comparison confirms only `apps/desktop/src-tauri/src/bin/native_smoke.rs` changed in this commit. All bytes before `#[cfg(test)]` are identical. The original 11 harness test names and assertions are retained, with one additional test. Production Core/no-symlink handling, filesystem permissions, environment/TMPDIR, JS flags, DB read-only queries, Tauri commands/capabilities, dependencies/locks, runner, CI, deadlines and timeouts are unchanged. A prior independent Linux mechanism control also rejected the alias and accepted the canonical runtime, but is not presented as observed Darwin path bytes or a new Mac execution.

## Independent verification

The explicitly handed-off native target was used exclusively; the main Core target was not accessed. Per the primary's request, this worktree's Core, workspaces and native lib entrypoint mtimes were refreshed to force source-correct cached dependency rebuilding; SHA-256 before and after was identical and is recorded. All commands used umask 022, `CARGO_INCREMENTAL=0`, jobs 2, the existing Linux SDK sysroot, and native target `/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target`.

| Actual command | Result |
| --- | --- |
| `cargo test --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol` | 13 host + 12 harness passed, 0 failed/ignored; includes corrected actual bind and new alias denial/canonical bind control |
| `cargo clippy --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol --all-targets -- -D warnings` | passed |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` | passed |
| `git diff --check ad30ad0b77dba3a2992ba881d28ce0a6d7e3dc13..8fde8b658bf49d18bd1f88290d16e3adc6278b5c` | passed |

The original generated-project test and new alias test both executed in the independent full 12-harness run. Untouched browser/runner suites were not mechanically repeated; their prior source-leaf evidence remains accurately separate. No additional cargo, target writes or source changes follow this verification; the native target was formally released to the primary before documentation was completed.

Reviewed source SHA-256: `775f355327660a3d21c51ddc317c9ac958c870b58dd7c1ee8c633efba8c12309`.

Evidence directory: `/workspace/.onboarding/goal-native-alias-review/`. Verification manifest SHA-256: `3f153c2b19cbca1a05bbfdd9aaf941af1b6e25b5db3476e57b4a87f15bee23b3`. It records exact commands, test/lint logs, source-diff proof and unchanged entrypoint hashes. The reviewer changed only this assigned report and ignored evidence, not source/locks/index/commits/remote state.

## Limits

This closes independent source/Linux regression review of the fixture correction. It does not turn AEFF's actual two Mac failures into successes. Fresh exact-head dual-macOS execution must still prove 13 host tests, all 12 harness tests, real Goal3 plus unchanged DB counts/sequence, Command3/Data3, task/project identity, native window lifecycle/owned daemon cleanup, screenshot capture and default release audit. Manual OS owner confirmation, populated Goal plans, runtime dispatch, real Provider/model/accounts, minimum supported macOS, signing and full product acceptance are not established by this change.
