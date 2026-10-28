# Evidence

Baseline reads and concrete wire/ADR only so far. No schema12 migration, graph candidate, validator job, Owner acceptance, dependent execution or final TaskAccepted has been run yet. Prior Core279/Root554 are predecessor evidence, not new graph completion.

Actual migration batch: `cargo test -p agentbaton-core --test goal_graph_migration -- --test-threads=1` 3 PASS0.05s; `cargo test -p agentbaton-core --test sessions_ipc actual_legacy_candidate_migration -- --test-threads=1 --nocapture` 1 PASS1.65s. Native cache exclusively held during commands, then released to Root, no active writer. Neither command proves graph scheduling/acceptance.

Owner-only runtime actual checks: graph capture 1PASS1.67s (`actual-graph-capture-2.log`); Owner→Summary 1PASS3.84s (`actual-owner-summary-2.log`); Owner→Summary→TaskAccepted full trusted projection/terminal session/cancel/sharedcount2 controls 1PASS6.21s (`actual-task-accepted-5.log`). Later command supersets earlier same pipeline, do not add them as unique tests. Deterministic Validate runtime still not implemented or verified; all failure/compile logs retained.
