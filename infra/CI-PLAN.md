# Environment CI

This is infrastructure preflight, not application or real-Provider testing.

- GitHub Actions: workflow_dispatch, or a push on main that changes `infra/ci-request.json`. Normal documentation changes do not trigger another matrix.
- Hosts: ubuntu-24.04, macos-15 (Apple Silicon, arm64). The runner OS is not the product minimum macOS promise.
- Scope: prepare Python/Node/Rust, validate design and setup constraints, run infrastructure fixtures and check Xcode/SDK availability.
- Limits: max 2 concurrent jobs, 2 jobs total, 25-minute timeout each, read-only repository token, no secrets and no publishing. Hosted execution may consume existing Actions allowance. No account budget or billing setting is changed.
- Evidence: commit SHA, runner OS/architecture, exact tool versions, command output, test result and job/run URLs. Missing execution is not success.
- Baseline hash checks use reference/design-v0.2-manifest.sha256.json; no binary source archive is required.
- Normal product CI, native GUI tests, signed packages and real Agent integrations remain gated on a later product-development approval.

Runner availability can change. Pin labels, verify actual architectures in logs, and treat Intel compatibility as deferred.

## Authorized product CI resource bounds

AB-START-001 separately authorizes the Product workflow under existing hosted-CI quota; the Environment matrix above has two jobs, at most two concurrent, and a 25-minute timeout with no credentials/publishing. Product job budgets normally stay at most30 minutes. The sole cold-build exception is `product.yml` job `desktop`, fixed to the macos-15 Apple Silicon matrix, max-parallel 2, fail-fast false and the original matrix runner selector, capped at 60 minutes (the existing Apple Silicon budget). Tests, default release audit and package steps all remain mandatory; product process/RPC/authorization limits are not extended. Unknown jobs/workflows, larger budgets or expanded runners/concurrency cannot borrow this exception. No account spending/billing/provisioning change is authorized here. Exact cause and independent evidence: docs/execution/goal-integration/CI_BUILD_BUDGET_ADR.md.
