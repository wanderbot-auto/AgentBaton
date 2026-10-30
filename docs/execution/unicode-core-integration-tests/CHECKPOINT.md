# Recovery checkpoint

Tree: `/workspace/.worktrees/unicode-core-integration-tests`; branch: `codex/unicode-core-integration-tests`; baseline `796bb90907fa1510281525b1076d4e5592ad93c6`; source `1ce5640994ca067204c48cb0168276111e782473`.

Modified source: `crates/core/tests/fixtures/integration_runtime.rs` (277 added/16 removed). Source SHA-256 `14b340de9b0c32f016e54321e24ad300cb38cd12b9375d12204046f45bb0049b`. No uncommitted source was discarded or recreated, and no other worktree was mutated.

Last actual commands: configured `rustfmt --edition 2021 --check` on this fixture and `git diff --check`, both exit0. The separate lightweight STATIC.json has 36 successful source checks (not product tests). No Cargo, daemon, generated ACP/Git product fixture, Node, browser, Native or macOS execution occurred in this author task.

Next: independent source review, then Root cherry-pick and actual `cargo test --locked -p agentbaton-core --test integration_ipc -- --nocapture` with the existing bounded runner and supported single-link system Git PATH. Both new cases must pass; retain exact failure evidence if anything rejects earlier. Never accept deadline/permission/unknown errors as successful path validation or enlarge original limits. Continue full Design v0.2 after this partial slice.
