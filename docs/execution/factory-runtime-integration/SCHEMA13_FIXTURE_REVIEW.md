# Schema13 test-fixture correction — independent limited review

Verdict: no unresolved must-fix found within this strictly test-only version-alignment delta. This is a source/evidence review, not formal PR approval or a passing full runtime regression.

Exact base `dac6d80ebebcc9285c1a3eadafc81668c5b9567c`; source commits `ff2872e6b6134791fa51e1e95314adbaf6d74e67`, `7805973e8b9dab881d6035c98b441a78b0c813b7` and final `116506ace23a073f1f60b1968110258aa4a8a0df`. An intervening own-doc commit is not a production change. Author snapshot `artifacts/factory-runtime-core/schema13-fixture-source-snapshot.json` SHA-256 `c6dab030c94865afe87423ef0360a1cdd0e73c207336e861108b312689ceb9ab` matches these frozen test bytes.

## Cause and correct boundary

The actual full Rust run at Root `8532b2adee5e2de872621674ebbe606822e25f3a` stopped in admission_performance.rs on completed fresh Store schema: actual 13, stale expected 12. The existing production Store already accepts through schema13 and rejects versions greater than13. Therefore tests after successful current Store::open/restart must require exactly13; true unsupported-future fixture inputs must now use14. Historical source databases, intermediate migration version12, backups and rollback-to12 remain historical and must not be relabeled current. This patch respects that distinction and accepts no alternative `12|13` state.

## Per-file review

| Test file | Current Store expectation | Future refusal input | Label/name only |
|---|---:|---:|---|
| admission_performance.rs | 12→13 once | none | none |
| command_validation.rs | 12→13 once | none | one schema diagnostic |
| command_validation_migration.rs | 12→13 once | 13→14 once | matching test name |
| data_workspace_migration.rs | 12→13 once | 13→14 once | matching test name |
| data_workspaces.rs | 12→13 once after successful restart | none | none |
| goal_graph_migration.rs | 12→13 once after upgrade | 13→14 once | matching test name |
| goal_runtime_migration.rs | 12→13 once after upgrade | 13→14 once | matching test name |
| goal_store_migration.rs | 12→13 once after upgrade | 13→14 once | matching test name |
| migration_v5.rs | 12→13 once after upgrade | none | none |
| migration_v6.rs | 12→13 once after upgrade | none | none |
| migration_v7.rs | 12→13 once after upgrade | 13→14 once | matching test name |
| sessions_ipc.rs | 12→13 once after live Store/IPC | none | none |
| storage.rs | none | 13→14 once | none |

Independent byte normalization enumerated and reversed exactly 12 current expectation edits, 7 future inputs, 6 matching function names and 1 diagnostic string. Every normalized final file equals its base file byte-for-byte. Every current numeric edit is the expected value attached to the existing user_version query; each future input retains the original exact VERSION_UNSUPPORTED and unchanged raw-byte/backup rejection assertions. No test is deleted, skipped or replaced, and assertion macros remain unchanged.

Thus all other bytes remain intact, including historical PRAGMA12 inputs, schema12 collision/rollback/backup cases and labels, unknown99, sentinel/raw BLOB/documents, table/column/event/permission/CAS/enforcement assertions, thresholds and performance timing. Source exports and a per-line change inventory are in VERIFICATION.json and before/after directories. Independent checks also confirmed all46 production source bindings equal both base/final commits and the previously frozen CAS candidate; all12 tracked Cargo.toml/Cargo.lock files are byte-identical. No production migration/DDL, dependency, permission or resource limit was altered.

## Evidence and execution limits

The six declared original full-run evidence references were independently rehashed. That run remains failed, exit101: 150 outer cases passed, one outer case failed; one nested child pass is tracked separately rather than added. Remaining suites and doctests were not reached. Preliminary format passed; Clippy was not run after failure. This patch does not retroactively turn that run green, and no new exact-source Cargo tests are claimed by the author snapshot or this review.

Reviewer ran only Python/git static byte/hash assertions; no Cargo, compiler, actual product executable, daemon, browser, remote command or target mutation. Source, locks, schema, STATUS and original failed evidence remain untouched. No target lease acquired. Root must integrate these exact source revisions and rerun the actual full Rust checks before any broader acceptance. macOS/Native/fullFactory/unknown/fault/large-data performance coverage are separate from this schema fixture correction.
