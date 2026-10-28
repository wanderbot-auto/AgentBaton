# Progress

Read docs06/07/08/ADR0003, existing pure plan receipt/candidate edges and current per-attempt validator resolution. Confirmed old GoalValidatorRef additional-definition semantics cannot authorize fresh subjects. Shared wire v1 sentRoot and saved JSON, Native author coordinating same entry/mode. Starting actual Core evidence primitives; no runtime graph/acceptance test completion claimed yet.

Actual migration batch: `cargo test -p agentbaton-core --test goal_graph_migration -- --test-threads=1` 3 PASS0.05s; `cargo test -p agentbaton-core --test sessions_ipc actual_legacy_candidate_migration -- --test-threads=1 --nocapture` 1 PASS1.65s. Native cache exclusively held during commands, then released to Root, no active writer. Neither command proves graph scheduling/acceptance.

Owner-only runtime actual checks: graph capture 1PASS1.67s (`actual-graph-capture-2.log`); Owner→Summary 1PASS3.84s (`actual-owner-summary-2.log`); Owner→Summary→TaskAccepted full trusted projection/terminal session/cancel/sharedcount2 controls 1PASS6.21s (`actual-task-accepted-5.log`). Later command supersets earlier same pipeline, do not add them as unique tests. Deterministic Validate runtime still not implemented or verified; all failure/compile logs retained.
