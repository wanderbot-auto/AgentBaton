# Handoff

Worktree: `/tmp/integration-apple-system-git`; branch: `codex/integration-apple-system-git`; base: `426cd1e530f8894cb38fff6c0386ae0451dea478`.

Only `crates/core/src/integration_fs.rs`, `crates/workspaces/src/lib.rs`, two new adjacent test files and this unit's docs change. The only test-module registrations are in those two already allowed files. `integration_io.rs`, `adoption.rs`, `source_capture.rs`, lockfiles, protocols, migrations and existing integration tests are untouched.

Root pending: independently inspect policy and source alignment; compile/execute Core `system_git_policy_tests` and workspace `system_git_tests`; run relevant workspace capture/adoption/inspect/isolation and integration positive/refusal/runtime targets; validate exact candidate on actual Apple Silicon and Intel CI. Existing failed ARM log remains evidence. A Linux pass cannot prove Darwin API correctness or Mac runtime support. No merge/Approve/production authorization is issued here.

Local source-check artifacts: `/tmp/integration-apple-system-git-source-check.py` and `/tmp/integration-apple-system-git-source-check.json`; their hashes and dimensions are in EVIDENCE. They compare saved exact base blobs with current source and do not execute Rust policy or tool subprocesses.
