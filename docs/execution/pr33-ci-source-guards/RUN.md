# Replay the exact split and source guards

Use the integration task worktree and a private validation checkout. Set the frozen A source to `32950beb9c8a297a80d19474c00a2a501180c01e` and the tested guard commit to `7f9191345bf55cafd62c543934f55692d7992fe3`. The merged tree must be `a1c2b70b8040b7bdf83be79c2f5305013276cff2`. The recorded replay used an isolated clone with read-only Git object alternates, a detached private merge created from `git merge-tree --write-tree`, and the task's private runtime from `scripts/collab.sh run`.

Before testing, compare the workflow directory to the frozen A source with `git diff --exit-code "$a" HEAD -- .github/workflows`, check the full candidate digest in [EVIDENCE.json](EVIDENCE.json), and require a clean tracked checkout.

```sh
PYTHONDONTWRITEBYTECODE=1 python -m unittest -v \
  tests.infrastructure.test_macos_ci_partition_source_fixture \
  tests.infrastructure.test_schema17_migration_workflow \
  tests.infrastructure.test_integration_native_ci \
  tests.infrastructure.test_pr_workflow_secret_isolation \
  tests.infrastructure.test_background_service_native_smoke_source \
  tests.infrastructure.test_schema19_browser_ci_workflow \
  tests.infrastructure.test_private_browser_python
PYTHONDONTWRITEBYTECODE=1 python scripts/validate_infrastructure.py
PYTHONDONTWRITEBYTECODE=1 python -m unittest discover -s tests/infrastructure -p 'test_*.py'
```

Use the requirements-pinned infrastructure Python environment. Reconfirm the combined tree, workflow equality and tracked checkout state after the run.

These tests validate the actual workflow source, the actual entry shell, and the actual inline source receipt code. The entry gate receives controlled status values locally. The receipt test uses a real depth-one local merge checkout, fixtures for GitHub inputs and upload outputs, and controlled platform observations. This replay does not execute the hosted schedulers, Native UI, migrations or package compilation.
