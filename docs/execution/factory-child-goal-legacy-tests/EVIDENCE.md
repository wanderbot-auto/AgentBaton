# Evidence

## Actual observations

- Git-local worktree baseline: `5491c94cffbe40292ff977e0d9cfedcb55026c9e`.
- `python3` AST parse of `crates/core/tests/fixtures/child_goal_legacy_migration/generate.py`: PASS; no module execution/Core process.
- Frozen manifest read-only inspection: real f032 schema14 tools and prior zero-table probe recorded by their original producer. This task has not re-executed that probe and does not treat it as nonempty migration evidence.

## Required executions (NOT_RUN)

```sh
python3 crates/core/tests/fixtures/child_goal_legacy_migration/generate.py --frozen /workspace/.onboarding/factory-native-shared-limits-f0322/frozen --repository /workspace/.worktrees/factory-child-goal-legacy-tests --output /workspace/.onboarding/factory-child-goal-legacy-tests/run1
cargo test -p agentbaton-core --test factory_child_goal_legacy_migration
```

The first command requires the coordinator's real14 process window; the second requires its Cargo window and installed genuine fixture. No actual results/counts, schema15 success, DDL-only success, active/unknown recovery, derived-source corruption success, macOS, Provider/model, OS dialog or product completion are currently claimed.
