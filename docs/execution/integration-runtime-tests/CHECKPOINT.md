# Checkpoint

Current source freeze: `8c41e50af6c05dbc8b6f770004f12c26062ed047` (first source `19c402e9ad05ce4b4271b750b64c5effc942dd6b`) on `codex/integration-runtime-tests`, baseline `97f14100f746c24aebf2bf7eba54264d1029c387`.

Only own documentation may change while Root compiles the frozen Rust source. No author Cargo/Actor/Git-fixture/cache lease is held. Do not run old schema15/16 tools for these tests; the Actor in the new target must actually report schema17. Root supplies supported-host PATH for the complete test process; `/usr/local/bin/git` with multiple links remains unsupported, not silently bypassed.

Root command proposal (NOT_RUN here): `cargo test --locked -p agentbaton-core --test integration_ipc integration_runtime:: -- --show-output --test-threads=1`, then the whole existing `integration_ipc` target. Use Root's current explicit `CARGO_TARGET_DIR`, `CARGO_INCREMENTAL=0`, jobs/serial lease and supported PATH. No additional cache is authorized.

New tests:

- `integration_runtime::accepted_graph_explicit_start_fresh_correctness_and_exact_cas_preserve_owner_source`
- `integration_runtime::native_policy_before_task_acceptance_automatically_integrates_once_without_explicit_wake`

Both are source-only until Root records an actual result. A successful private finalizer is a trusted-host IPC dimension, not genuine OS approval. Fresh Command correctness is only the declared fixed-command contract; Owner acceptance remains separate. No scientific/live-model/real Provider or signed/unsigned delivery evidence is produced here.

Outstanding: compile/type/link checks; actual normal-path results; all runtime failures, cancellation/revocation/expiry/recovery/race/mixed-budget matrices; other supported changes/platforms and complete AB-VAL-009/full product scope. Parent/Core may provide exact later production fix commits for safe integration; do not hand-edit production here.
