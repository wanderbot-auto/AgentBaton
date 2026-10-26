# AgentBaton — authorized development handoff

The owner has approved full Design v0.2 development in `wanderbot-auto/AgentBaton` after readiness review. The previous preparation-only boundary is superseded by `infra/execution-gate.json` and `docs/execution/launch/APPROVAL.md`, without expanding production, real Provider test credentials/calls, or paid-resource permissions.

Start with AGENTS.md and codex/MASTER_PROMPT.md. The active launch goal and runbook are in codex/GOAL.md and codex/RUNBOOK.md. Product implementation progress belongs in docs/execution/, not immutable design-baseline status fields.

## Existing evidence

PR #1 prepared public-repository PR CI; run 40352221818 passed on Linux, macOS ARM64 and Intel before merge. Ruleset 25893455 was verified active in the preceding readiness check. Cloud publication/setup results were supplied by the owner. Recheck relevant live state before integration; these records do not prove product tests or current-commit tests have passed.

## Checks

```bash
python3 scripts/check_gate.py --scope product
python3 scripts/verify_bootstrap_manifest.py
python3 scripts/validate_design.py
python3 scripts/validate_infrastructure.py
python3 -m unittest discover -s tests/infrastructure -p 'test_*.py' -v
npm test
cargo test --locked --manifest-path tools/environment-smoke/Cargo.toml
cargo clippy --locked --manifest-path tools/environment-smoke/Cargo.toml --all-targets -- -D warnings
```

These are infrastructure/baseline checks. Add actual product checks and commands with implementation; never relabel fixtures as product coverage. Preserve the three required Environment check names and add product CI through reviewed PRs. The validator may evolve with new workflows only while retaining security guarantees and regression tests.

Do not recreate the repository, use legacy bootstrap --apply, force-push, bypass rules or overwrite unknown changes. Preparation hashes may be updated for deliberately reviewed file changes; the original Design v0.2 manifest and approved requirements remain intact. Rehashing files is integrity bookkeeping, not correctness or authorization proof.

Setup/startup hooks only prepare environments; they must not autonomously begin product work on every environment launch. Actual development starts from the user's task. No Cloud task has been directly launched by this handoff.
