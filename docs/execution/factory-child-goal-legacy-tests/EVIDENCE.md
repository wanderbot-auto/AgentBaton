# Evidence

## Actual observations

- Git-local worktree baseline: `5491c94cffbe40292ff977e0d9cfedcb55026c9e`.
- `python3` AST parse of `crates/core/tests/fixtures/child_goal_legacy_migration/generate.py`: PASS; no module execution/Core process.
- Standalone source formatting: `RUSTUP_HOME=/workspace/.toolchains/rustup /workspace/.toolchains/cargo/bin/rustfmt --edition 2021 crates/core/tests/factory_child_goal_legacy_migration.rs` PASS. This did not run Cargo, compile Core, use a shared target or exercise migration.
- Frozen manifest read-only inspection: real f032 schema14 tools and prior zero-table probe recorded by their original producer. This task has not re-executed that probe and does not treat it as nonempty migration evidence.

## Exact execution commands and remaining NOT_RUN

```sh
python3 crates/core/tests/fixtures/child_goal_legacy_migration/generate.py --frozen /workspace/.onboarding/factory-native-shared-limits-f0322/frozen --repository /workspace/.worktrees/factory-child-goal-legacy-tests --output /workspace/.onboarding/factory-child-goal-legacy-tests/run1
AGENTBATON_ACTUAL_SCHEMA14_FIXTURE=/workspace/.onboarding/factory-child-goal-legacy-tests/run2 cargo test -p agentbaton-core --features actual-legacy-migration-tests --test factory_child_goal_legacy_migration
```

The first command ran once under the coordinator's exclusive lease at source722ba21 and **FAILED** at Factory scope proposal: INPUT_CHANGED/not_started, after successful actual Native Goal/Graph history creation. Actual schema14 counts: goal_scopes1, goal_runs1, goal_graph_capabilities1, factory_scopes0, factory_runs0, factory_child_plan_revisions0, events43. Native consumed approval kinds: connection1, grant1, goal1, goal_graph_execution1. Goal run cancelled; capture job completed; FK clean. SIGTERM wait exit0; no forced kill. Generated PID1385969/start_ticks13451218 was gone after join. Original f032 tool modes0o500, sizes32546424/2689584 and SHA256 unchanged before/after.

Retained private runtime `/tmp/ab14-jji2eld7` (mode0700), report/rawRPC `/workspace/.onboarding/factory-child-goal-legacy-tests/run1/manifest.json`, `FAILURE_SUMMARY.json`, `daemon.log`.

The second generator execution was separately authorized at source `869c9214c75c6471cb911ac7355d33c47a7ff630`, with the identical command except `--output /workspace/.onboarding/factory-child-goal-legacy-tests/run2`, and returned exit0. Actual14 counts: Goal scope1/run1/Graph cap1; Factory scope1/run1/child-plan revisions1/head1; events69. Real consumed approvals: Connection1/Grant2/Goal1/Graph1/Factory1. Graph confirmation25 then authorization26; native capability raw document SHA256 `219f1261518e4100649a6ce8a1a8b17188fadf2ae27a9c8ef47afcb0b2c1e127`. Original IDs/proposal snapshots/results/raw plan/event documents are retained in the actual private SQLite backup and raw RPC manifest. Output0700, backup0600, 860160 bytes, SHA256 `36a2567b724b3275808015b7a0d66635e441e20f6a637b121ee41212ec3e58c7`; FK clean. Generated PIDs1387016/start13481747 and1387076/start13481880 were both gone after clean SIGTERM wait0, no forcedkill. Original frozen tool SHA and mode/size equal before/after. Successful runtime cleanup occurred after actual owned join and original14 backup. Run1 remains retained unchanged.

Private run2 `manifest.json`, `SUCCESS_SUMMARY.json`, `daemon.log`, `actual-schema14.sqlite3` are not public-uploaded. No schema15 execution or Core process was started after the lease release. The cargo command remains NOT_RUN. No schema15 success, DDL-only success, active/unknown recovery, derived-source corruption success, macOS, Provider/model, OS dialog or product completion is claimed.

Coordinator expanded the allowed paths solely to install that empty test feature/dedicated target in Core Cargo.toml; it was committed with no dependency/lock/production changes at23314cb. CI integration and rebuild provenance remain PENDING, and the six checks are not ordinary659+7 results.

## Actual dedicated schema15 execution (FAILED)

At `23314cbacbb16304e8873d68a6385a33f8ff69e4` (unchanged Core5491), coordinator allocated one offline/locked six-test window. Actual cargo/rustc1.99.1; test debug0, incremental0, jobs2, test threads1; DEV_DEBUG/RUSTFLAGS/wrappers unset. Shared target `/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target`. Command:

```sh
env -u CARGO_PROFILE_DEV_DEBUG -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS -u RUSTC_WRAPPER -u RUSTC_WORKSPACE_WRAPPER RUSTUP_HOME=/workspace/.toolchains/rustup CARGO_HOME=/workspace/.toolchains/cargo CARGO_TARGET_DIR=/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 CARGO_PROFILE_TEST_DEBUG=0 AGENTBATON_ACTUAL_SCHEMA14_FIXTURE=/workspace/.onboarding/factory-child-goal-legacy-tests/run2 /workspace/.toolchains/cargo/bin/cargo test --offline --locked -p agentbaton-core --features actual-legacy-migration-tests --test factory_child_goal_legacy_migration -- --nocapture --test-threads=1
```

Result: compile18.87s, tests1.69s, exit101; **4passed,2failed,0ignored,0filtered**. Actual ELF `target/debug/deps/factory_child_goal_legacy_migration-d42921b4951d716a`,14669968 bytes, ELF magic7f454c46, SHA256 `440e421224c30a84877d174c43ed8d46f1e8c7daaf63346df76d77d3614b617c`. SOURCE_BEFORE/AFTER prove exact source and immutable run2 manifest/DB unchanged; 12 original manifest/root paths unchanged, only the explicitly authorized Core Cargo test gate differs. Existing baseline warns unused factory_child_goal_failure_context; no warning was suppressed.

PASS: corrupt original result/capability/ordered receipt rejected; future16 refused before backup/recovery; migration collision rolled back; orphan original scope rejected.

FAIL1: wrong Native capability FK source accepted. `goal_graph_capabilities.run_id=a8c0b414-aa35-43a3-a9c6-c50098768b7d`; its relational approval_id was changed to actual consumed Goal scope `f491acbc-ba7b-46f8-b38e-8fa0d625b223` (kindgoal), while document.approval_id remained the actual Graph ID. FK clean, Store::open unexpectedly accepted schema15. `/tmp/legacy14-nB5XV7/history.sqlite3` and pre-v15 backup retain the exact mismatch. Checked_capability at goal_graph.rs243 reads only document. Root and sole Core writer notified; assertion retained, not accepted as success.

FAIL2: positive migration reached final reopen backup-count assertion after passing raw original rows, original14 backup and FK checks, then counted3vs1. Our helper included open backup SQLite `-wal/-shm`; narrow `.sqlite3` extension filtering is corrected, not rerun. Original failed `/tmp/legacy14-sQyTlh` is retained. This test is still FAIL.

Private complete log/provenance `/workspace/.onboarding/factory-child-goal-legacy-tests/migration-run1/`. Window released; no automatic retry. Core fix/new independent run, CI integration/rebuild manifest, active/unknown recovery and actual derived-source corruption remain open. No overall migration/product pass.
