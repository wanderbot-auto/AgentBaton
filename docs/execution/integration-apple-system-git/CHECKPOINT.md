# Handoff

Worktree: `/tmp/integration-apple-system-git`; branch: `codex/integration-apple-system-git`; base: `426cd1e530f8894cb38fff6c0386ae0451dea478`.

Only `crates/core/src/integration_fs.rs`, `crates/workspaces/src/lib.rs`, two new adjacent test files and this unit's docs change. The only test-module registrations are in those two already allowed files. `integration_io.rs`, `adoption.rs`, `source_capture.rs`, lockfiles, protocols, migrations and existing integration tests are untouched.

Root pending: independently inspect policy and source alignment; compile/execute Core `system_git_policy_tests` and workspace `system_git_tests`; run relevant workspace capture/adoption/inspect/isolation and integration positive/refusal/runtime targets; validate exact candidate on actual Apple Silicon and Intel CI. Existing failed ARM log remains evidence. A Linux pass cannot prove Darwin API correctness or Mac runtime support. No merge/Approve/production authorization is issued here.

Local source-check artifacts: `/tmp/integration-apple-system-git-source-check.py` and `/tmp/integration-apple-system-git-source-check.json`; their hashes and dimensions are in EVIDENCE. They compare saved exact base blobs with current source and do not execute Rust policy or tool subprocesses.

Follow-up base f379e0a adds only one Mac actual-filesystem test and this unit's docs. Production sources and all prior 11 tests are unchanged. The test opens fixed system paths read-only with NOFOLLOW, logs closed scalar facts, then requires the unchanged production trust proof and held pin to succeed. It has one original-style five-second control and no sleeps/retry/timeout increase. Actual filesystem observation, Rust compilation and execution remain NOT_RUN by this author; Root must review and execute it on both real Mac CI lanes. Preserve the earlier ARM failure and do not infer native success from source checks or a Linux test count.

## Root composed verification — 2026-10-31T06:21:02.745312+00:00

See [PLATFORM_CLOSURE_RESULTS](../authorized-integration/PLATFORM_CLOSURE_RESULTS.json) SHA256 d76e816d670f9d213d88830275d24f5d640f1c55e008c5d52b98da94f20b055b for exact sources, actual commands, preserved failures, independent reviews and pending platform dimensions. This supplements historical source-only records; no full requirement/native/Provider acceptance is implied.
