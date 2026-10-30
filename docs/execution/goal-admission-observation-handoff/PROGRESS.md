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


## Independent static P2 correction `d9149a0111dd81776f3853e0e988bb5b0ba7a521`

Reviewer identified that the original thin wire-only `handle` wrapper now has only test references; a non-test dead_code warning was expected. This was a static finding, not an actual Cargo/strict check failure. A single `#[cfg(test)]` now gates only that unchanged thin wrapper. The actual handle_observed implementation, all test bodies, wire data, error/recovery transformation, budgets and guards are unchanged; no allow was added. Configured rustfmt --check and diff checks passed. Root compilation/runtime and independent closure remain pending.


## Root actual compilation failure and minimal adapter correction `04de1f8136c86288b7c479127ca826441c4a728d`

Root source429a6f7 failed E0599 before running tests: the original delayed-frame test uses blocking request.reply.send(Value), while the new reply enum initially retained only try_send. Original log SHA256 `d9cff462d2d1a33fe7416f5aff9845000ed55e034bc1c3f4fde9a3bf6afaba9c` remains unchanged. This is an actual compile failure, separate from the earlier static unused-wrapper finding.

A cfg(test)-only send adapter now directly calls the original Wire SyncSender::send, preserving its blocking/disconnected behavior and SendError<Value> payload. The Goal variant also blocks on its original channel and sends the unchanged wire with no diagnostic; an error maps back to the original wire Value. The old test body is unchanged and was not converted to try_send. Author rustfmt/diff checks passed, but no Cargo/runtime retry or successful compiler result is claimed; Root and independent reviewer must verify next.
