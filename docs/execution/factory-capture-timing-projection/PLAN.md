# Factory capture timing finite projection

Scope: AB-GOAL / Factory budget evidence, under full Design v0.2 and AB-START-001. This work changes only the existing post-owned-shutdown public diagnostic collector, its dedicated pure tests and these four records. Baseline `34f8b23d4fdd529caf4f7da6f949604f2746e0e9`; isolated branch `codex/factory-capture-timing-projection`.

Project six explicitly approved nullable u64 millisecond diagnostics from `goal.capture_settled`: producer_elapsed_ms, runtime_reservation_ms, capture_worker_started_queue_ms, capture_worker_completed_queue_ms, capture_join_observed_queue_ms, capture_worker_elapsed_ms. Values are diagnostic only, not actual runtime/admission/acceptance/stop or budget decisions. No new schema/version, event association rule, ordering inference or deadline change.

Keep all existing controls and unrelated functions unchanged. Add real generated own SQLite/event fixtures covering finite valid/unknown/invalid/privacy cases; run the complete dedicated pure suite. Freeze a source commit for independent review. Root owns integration and any genuine runtime/CI validation; this leaf does not alter Core, workflows, DDL, locks, permissions or global STATUS.
