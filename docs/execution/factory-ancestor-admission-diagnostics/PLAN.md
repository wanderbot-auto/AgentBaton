# Original ancestor-admission refusal diagnostics

Requirement context: bounded autonomous rework, shared child/root/project budgets, honest failure evidence (Design v0.2 D18/D19; WP05/WP06/WP10). This diagnostic does not complete any requirement or authorize more work.

PR #18 head `fbeb2e96854868735b9c07f27485c5cf8f748502`, Product `41094992088`, Intel job `124198322532` retained a `RESOURCE_LIMIT` block at Graph revision 14 before the second Validator started. The public post-shutdown child total (15589 ms) plus the predeclared 6000 ms Validator reservation exceeds the unchanged 20000 ms child cap. The prior capture was 3919/4000 ms and joined. These are different observations from earlier capture overruns and unknown outcomes. The old projection cannot identify the exact original admission branch or its instant.

Carry a small typed sample from the existing ancestor guard through the existing CoreError details reference and existing blocked event. Publish only strict numeric/count/closed-enum fields. Preserve all original authority, stop, clock, accounting, overflow, unknown-runtime, error-code/message/side-effect/retryability and refusal behavior. Add no IPC, retry, process, new ledger or migration.

Validate pure field/SQLite projection, then independently review and run the new Rust units and applicable Core/real rework/CI tests in the integrating executor. No fixture cap or observation window is changed.
