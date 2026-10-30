# Checkpoint

Source `4e9e20d25c7b104c57cffc51a92de3a2eff82a71`.
Base `61c0fce36c1819d136b91c6349e8d86031156f8d`.
Tree `/workspace/.worktrees/factory-history-worker`, branch `codex/factory-history-worker`.

Product paths:

- ipc.rs: fixed Owner read selector, separate service field, ready-route enqueue, finished poll and owned cleanup.
- factory_reads.rs: only DatabasePin plus capture/open_readonly/recheck visibility to sibling module; complete original bodies preserved.
- factory.rs: private module registration/export.
- factory_history_reads.rs: single owned read worker and cfg-only finite synchronization/inspection controls.

Tests:

- factory_child_goal_state_tests.rs: append4-line private module only; factory_history_reads_tests.rs defines10 real Core/SQLite/owned-thread controls.
- goal_stop_service_tests.rs: append private path/module only; factory_history_actor_tests.rs defines2 actual Unix/generated ACP/manual Actor controls. No old body/assertion changed and no duplicate old test module included.

Suggested Root commands, not run here:

1. cargo test --locked -p agentbaton-core --lib history_worker_tests -- --show-output --test-threads=1
2. cargo test --locked -p agentbaton-core --lib history_actor_tests -- --show-output --test-threads=1

Use Root's current offline/resource/source/tool guards. The10 service controls cover complete DTO/list bytes, true readonly write refusal, real concurrent WAL cancellation, empty-list revision, retryable saturation, new-call digest corruption, original pre-expired request, actual SQLITE_INTERRUPT acknowledgement with concurrent Core write/Actor transaction unaffected, deadline interruption, panic/shutdown resource retention and real revocation preserving original effect refusal. Two Actor controls reuse exact approved4s/20s/30s generated ACP fixture and actual reaped-process/lease/Task/receipt/attempt/event assertions. No claim of Daemon.run, complete FactoryGraph or Mac from those two.

Root must then select current-source full Core, Factory/sessions daemon regression, strict Clippy/fmt and exact future CI/Mac. No assertions, deadlines or budgets may be loosened to hide a failure. A schema17 composition needs an explicit exact-schema delta and corresponding evidence; this source is schema15 Parent61 only.
