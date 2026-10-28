# Goal runtime evidence

Author source frozen at 9f833738f4bfc5d3a86edc51d71e047662eceb06. Runtime module SHA-256 `65c953ca8ae0a27fcb4377f9222059818d51f4d8287ad34e80630e983a9220b1`. All final checks below used the unchanged frozen source; earlier logs retain their original versions. Linux x86_64 generated local protocol fixtures use no Provider account, model call, billing identity or credential. Native Owner finalizer fixtures exercise the actual private Core boundary, not an OS dialog. Prior Data/schema10 binaries are not Goal/schema11 proof.

All logs below are preserved under `artifacts/goal-runtime-core/` in this worktree. Main Cargo target is `/workspace/.worktrees/validation-integration/target`, author lease released to the independent Core reviewer after final build. CARGO_HOME=/workspace/.toolchains/cargo, RUSTUP_HOME=/workspace/.toolchains/rustup, CARGO_INCREMENTAL=0, CARGO_BUILD_JOBS=2. Tests use `-- --test-threads=1`; actual protocol tests additionally `--nocapture` to preserve content-addressed full proposal exports.

| Actual command | Evidence | Result |
| --- | --- | --- |
| cargo test -p agentbaton-core --test goal_runtime_migration --test migration_v3 --test migration_v5 --test migration_v6 --test migration_v7 --test goal_store_migration --test command_validation_migration --test data_workspace_migration --test storage | migration11-2.log |29 passed; private logical backups/original rows/DDL rollback/future12 unchanged bytes |
| cargo test -p agentbaton-core --lib goal_store::runtime::tests | final-authority-units-1.log |11 passed19.94s;12th ordered advance test added afterward |
| cargo test -p agentbaton-core --test sessions_ipc actual_goal_ | all-actual-goal-1.log |15 passed30.76s; generated ACP, not real Provider |
| cargo test --workspace | full-workspace-4.log; full-workspace-4-counts.json |549 unique outer Rust tests +7 compile-fail doc tests PASS; includes Core52, private runtime14, sessions IPC34, Data21 and all migration fixtures |
| cargo clippy --workspace --all-targets -- -D warnings | clippy3.log |PASS; no lint suppressions |
| cargo fmt --all -- --check | final-format.log |PASS |
| cargo build -p batond -p baton | final-debug-build.log |PASS debug binaries from frozen source |
| Existing smoke-local-product.py with BINARIES set to immutable candidate, then actual fresh DB schema query | frozen-binaries-smoke.log |PASS real daemon/CLI Unicode, idempotency, evidence cancellation, SIGKILL/restart; actual pragma11 and0 Goal rows |

## Independent review controls

C1–C17 below are author-local coverage labels, not the independent reviewer’s differently numbered G findings. Exact test names and actual coverage take precedence. Test names are exact Rust function names; common module for private units is `goal_store::runtime::tests`. Actual IPC tests are in `crates/core/tests/sessions_ipc.rs`. No failed case was deleted or relaxed.

| Review finding | Exact evidence |
| --- | --- |
| C1 governed ordinary/low-level bypass | actual_goal_shared_budget_cannot_be_bypassed_by_normal_session_or_trusted_lowlevel_attempt |
| C2 original Native receipt/source/order/full snapshot | native_execution_origin_requires_full_snapshot_commitment_source_and_positive_order |
| C3 unknown cancel/restart/no empty-map proof | actual_goal_live_daemon_sigkill_does_not_replay_or_infer_stop_from_an_empty_supervisor; actual_process_exit_before_or_after_sent_marker_recovers_unknown_without_replay_or_false_cancel |
| C4 own dirty output vs changed original input | actual_goal_same_scope_replan_after_real_writes_keeps_history_and_budget_without_new_owner; actual_goal_changed_original_git_input_refuses_new_plan_or_advance_after_real_output |
| C5 immutable node/SQL/session/attempt | reserved_node_tampering_cannot_change_a_fixed_prompt_or_its_session_and_attempt |
| C6 lower stop source and sequence | actual_goal_completed_candidate_requires_unchanged_lower_owned_stop_receipts |
| C7 missing final meter remains unknown | actual_process_exit_before_or_after_sent_marker_recovers_unknown_without_replay_or_false_cancel; actual_goal_timeout_and_malformed_terminal_never_replay_or_become_task_acceptance |
| C8 shared optional policy not widened | persisted_project_optional_limits_cannot_be_raised_or_removed_by_a_later_root |
| C9 shared budget/atomic reservation | actual_goal_two_roots_share_project_consumption_and_third_root_cannot_reset_it; actual_goal_reservation_failure_rolls_back_attempt_session_lease_and_all_shared_counters |
| C10 complete fixed TaskRequest | actual_goal_native_approval_reserves_dispatches_complete_contract_and_reaps_candidate_without_accepting_task |
| C11 Data historical proposal/advance reads | actual_data_goal_proposal_recovery_is_historical_after_input_drift_but_new_authority_is_refused; actual_data_goal_executes_and_plan_advance_receipt_recovers_after_drift_without_reexecution |
| C12 first finite capability and later revision | zero_history_declared_token_and_cost_reservations_do_not_prove_metering_capability; actual_goal_replan_cannot_introduce_unmetered_finite_token_or_cost_cap |
| C13 actual overrun retained | actual_goal_output_reservation_overrun_blocks_renamed_replan_without_erasing_observed_bytes |
| C14 eligible implicit/parent edges and Summary | implicit_input_and_parent_edges_do_not_starve_the_real_eligible_producer; actual_goal_independent_summary_and_long_node_ids_use_the_same_authorized_executor |
| C15 aggregate raw/metadata caps, full300 history | three_hundred_persisted_nodes_near_four_mib_remain_complete_and_oversize_refuses_before_decode; reservation_collection_has_total_raw_limit_not_only_per_row_caps; oversized_sql_metadata_is_bounded_before_node_comparison |
| C16 alias cannot mint output0/mapping | unapproved_output_alias_never_becomes_zero_observed_output_or_execution_authority |
| C17 cancellation intent postcommit fault | recorded_cancel_fault_never_reports_not_started_and_same_key_recovery_does_not_repeat_stop |
| unknown persisted run/node state | unknown_persisted_run_state_is_never_treated_as_an_inactive_finished_run; reserved_node_tampering_cannot_change_a_fixed_prompt_or_its_session_and_attempt |
| trusted output root/FD measurement | selected_output_measurement_requires_original_root_and_stable_regular_nofollow_leaf |
| ordered autonomous version chain | autonomous_plan_advance_requires_an_unchanged_ordered_core_receipt_chain |

## Retained failures and limits

Earlier logs preserve actual defects (running-turn misclassified terminal, missing SQL history argument, Git-only root predicate incorrectly making Directory output unknown) and invalid new fixtures/expected byte counts, fixed without raising production timeouts or lowering assertions. migration11-1.log preserves a helper that counted SQLite WAL/SHM sidecars as backup images; corrected extension selection retains private-image and single-image assertions. full-workspace-1.log preserves the outdated schema10 test assertion; full-workspace-2.log is an explicitly interrupted partial run after a fixture replacement matcher failed, not a passing full suite. full-workspace-3.log preserves actual Data completed/candidate with unknown output due to the Git-only root check; fixed through trusted Data confirmed-copy/root identity plus pinned FD reads. output-root-guard-1.log preserves unavailable rustix mkfifo test compile failure, fixed with portable POSIX /usr/bin/mkfifo. clippy1.log retains two style failures fixed without allow attributes. Core strict unknown fields, missing authority, finite unsupported metering, aliases, drift, unknown effects and overrun all fail closed.

actual_output_bytes is the sum of final whole sizes of this attempt's selected mapped artifact files, repeated files counted again on later attempts. It is not protocol output, cumulative filesystem writes or a workspace OS quota. Observation reads are capped at1MiB; unsupported/missing/unsafe measurements remain null. Actual runtime is the owned worker interval including startup/stop; unreaped time is a separate lower bound, never final actual. Reservations and old attempt consumption persist across versions/restarts/unknown. No token/cost measurement is invented.

Full Goal remains incomplete: current executor supports independent Work/Summary in an already approved workspace. Bound dependent validation/Owner acceptance, actual child-task/delegation/factory execution and larger off-actor output verification remain next dependency work. No TaskAccepted, arbitrary renderer execution, new workspace, identity, host, recipient, OS sandbox, production release, signature or real Provider call is inferred. Browser/Native/macOS and independent final review remain separate integration evidence. Full author regression and the immutable schema11 candidate are complete. Main Cargo target is now exclusively assigned to the independent reviewer; the author will not write it without a new explicit handoff.


## Immutable candidate

`/workspace/.onboarding/goal-runtime-schema11-9f833738/manifest.json`, SHA-256 `2114390f03df55e6d48c5f1b20864f2c22f9351c88e1eb66c5c19e80703f04dc`. Source_files94/canonical map SHA-256 `76ba31c43c03709e106a5d015d78ebce063d430b721af25ae593e9164a40fa13`, git_head/source_head9f833738f4bfc5d3a86edc51d71e047662eceb06, dirtyfalse at freeze. Manifest includes exact per-file and final-check hashes/compiler/platform; binaries have no compiled Goal crash-test environment controls.

- batond: same directory, SHA-256 `081b84b5813e46f1f111b2a5c80bd6888563e9673147df1a9b831576149aa4ca`,96023584 bytes.
- baton: same directory, SHA-256 `1d80513e5095b1ab7fc7703a858b92b2f98be7b1756536eb1c87dab1383b1656`,71044024 bytes.

These are unsigned Linux debug validation executables, not macOS installers. Old schema10 candidates remain untouched. Parent owns subsequent integration/PR/CI/macOS packaging, independent Core reviewer owns the main target, and the separate real IPC leaf received this exact manifest. No merge, account, model call, fee, production or signing authority was added.

Data preview fixture upstream0db994786b3e70e21b7c0c57a15cf210fd9e9fdf was cherry-picked as5c48eeb, after parent handed back file ownership. The only subsequent change in that file is the latest schema assertion10→11 (4eb4458); large prepare35s, ordinary/cancel5s, production30s, response3s/client5s,32MiB and original safety assertions remain as upstream. Linux PASS is not macOS proof.
