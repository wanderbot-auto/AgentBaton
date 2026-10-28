# Environment CI

This is infrastructure preflight, not application or real-Provider testing.

- GitHub Actions: workflow_dispatch, or a push on main that changes `infra/ci-request.json`. Normal documentation changes do not trigger another matrix.
- Hosts: ubuntu-24.04, macos-15 (arm64), macos-15-intel (x64). The runner OS is not the product minimum macOS promise.
- Scope: prepare Python/Node/Rust, validate design and setup constraints, run infrastructure fixtures and check Xcode/SDK availability.
- Limits: max 2 concurrent jobs, 3 jobs total, 25-minute timeout each, read-only repository token, no secrets and no publishing. Hosted execution may consume existing Actions allowance. No account budget or billing setting is changed.
- Evidence: commit SHA, runner OS/architecture, exact tool versions, command output, test result and job/run URLs. Missing execution is not success.
- Baseline hash checks use reference/design-v0.2-manifest.sha256.json; no binary source archive is required.
- Normal product CI, native GUI tests, signed packages and real Agent integrations remain gated on a later product-development approval.

Runner availability can change. Pin labels, verify actual architectures in logs, and reevaluate the Intel testing strategy before its hosted runner is retired.

## Authorized product CI resource bounds

AB-START-001 separately authorizes the Product workflow under existing hosted-CI quota; the Environment matrix above stays three jobs, maxparallel2 and25 minutes with no credentials/publishing. Product job budgets normally stay at most30 minutes. The sole cold-build exception is `product.yml` job `desktop`, fixed to the existing macos-15/macos-15-intel matrix, maxparallel2, fail-fast false and the original matrix runner selector, capped at45 minutes. Tests, default release audit and package steps all remain mandatory; product process/RPC/authorization limits are not extended. Unknown jobs/workflows, larger budgets or expanded runners/concurrency cannot borrow this exception. No account spending/billing/provisioning change is authorized here. Exact cause and independent evidence: docs/execution/goal-integration/CI_BUILD_BUDGET_ADR.md.
