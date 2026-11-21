# Replay the local partition controls

Run these commands from the task worktree. The matching Git object `83c22bcc479c271d3389d812f954b63873066422` must be present. It provides the real PR merge tree and two parent SHAs plus the exact existing sidecar staging helper. Use the configured Rust and Python installations for Cargo metadata, and install the repository's pinned infrastructure dependencies for the historical inverses.

```sh
scripts/collab.sh run -- env PYTHONDONTWRITEBYTECODE=1 \
  python3 docs/execution/pr33-ci-partition-contract/local-contract.py

scripts/collab.sh run -- env PYTHONDONTWRITEBYTECODE=1 PYTHONPATH="$PWD" \
  python docs/execution/pr33-ci-partition-contract/workflow-inverse-control.py

scripts/collab.sh run -- env PYTHONDONTWRITEBYTECODE=1 python -m unittest \
  tests.infrastructure.test_schema17_migration_workflow \
  tests.infrastructure.test_schema19_browser_ci_workflow \
  tests.infrastructure.test_integration_native_ci \
  tests.infrastructure.test_background_service_native_smoke_source \
  tests.infrastructure.test_legacy_migration_ci_evidence \
  tests.infrastructure.test_record_development_bundle_provenance
```

The receipt positives use synthetic statuses and artifacts bound to the actual Git merge coordinates. Use cases include failed, skipped, cancelled, timed-out or absent dependencies, stale attempts, head-vs-merge confusion, wrong source and runner, missing or extra stages, artifact substitution and disallowed archive members. The staging control calls the actual shell helper in a temporary directory with synthetic input bytes. The compiler shim forwards only the exact Cargo metadata request. It refuses a compile request, and the real staging helper refuses incomplete input. No staged bytes are executed.

The inverse control works only against the pinned workflow reference. It reconstructs that file exactly from role-bound slices and invokes the current inverses. It does not produce a runnable split workflow. [Guard and topology contract](CONTRACT.md)
