# Checkpoint

Source: `29c27ef365038df079ced96668c209c23c0f4b62`.
Tree: `/workspace/.worktrees/factory-summary-proof`, branch `codex/factory-summary-proof`.
Source is frozen; subsequent own-record commit does not change it. Root integration tree and Core17 author tree were not edited.

Changed source paths:

- `crates/core/src/goal_factory.rs`: private request SQL snapshot/proof; get/list only; two cfg(test) strict comparison accessors returning readonly DTOs.
- `crates/core/src/factory_child_goal_state_tests.rs`: appended one test submodule registration; original bytes unchanged.
- `crates/core/src/factory_summary_proof_tests.rs`: four tests.

Suggested Root entry (NOT RUN here): `cargo test --locked -p agentbaton-core --lib summary_proof_tests -- --show-output --test-threads=1`, under Root's usual offline/resources/tool/source guard. Then existing Core/Factory/sessions IPC/Clippy checks as Root selects. Do not move old actual results onto this source.

New functions:

1. `request_snapshot_summary_and_complete_view_match_original_strict_bytes`
2. `later_request_rechecks_real_adopted_plan_and_preserves_initial_origin`
3. `owned_snapshot_and_strict_fallback_reject_corrupt_history_on_new_calls` (six deliberate faults)
4. `existing_transaction_never_reuses_proof_after_own_write_or_across_requests`

Pending: independent source review, Rust compilation/test execution, output/refusal equivalence on all existing cases, current-source complete runtime/CI evidence and measured readonly comparison. No claim that Intel4013ms or all read latency is fixed. Any discovered compile/test error must preserve this NOT_RUN source history before a separate fix leaf.
