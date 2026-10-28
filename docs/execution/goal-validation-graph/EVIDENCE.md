# Evidence

Baseline reads and concrete wire/ADR only so far. No schema12 migration, graph candidate, validator job, Owner acceptance, dependent execution or final TaskAccepted has been run yet. Prior Core279/Root554 are predecessor evidence, not new graph completion.

Actual migration batch: `cargo test -p agentbaton-core --test goal_graph_migration -- --test-threads=1` 3 PASS0.05s; `cargo test -p agentbaton-core --test sessions_ipc actual_legacy_candidate_migration -- --test-threads=1 --nocapture` 1 PASS1.65s. Native cache exclusively held during commands, then released to Root, no active writer. Neither command proves graph scheduling/acceptance.
