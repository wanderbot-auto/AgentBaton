# AgentBaton — environment preparation only

The user authorized preparation of the private `wanderbot-auto/AgentBaton` repository and development/test environments. The user has not approved starting product development.

Read `AGENTS.md` and `infra/execution-gate.json` first. The gate is an explicit workflow/instruction check, not an OS sandbox or a substitute for GitHub branch protection. Do not change it without new user approval.

## Verification

```bash
python3 scripts/check_gate.py --scope preparation
python3 scripts/validate_design.py
python3 scripts/validate_infrastructure.py
python3 -m unittest discover -s tests/infrastructure -p 'test_*.py' -v
npm test
cargo test --locked --manifest-path tools/environment-smoke/Cargo.toml
```

The source design text and JSON are preserved from v0.2. The original ZIP remains in conversation/local backups; immutable baseline files are checked against the original SHA-256 manifest. Repository and environment metadata may be updated as actual evidence arrives.

## Existing repository

The remote repository has been created by the user. Do not run the legacy `bootstrap-github.sh --apply` creation command against it. That script remains for historical bootstrap dry-run/owner-guard tests only. Do not force-push, overwrite unknown content or recreate the repository.

## Execution scopes

`environment-preflight.yml` is restricted to environment smoke checks. It may run manually or when an explicit `infra/ci-request.json` request is committed on main. Product features, real Agent calls, production publishing and paid resource provisioning are not authorized.

Codex Cloud instructions are in `codex/ENVIRONMENT_SETUP.md`. Having a setup script is not a published Cloud environment. Record actual environment identity, tested commit and successful published state before marking it ready.

Do not claim macOS product builds, GUI tests or signed installers from these infrastructure fixtures. Stop after reporting verified readiness or concrete remaining blockers and await the user's product-development confirmation.
