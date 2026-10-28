# Development application acceptance

This development artifact is an incomplete implementation of Design v0.2. See the exact commit's STATUS and requirements-progress.json for implemented and unrun scope. It is not a production release. No Developer ID signing or notarization is configured; inspect build-record.txt for actual signature evidence. Do not disable macOS security protections to install it.

Record macOS version, architecture, exact commit and SHA-256 before testing. Native execution and manual acceptance remain unverified until their results are recorded; successful CI compilation alone does not establish desktop correctness or minimum supported OS.

- Install the application using the development DMG; verify it starts under the user's approved security policy. Record any Gatekeeper denial rather than bypassing it.
- Confirm empty state with no fabricated tasks or Provider accounts, and switch Chinese/English using keyboard controls.
- Create a task, restart the UI, and verify the stored task and journal remain available from the same daemon. A draft is not an executing Provider task.
- Cancel a never-dispatched draft and verify its state/evidence. Provider cancellation and unknown-outcome reconciliation require separate applicable tests.
- Close the UI; check the daemon remains available using `baton`. Reopen without a second writer. Record startup errors and actual process evidence.
- Attempt invalid API/version/authentication, stale revision and duplicate idempotency-key writes; verify they fail without altering unrelated state.
- Inspect the package for daemon/CLI sidecars, no test-control TCP listener, no copied personal accounts or signing secrets.

Not yet accepted: actual Provider execution, orchestration, workspace isolation/adoption, capability/configuration transactions, remote Hosts, retention and full desktop lifecycle. Missing authorization blocks live Provider validation only, not the remaining implementation.
