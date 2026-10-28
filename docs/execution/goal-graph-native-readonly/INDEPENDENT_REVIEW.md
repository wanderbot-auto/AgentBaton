# Native Graph readonly independent source review

**PASS for frozen source/preparation, preservation and actual Python/static checks; no blocking static finding.** This is **not** Rust/Core12/native/macOS runtime acceptance. Root must compose Core12 and the UI-owned production bridge and allocate its existing exclusive Cargo window before those checks can run. Do not relax strict12 to make this baseline11 leaf pass.

Source **85cc8e7e8be9ba660b7755c1df320cbc776c7eca**, protected baseline **a1be94d26d10bb245bb35eb50226b82012c967da**, tree `/workspace/.worktrees/goal-graph-native-readonly`; later author docs-only **b88b77aa545cf38a30a4d384808fa398f3780fc2**. Exactly four source paths differ. Core, UI lib/approval, locks, workflow, authority and Root STATUS are unchanged. Final author source/index remains clean. Exact source SHA-256:

| Source | SHA-256 |
|---|---|
|native_smoke.rs|01519ecf98e71f941007e3fa3b00158f3bc8be0f4820b0bbc1e1ea24e7cb8992|
|apps/desktop/src-tauri/tests/test_native_runner.py|d21afd62483feb2423e67485352b10fb2ec458bd19df78d7cfff683f36ff3d02|
|scripts/native-macos-smoke.sh|aff60d635cc9a7ef11a7f4004f89ac3cd485b92ecaaa7bb9cf8fae7ca899ec10|
|scripts/native-run.py|f5ed6d82b93dceb4913e250b5ff8ac50577a9c8675979ad09ae76bbddaf2ed23|

## Contract and preservation findings

New Graph observations are additive and read-only. The private host uses fixed isolated Python `-I -S`, actual runtime database path, SQLite URI `mode=ro`, query_only, read transaction and one SELECT over six tables plus event sequence. It requires exactly8 integer fields: strict schema12, zero capabilities/jobs/candidates/validations/owner-decisions/acceptances and nonnegative sequence. There is no11|12 compatibility success path. Existing execution schema guard/fixture moves strictly11→12; prior wrong12 control becomes wrong13, new wrong11 is added and wrong10 remains.

The private callback captures original Goal, execution and Graph database snapshots and records them in that order, rejecting wrong phase/window/error/replayed checkpoints or unequal intervals. Final Rust validation first validates original execution evidence, then binds Graph before/after counts/sequence to both independent database groups and both original JS intervals. The browser adds actual fixed `goal_graph_runs_list` requiring[] and fixed missing get requiring NOT_FOUND/not_started with no result property, plus system snapshots and strict final flags. Graph flags and schema/table data cannot replace old Goal/execution evidence. The host is still the production restricted dispatch path; there is no new test-only Graph approval/propose/Owner/dispatch authority.

Mechanical independent preservation checks establish:

- All original **21 Python test function bodies** byte-identical; four added tests make25. The older method name containing schema11 remains historical while its actual guard is strictly12.
- Original **17 Rust tests and their module prefix** are identical after reversing only enumerated necessary schema changes; five added source tests make22. They contain actual-Core schema/table/journal drift, wrong-phase/window/replay/type/cross-interval, exact missing-denial and private Unix-daemon list/get controls, **not executed in this review**.
- Original Goal/execution SQL and SECOND JS are byte-identical. FIRST is identical after removing only additive Graph reads/report fields. Existing runner/shell lines are retained after necessary execution-schema guard normalization.
- All22 original top-level Rust function bodies were checked: unchanged except the two explicitly enumerated schema edits and additive Graph checkpoint/final-validator calls in native_smoke_report. Original bounded SQL reader2seconds/4096bytes, capture, phase commit, watchdog/main, dual-window/lifecycle and owned reap logic remain unchanged. No old assertion/deadline/test was deleted to seek green.

## Actual independent verification

- `python3 -B apps/desktop/src-tauri/tests/test_native_runner.py`: **25 passed, exit0**,2.548seconds. Log SHA **770e9adbeb443fc7fa9cbc1f4ca57b235fcf36ea8b4cf5e6a19e5951ea2740b3**. PYTHONDONTWRITEBYTECODE=1 and reviewer TMPDIR keep author source/cache untouched.
- `python3 -B .../independent-static-proof.py`: actual frozen Python validator extracted without runner execution; positive synthetic GOOD accepted, **123 negative controls rejected**, including the exact original81 plus missing fields, wrong10/11/13, nonzero six-table drift, fake booleans, coherent Graph-only interval substitution and fabricated missing-result. These are metadata unit controls, not compiled Rust controls.
- Exact extracted SQL was executed on a **generated minimal synthetic SQLite unit fixture**: old and new snapshots agree; database bytes unchanged by each readonly query; each of six real fixture table rows increments the proper observed count and is rejected by the actual frozen validator; wrong11/13 is observed then refused. This is not Core12 migration/schema-authority/actual daemon evidence.
- `node --check` on independently extracted FIRST and SECOND: both exit0. `bash -n` on frozen smoke script:exit0. Direct `rustfmt --edition2021 --check` on reviewer-owned frozen Rust:exit0 (parse/format only). Exact source diff `git diff --check baseline source`:exit0.

Commands, logs, source snapshots, extracted JS, proof script and metadata are retained here. Author manifest SHA782f430a6a6c40a0b2bdb9c77eff0398a0ac86e27e0e827cebc3e3ee5934b94c and all15 referenced artifacts were independently rehashed. Author PLAN/PROGRESS/CHECKPOINT/EVIDENCE correctly distinguish author checks, baseline Core11/bridge, synthetic unit evidence and all unrun runtime dimensions; no actual Rust/Mac/Owner/TaskAccepted result is fabricated.

## Remaining acceptance and boundaries

**NOT_RUN:** Rust typecheck/test22 harness and composed Native lib suite, all-target Clippy, actual composed Core12 migrations/six-table drift/private daemon+Unix IPC, production Graph bridge/WebView invokes, actual macOS dual architecture/native screenshots/lifecycle, default release audit, new packages, OS Owner dialog, real Provider/model and actual Graph approval/execution/validation/TaskAccepted. The current leaf deliberately keeps protected Core11; its actual12 requirements must fail until correct composition. Old schema11 artifacts or25Python results cannot substitute.

Root may compose this four-source leaf with final Core12 and the reviewed UI bridge, then use one explicitly handed-off existing target for real regression and Clippy. Later exact CI/native/package subjects require fresh evidence and original failure preservation before integration. This scoped source PASS grants no new authority and is not whole-product completion.

No Cargo command, cache/target/lock/source/Root STATUS write, GraphQL/GitHub approval, remote operation, credential/account/model call, paid resource or subagent was performed by this reviewer. No Cargo lease was acquired. Only the reviewer evidence directory and its generated unit fixtures were written.
