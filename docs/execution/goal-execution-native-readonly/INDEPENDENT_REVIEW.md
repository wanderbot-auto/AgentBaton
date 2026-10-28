# Independent Native Goal execution readonly review

**PASS: no blocking finding in the frozen readonly leaf.** This is independent code/test review evidence, not a formal GitHub Approve. Native macOS WebView execution and new flags remain unverified until new exact-source macOS CI.

Source776e02e1a61a82b676972b9d40c9317524984c56, baseline78b317a4dc42f088f752852bf04028eb0bb51d9c, author worktree /workspace/.worktrees/goal-execution-native-readonly. Current author HEAD33cf2e6d962f7ae4ebf04b82b2615e3b5f1e65e7 adds docs; all four source bytes match776e02e and the tree is clean. Partial requirements only: AB-SEC-002, AB-TSK-003/007/008, AB-REL-001/008. Existing AB-START-001 and OWNER001/002 boundaries remain unchanged.

## Exact source hashes

| File | SHA-256 |
| --- | --- |
| apps/desktop/src-tauri/src/bin/native_smoke.rs | fc12725e5d8ea45b2ebc891cc1d314b5ee709a2778ef555da96e79209bd5182a |
| apps/desktop/src-tauri/tests/test_native_runner.py | 9fe6bb9c6d70898f7de0c328a7cdce7db88c1cafc2bbc982f72c6a292acb6aeb |
| scripts/native-run.py | 3627339eb7f93e76157361c4573669c9b286b1c776c6e28292b85906d8b49f86 |
| scripts/native-macos-smoke.sh | ab4ff790b67f3d8c59700a3ba619d464ed0ea61da076d5cb1db54c3763b651c5 |

Only these four sources differ between baseline and source commit. Core src is identical to9f833738f4bfc5d3a86edc51d71e047662eceb06; **this leaf does not prove G19 or full Goal execution**.

## Independent actual commands/results

Same unchanged author worktree, parent-leased Native target only, existing Linux SDK/cache. Main target was not accessed. CARGO_INCREMENTAL=0, CARGO_BUILD_JOBS=2, RUST_TEST_THREADS=1. Full SDK/toolchain/env, UTC interval, commands and hashes are in independent-results.json.

- cargo test --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol: PASS,28 library+17 harness tests (45 unique),0 failures/ignored/filtered;3.938s command. Main/doc targets contain0 tests. Harness17 consists of original12 plus5 additive tests.
- cargo clippy --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol --all-targets -- -D warnings: PASS,0.307s.
- cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check: PASS,0.121s.
- PYTHONDONTWRITEBYTECODE=1 python3 apps/desktop/src-tauri/tests/test_native_runner.py: PASS21 tests (original17+4);2.666s command,2.531s unittest body.
- Actual extracted FIRST and SECOND JavaScript: node --check passed for both. bash -n scripts/native-macos-smoke.sh passed. Python compile checks passed. These are syntax evidence, not WebView invocation.
- Independent AST-extracted exact Python report validator:55 additional negative controls reject bool/float/string/null/negative numeric counts in both snapshots, numeric fake flags, coordinated execution-only interval replay and fabricated missing-run result. These55 are isolated validator controls, not additional product/GUI test counts.
- git diff --check passed. All four source hashes and three entrypoint hashes/mtimes are identical before/after. Reviewer did not touch source or entrypoint mtime.
- Rehashed14 author evidence artifacts and their four source bindings, all match. Independent Cargo reran the same cached author tree; test paths are from the new log. Three observed ELF hashes/sizes/mtimes are recorded, and all mtimes precede independent test start. No large binary copy or cache deletion.

No failing command occurred in this independent review; negative controls pass by requiring rejection. Logs and control script are retained.

## Mechanical preservation

preservation-and-controls.json independently proves:

- Entire original Rust fixture_tests module prefix, including all12 original tests, is byte-identical.
- All17 original Python test function bodies are byte-identical.
- Original four-field GOAL_READONLY_SQL and goal_database_snapshot/bounded_goal_query/valid_goal_counts/record_goal_checkpoint functions are byte-identical; hashes also equal author's preservation manifest.
- Removing only new Goal execution readonly lines/fields normalizes FIRST exactly back to the original. All original report fields remain. SECOND is byte-identical.
- Both native-run.py/native-macos-smoke.sh retain every original line in original order.
- No new CI, product/Core, permission, lock, migration, default app-feature or release-audit changes.

## Contract/security review

- **Fixed native calls, no new approval:** native_smoke.rs800–815 add only system_snapshot, goal_runs_list with actual task.id and goal_runs_get with fixed native-readonly-missing-goal-run. Empty-scope execution UI must remain unmounted. List must return[]; fixed get must return real NOT_FOUND/not_started and no own result field. No new approve/finalize/propose/dispatch/advance/cancel mutation is introduced in this readonly interval. Existing task setup and later draft cancellation remain original fixture behavior.
- **Independent database proof:** lines34–68 run a fixed private-host Python query with -I/-S, private runtime path, mode=ro/query_only/BEGIN and original bounded2-second/4096-byte child query helper. It independently reads actual SQLite schema11, all three goal_runs/goal_run_nodes/goal_run_reservations zero counts and journal sequence. Renderer-selected SQL/executables/paths are absent. JS flags alone cannot replace these snapshots.
- **Phase and replay binding:** lines70–108 permit only phase0/main, valid complete integer counts and sequence equality with the original Goal snapshot for the same checkpoint. Before/after are single-use and identical; failed/error reports cannot be revived. Integration at610–650 collects both independent snapshots, records original and execution checkpoints, persists them, and publishes failure on any mismatch.
- **Strict report evidence:** Rust109–141 requires true JSON booleans, exact complete5-field unsigned count/schema snapshots, equality, exact2-field sequence object bound to original JS+DB intervals, and exact3-field missing-run ID/code/not_started error. Python native-run.py54–78 independently rejects bool-as-int, floats, incomplete/extra fields, wrong schema/nonzero counts, wrong intervals and fake error shape. Native shell assertions add required evidence without removing old assertions.
- **Actual Linux negatives:** execution_readonly_schema_and_each_actual_table_drift_fail_closed1565–1589 changes real SQLite PRAGMA user_version to10/12 and separately inserts deliberately malformed generated rows into each execution table. Actual private query observes the changed value and rejects it. These are rejection fixtures, not SQL-forged approved Goal executions or migration acceptance.
- **Actual Unix/Core test:** execution_actual_private_daemon_empty_list_and_fixed_missing_get_are_readonly starts a real private Core daemon thread, creates its fixture task before the readonly interval, invokes authenticated Unix list/get, requires[] and exact absence error with no result, and checks both independent old/new snapshots unchanged. Its owned daemon is stopped/joined by Drop. This is genuine Linux Core Unix IPC evidence, not a browser/native WebView simulation.
- New checkpoint/type/evidence tests reject wrong phases/windows, replay, missing/fake values, schema/table drift and original-vs-execution interval mismatches. Existing lifecycle, bounded query, private runtime, stale timeout and failure-preservation tests remain intact and pass.

## Limits, resources and handoff

Rust1.99.1, Python3.12.14, Node24.19.0, Linuxx86_64. Native target /workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target was checked for active writers, exclusively leased for this review, and **RELEASED after Cargo checks**. No further Cargo writer by reviewer. Main target never touched; no active target/evidence/cache deletion. Disk remained approximately2.8GiB free. All review writes are under /workspace/.onboarding/goal-execution-native-readonly-review; runtime tests used their normal private temporary fixtures.

Frozen9f833 Linux sidecars and existing frontend resources are compile resources only. Actual new macOS Tauri WebView invokes, new JS flags in WebView, native Owner dialog, dual-architecture CI/packages, release/signing, real Provider/model/accounts and full Goal/G19 behavior are not_run/not_proved. Node syntax does not stand in for these. Root must run and inspect the exact integrated new macOS CI, report fields, independent DB snapshots and lifecycle artifacts before native acceptance.

No product edits, remote writes, CI launches, formal approvals, models, credentials, accounts, purchases or recursive subagents were used.

