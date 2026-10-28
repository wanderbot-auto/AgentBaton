# Goal runtime evidence

Author source checkpoint947cd99661fd4c5226064d9906d629c48c475918 plus the ordered plan-advance receipt negative added afterward; final freeze is pending. Linux x86_64 generated local protocol fixtures use no Provider account, model call, billing identity or credential. Native Owner finalizer fixtures exercise the actual private Core boundary, not an OS dialog. Prior Data/schema10 binaries are not Goal/schema11 proof.

All logs below are preserved under `artifacts/goal-runtime-core/` in this worktree. Main Cargo target is `/workspace/.worktrees/validation-integration/target`, exclusive author lease. CARGO_HOME=/workspace/.toolchains/cargo, RUSTUP_HOME=/workspace/.toolchains/rustup, CARGO_INCREMENTAL=0, CARGO_BUILD_JOBS=2. Tests use `-- --test-threads=1`; actual protocol tests additionally `--nocapture` to preserve content-addressed full proposal exports.

| Actual command | Evidence | Result |
| --- | --- | --- |
| cargo test -p agentbaton-core --test goal_runtime_migration --test migration_v3 --test migration_v5 --test migration_v6 --test migration_v7 --test goal_store_migration --test command_validation_migration --test data_workspace_migration --test storage | migration11-2.log |29 passed; private logical backups/original rows/DDL rollback/future12 unchanged bytes |
| cargo test -p agentbaton-core --lib goal_store::runtime::tests | final-authority-units-1.log |11 passed19.94s;12th ordered advance test added afterward |
| cargo test -p agentbaton-core --test sessions_ipc actual_goal_ | all-actual-goal-1.log |15 passed30.76s; generated ACP, not real Provider |
| cargo test --workspace | full-workspace-1.log |running at this checkpoint; exact final result still pending |

## Independent review controls

Test names are exact Rust function names; common module for private units is `goal_store::runtime::tests`. Actual IPC tests are in `crates/core/tests/sessions_ipc.rs`. No failed case was deleted or relaxed.

| Review finding | Exact evidence |
| --- | --- |
| G1 governed ordinary/low-level bypass | actual_goal_shared_budget_cannot_be_bypassed_by_normal_session_or_trusted_lowlevel_attempt |
| G2 original Native receipt/source/order/full snapshot | native_execution_origin_requires_full_snapshot_commitment_source_and_positive_order |
| G3 unknown cancel/restart/no empty-map proof | actual_goal_live_daemon_sigkill_does_not_replay_or_infer_stop_from_an_empty_supervisor; actual_process_exit_before_or_after_sent_marker_recovers_unknown_without_replay_or_false_cancel |
| G4 own dirty output vs changed original input | actual_goal_same_scope_replan_after_real_writes_keeps_history_and_budget_without_new_owner; actual_goal_changed_original_git_input_refuses_new_plan_or_advance_after_real_output |
| G5 immutable node/SQL/session/attempt | reserved_node_tampering_cannot_change_a_fixed_prompt_or_its_session_and_attempt |
| G6 lower stop source and sequence | actual_goal_completed_candidate_requires_unchanged_lower_owned_stop_receipts |
| G7 missing final meter remains unknown | actual_process_exit_before_or_after_sent_marker_recovers_unknown_without_replay_or_false_cancel; actual_goal_timeout_and_malformed_terminal_never_replay_or_become_task_acceptance |
| G8 shared optional policy not widened | persisted_project_optional_limits_cannot_be_raised_or_removed_by_a_later_root |
| G9 shared budget/atomic reservation | actual_goal_two_roots_share_project_consumption_and_third_root_cannot_reset_it; actual_goal_reservation_failure_rolls_back_attempt_session_lease_and_all_shared_counters |
| G10 complete fixed TaskRequest | actual_goal_native_approval_reserves_dispatches_complete_contract_and_reaps_candidate_without_accepting_task |
| G11 Data historical proposal/advance reads | actual_data_goal_proposal_recovery_is_historical_after_input_drift_but_new_authority_is_refused; actual_data_goal_executes_and_plan_advance_receipt_recovers_after_drift_without_reexecution |
| G12 first finite capability and later revision | zero_history_declared_token_and_cost_reservations_do_not_prove_metering_capability; actual_goal_replan_cannot_introduce_unmetered_finite_token_or_cost_cap |
| G13 actual overrun retained | actual_goal_output_reservation_overrun_blocks_renamed_replan_without_erasing_observed_bytes |
| G14 eligible implicit/parent edges and Summary | implicit_input_and_parent_edges_do_not_starve_the_real_eligible_producer; actual_goal_independent_summary_and_long_node_ids_use_the_same_authorized_executor |
| G15 aggregate raw/metadata caps, full300 history | three_hundred_persisted_nodes_near_four_mib_remain_complete_and_oversize_refuses_before_decode; reservation_collection_has_total_raw_limit_not_only_per_row_caps; oversized_sql_metadata_is_bounded_before_node_comparison |
| G16 alias cannot mint output0/mapping | unapproved_output_alias_never_becomes_zero_observed_output_or_execution_authority |
| G17 cancellation intent postcommit fault | recorded_cancel_fault_never_reports_not_started_and_same_key_recovery_does_not_repeat_stop |
| ordered autonomous version chain | autonomous_plan_advance_requires_an_unchanged_ordered_core_receipt_chain |

## Retained failures and limits

Earlier logs preserve actual defects (running-turn misclassified terminal, missing SQL history argument) and invalid new fixtures/expected byte counts, fixed without raising production timeouts or lowering assertions. migration11-1.log preserves a helper that counted SQLite WAL/SHM sidecars as backup images; corrected extension selection retains private-image and single-image assertions. Core strict unknown fields, missing authority, finite unsupported metering, aliases, drift, unknown effects and overrun all fail closed.

actual_output_bytes is the sum of final whole sizes of this attempt's selected mapped artifact files, repeated files counted again on later attempts. It is not protocol output, cumulative filesystem writes or a workspace OS quota. Observation reads are capped at1MiB; unsupported/missing/unsafe measurements remain null. Actual runtime is the owned worker interval including startup/stop; unreaped time is a separate lower bound, never final actual. Reservations and old attempt consumption persist across versions/restarts/unknown. No token/cost measurement is invented.

Full Goal remains incomplete: current executor supports independent Work/Summary in an already approved workspace. Bound dependent validation/Owner acceptance, actual child-task/delegation/factory execution and larger off-actor output verification remain next dependency work. No TaskAccepted, arbitrary renderer execution, new workspace, identity, host, recipient, OS sandbox, production release, signature or real Provider call is inferred. Browser/Native/macOS, full regression, independent final review and immutable schema11 candidate are pending at this checkpoint.
