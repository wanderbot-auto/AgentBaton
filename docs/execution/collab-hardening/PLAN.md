# Minimal local collaboration hardening

Base: f71556c154cb1f20679f4882c861895a4e0fb538, origin/main.
Task: collab/collab-hardening; one writer, isolated worktree registered by collab.sh.
Scope: collaboration scripts, CLI regression tests, collaboration/agent instructions, task evidence, integration STATUS and reviewed bootstrap digests. The integration owner added BOOTSTRAP_MANIFEST.json to the local reservation under the registry lock before editing its tracked digests.

1. Retain existing worktree/path ownership protocol.
2. Add task-local runtime/build command wrapper and foreground leases; serialize native dev without changing product ports or trust checks.
3. Exercise actual CLI with temporary local Git repositories; existing Environment unittest discovery automatically runs the tests on Linux and ARM macOS.
4. Independent review, infrastructure validation, scope and merge preview; feature PR with actual CI. Only append existing Product checks to required rules after verifying check names on that PR; preserve existing protections.

Acceptance: independent task runtime/target paths; reject overlapping run/native commands and finish while running; forward signals and keep argv/exit status; retained existing scope/conflict behavior. No new product integration, real Provider calls, credentials or production deployment.

OWNER-001/002 read: this narrow user-requested infrastructure repair preserves their product/UI instructions; it does not claim to finish them. Local executor, no new Cloud run or persistent Goal created.

CI-discovered scope adjustment: the integration owner reserved tests/infrastructure/test_factory_bound_provider_exit.py under the registry lock. An unchanged synthetic proc test depended on a real 20ms observation window; independent delayed-sleep reproduction matched its failure. Inject only its namespace clock and add an expired-observation refusal control. Product/browser source, deadlines and old assertions stay intact.
