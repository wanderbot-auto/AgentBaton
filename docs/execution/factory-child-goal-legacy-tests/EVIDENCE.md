# Evidence

## Actual observations

- Git-local worktree baseline: `5491c94cffbe40292ff977e0d9cfedcb55026c9e`.
- `python3` AST parse of `crates/core/tests/fixtures/child_goal_legacy_migration/generate.py`: PASS; no module execution/Core process.
- Standalone source formatting: `RUSTUP_HOME=/workspace/.toolchains/rustup /workspace/.toolchains/cargo/bin/rustfmt --edition 2021 crates/core/tests/factory_child_goal_legacy_migration.rs` PASS. This did not run Cargo, compile Core, use a shared target or exercise migration.
- Frozen manifest read-only inspection: real f032 schema14 tools and prior zero-table probe recorded by their original producer. This task has not re-executed that probe and does not treat it as nonempty migration evidence.

## Required executions (NOT_RUN)

```sh
python3 crates/core/tests/fixtures/child_goal_legacy_migration/generate.py --frozen /workspace/.onboarding/factory-native-shared-limits-f0322/frozen --repository /workspace/.worktrees/factory-child-goal-legacy-tests --output /workspace/.onboarding/factory-child-goal-legacy-tests/run1
cargo test -p agentbaton-core --test factory_child_goal_legacy_migration
```

The first command ran once under the coordinator's exclusive lease at source722ba21 and **FAILED** at Factory scope proposal: INPUT_CHANGED/not_started, after successful actual Native Goal/Graph history creation. Actual schema14 counts: goal_scopes1, goal_runs1, goal_graph_capabilities1, factory_scopes0, factory_runs0, factory_child_plan_revisions0, events43. Native consumed approval kinds: connection1, grant1, goal1, goal_graph_execution1. Goal run cancelled; capture job completed; FK clean. SIGTERM wait exit0; no forced kill. Generated PID1385969/start_ticks13451218 was gone after join. Original f032 tool modes0o500, sizes32546424/2689584 and SHA256 unchanged before/after.

Retained private runtime `/tmp/ab14-jji2eld7` (mode0700), report/rawRPC `/workspace/.onboarding/factory-child-goal-legacy-tests/run1/manifest.json`, `FAILURE_SUMMARY.json`, `daemon.log`. No second generator execution occurred. The source correction uses independent initial approved Grants/workspaces; its run2 is NOT_RUN and needs a new lease. The cargo command remains NOT_RUN. No schema15 success, DDL-only success, active/unknown recovery, derived-source corruption success, macOS, Provider/model, OS dialog or product completion is claimed.
