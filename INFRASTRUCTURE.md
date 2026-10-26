# AgentBaton — environment preparation only

Repository: `wanderbot-auto/AgentBaton`. The owner changed visibility to public on 2026-10-26 and authorized continuing environment preparation. Product implementation remains NOT authorized.

Read `AGENTS.md` and `infra/execution-gate.json` first. The gate is a workflow/instruction check, not an OS sandbox or GitHub branch protection. Do not change execution authorization without separate user approval.

## Verification

```bash
python3 scripts/check_gate.py --scope preparation
python3 scripts/verify_bootstrap_manifest.py
python3 scripts/validate_design.py
python3 scripts/validate_infrastructure.py
python3 -m unittest discover -s tests/infrastructure -p 'test_*.py' -v
npm test
cargo test --locked --manifest-path tools/environment-smoke/Cargo.toml
cargo clippy --locked --manifest-path tools/environment-smoke/Cargo.toml --all-targets -- -D warnings
```

The design v0.2 baseline is preserved. Original design hashes are separate from preparation-file integrity hashes. Update preparation hashes only for reviewed changes; do not weaken tests or alter product requirements to make checks pass.

## Repository and CI

Do not recreate the repository or use legacy `bootstrap-github.sh --apply`. That script is historical bootstrap test coverage, not a deployment command. Do not force-push or overwrite other work.

The preflight runs on PRs targeting main, manual dispatch, and an explicit `infra/ci-request.json` change on main. It has read-only contents permission, no secrets, pinned actions and bounded GitHub-hosted jobs. Public PR code is untrusted; do not use privileged PR events or personal runners. The three check names match `infra/github-main-ruleset.json`.

`infra/BRANCH_PROTECTION.md` explains activation. A committed ruleset JSON is NOT an active ruleset. The current integration has repository content write operations but no ruleset write operation.

## Cloud and authorization

The user reported all Cloud preparation checks passing, with no repository changes or injected credentials, and supplied a screenshot showing the AgentBaton environment published and private to the user. This is user-provided evidence; the cloud EVIDENCE.json and published environment identifier have not been directly retrieved here.

No product work packages, real Provider calls, production publication or paid resource creation are authorized. No app or installer has been built. Stop after the environment readiness/blocker report and wait for a separate product-development confirmation.
