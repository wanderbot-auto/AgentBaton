# Source progress

Base `71fd73958dd6b340bf030c0e747cd392fa82cc72`; implementation `9942d2de833d183a486a00d0de6960e09209f23b` on `codex/goal-admission-observation-handoff`. Only the six allowed Core source/test files changed.

- ipc.rs: private reply adapters, stack-owned optional observation, unchanged external error wrapper, original two completion paths and automatic Goal consumption.
- goal_factory.rs: bounded opaque observation capture and exact same-run/revision/plan consumption through the existing decoder.
- goal_runtime.rs: original block method delegates to an observed variant with the same transaction/state/event write; one cfg(test)-only bridge invokes the real original ancestor guard.
- goal_store.rs: crate-private reexport only.
- factory_history_reads_tests.rs: ordinary wire sender adapter only.
- factory_child_goal_reservation_tests.rs: existing test prefix retained; three new Core/SQLite test sources.

Actual configured rustfmt and git diff --check passed. Twenty-six lightweight source/byte-preservation checks passed. Cargo compilation, the new Rust cases, actual Actor/Provider/process/browser/Mac and exact CI are NOT_RUN. No product tree, budgets, permissions, clocks, original assertion or Provider identity changed.

The positive propagation test uses real native-approved queued Goal/Store records, real recorded advance intent, a deliberately oversized negative argument to the real original reserve_ancestors guard, the production intent wrapper, private owned reply and real blocked event writer. It is not an actual automatic Actor/Provider execution. Negative sample cases deliberately corrupt closed revision scalars or run binding; they do not claim a naturally advanced concurrent run. No test has yet been executed.
