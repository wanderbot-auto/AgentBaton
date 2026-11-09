# Frozen checkpoint

Base: `0cee7d57907af547880db1367c0386c4bc108950`. Source: `385487b36d9625d6556c6242f0310b6dca2dc209` on `codex/integration-core-lints`. Worktree: `/workspace/.worktrees/authorized-integration`.

Artifact manifest: `/workspace/.onboarding/integration-core-lints-source-385487b/MANIFEST.json`; SHA256 `5e1bba41090f5dde8be6be95d70d81127c62f6881ea9f172487b88f0149b660a`. The manifest binds thirteen source hashes, inverse/format evidence and the original Clippy failure log SHA256 `25579121f4374b2fade2995341bf4179fcb400102bef3cf4ceac3b577eed651a`. The source remains separate from this documentation commit.

Root should run workspace all-target compilation and strict Clippy, relevant Core/IPC and existing JSON/Native bridge roundtrips. Transparent Box serialization equivalence is the intended Rust/Serde contract, not an author-executed roundtrip result. Review must verify no pending context is lost by boxed spawn errors and moved cfg modules retain exact bodies. No permissions, budgets, migrations, fixture assertions, dependencies, locks, SourceCapture files or other author files changed. Full authorized integration and macOS acceptance remain outside this lint checkpoint.
