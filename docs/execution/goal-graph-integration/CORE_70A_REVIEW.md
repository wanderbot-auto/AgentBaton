# Final independent Core/workspace review — 70a48d4

**No blocking finding remains in the reviewed existing-workspace Goal validation/Owner/dependency slice. Independent Rust verification passed.** This is not final Design v0.2, Native/UI, macOS, Provider-account, factory, parent/child execution or natural-language planning acceptance.

## Exact source and exclusive execution

Source commit `70a48d41663108337529915358109026e98ddf98`, tree `/workspace/.worktrees/goal-validation-graph`. Before/after all 100 Rust/Cargo manifest/lock hashes match the author immutable manifest SHA `c9fe6d7a517b15baf1386fc6111bc7413983c22e9f299c35ece242afdea9ba06`. Source-map SHA `7ef299c501455b4ae9ec5ee8eaa0d19af426a620f856a4b11da866c29ff1dce7`; all lock hashes unchanged. Hash method is SHA256 of compact sorted JSON path→SHA256 map. Exact files and actual Git heads/timestamps are in source-before/after.json. Only author docs were allowed to change during execution; they are not passed off as frozen Rust.

Author formally released the sole Native cache, Root confirmed, then reviewer ran the commands below using that same tree/cache. No parallel Cargo writer, new target, product edits or lowered tests. Environment: CARGO_HOME=/workspace/.toolchains/cargo, RUSTUP_HOME=/workspace/.toolchains/rustup, CARGO_TARGET_DIR=/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target, CARGO_INCREMENTAL=0, CARGO_BUILD_JOBS=2, RUST_TEST_THREADS=1. rustc1.99.1. Full exact commands/results/environment are in workspace-result.json and checks-result.json.

| Actual command | Result |
| --- | --- |
| cargo test --locked --workspace --no-fail-fast -- --test-threads=1 --nocapture | exit0; 593 outer tests +7 compile-fail doctests; zero failures/ignored; 512.86s |
| cargo fmt --all -- --check | exit0;0.78s |
| cargo clippy --locked --workspace --all-targets -- -D warnings | exit0;0.13s cached check |
| cargo build --locked -p batond -p baton | exit0;0.10s cached build |

Outer unique counts: Config67, Core315 (69 library + integrations including sessions55), Providers82, Workspaces127, daemon2, CLI0. Five passing nested subprocess re-executions are excluded from 593, with each enclosing process and outer summary recorded in test-counts.json. Seven compile-fail doctests are listed separately: expected compile diagnostics are not regression failures. Full workspace has49 Cargo Running/Doc-tests sections. Earlier author Core1/Core2/Core3 failures remain intact and are not relabeled green; final fixes were independently rerun here.

## Source review and actual controls

Review continues immutable364fc3 and earlier50a/ad4 findings; those reports/failures remain unchanged. The final production delta from364 uses the same bounded graph-proposal helper (including event/envelope overhead), includes only stored exact goal_graph_execution in the existing fixed-execution concurrent-finalize path, and leaves other approval/setup global-idle guards intact. Unknown proposal IDs strictly parse fixed parameters and return to the original route: active=WORKSPACE_BUSY; idle=NOT_FOUND/not_started/no events. Formatter/type-only changes do not grant authority. Historical migration assertions now require latest12 and reject future13; original historical11 documents/backup assertions stay11.

The final actual suite exercised:

* New explicit Graph capability vs legacy preview/execution kind; no inference of new subject/recipient/workspace authority from old definitions. Trusted Native source, full snapshot commitment, CAS and ordered positive receipt sequence, strict typed namespaces and request fields.
* Atomic attempt/intent/sessionless reservation/job plus shared root/project/ancestor ledgers. Ordinary sessions cannot bypass shared reservation. The hard3 attempt test preserves bootstrap1+Work1+Validator1, accepts the Owner criterion only, then blocks Summary with no fourth attempt/refund. Two unrelated roots wait on the same global slot without consuming attempts or writing repeated events. A newer validated head pauses once and requires exact explicit adoption; original same-key result recovers without automatic replay.
* Real bounded FD-pinned capture of outputs after actual owned producer stop; candidate is not acceptance. Root replacement, symlink/hardlink/FIFO, original-input drift, missing pin and same-SHA inode replacement fail closed. The output hash is not invented from old v1 byte counts. IO absolute-deadline controls consume initially sufficient budget during actual preflight/image setup and prove no child/marker was created. Existing old command test file is byte-identical after removing only the one new deadline test (protected old SHA3b039348…828ea7e); current fixture SHA289b61ff…109a9d9. Exact comparison/diff retained.
* Integrity/Data/Command validators execute separate charged owned attempts from approved definitions/new Graph capability. Checked producer bytes are not counted again as new output. CommandRun retains the original unreaped child across postprocessing failures; actual stop/join is distinct from cancel requested. Real command cancel and definition revoke each reaped their actual child and blocked downstream/Owner/TaskAccepted. Wall timeout returned failed, worker_joined and actual measured runtime (1853ms), not a zero or successful deadline. Consistent in-flight DB snapshot recovery remains unknown/fenced with null actuals and no replay/refund. This recovery test is not a real SIGKILL test; the original process was joined before the snapshot was restarted.
* All required validations precede exact Owner decisions and downstream Summary candidate inputs. Complete final Owner acceptance atomically records Task acceptance_ref/sequence. Historical tamper test exercises17 cases: deleted/source/reordered Owner/capture receipts, candidateSHA, Task id/project/goal, and foreign result run/node/plan/owner/revision/taskAccepted/approval; generic Taskget/list, Graphdetails and historical finalization refuse without event growth. Four fresh final-Owner controls reject changed bytes, same-byte replaced inode, revoked and cancelled graph. Later actual filesystem drift does not erase valid historical acceptance or permit a new action.
* Raw aggregate budgets before SQLite materialization,300-node near4MiB complete history and explicit overflow refusal without dropping rows; reservation/metadata/idempotency limits. Worst128NUL request-id helper checks complete1MiB row/event/envelope boundary; normal actual proposal uses that escaped request ID. This is not claimed as an actual near1MiB approved IPC proposal. Lossless observation_json projection is display-only, matches original observation digest and does not rewrite authoritative Value/receipt/event/Owner snapshot.

## Earlier findings closure on final integrated run

| Finding | Independent final verification |
| --- | --- |
| OO-JOIN-1 / OO-TIME-2 | actual_capture_prestart_queue_exhaustion_never_spawns_or_claims_an_io_join: actual1744ms queued delay exhausted1200ms reservation, not_started/no thread/no join/no candidate/no refund. Static private CaptureCompletion preserves prepare/spawn errors separately; no actual OS thread-spawn exhaustion was induced. |
| OO-ACCEPT-3 / OO-REPLAY-5 | actual_accepted_history_and_owner_replay_reject_durable_receipt_tampering_without_file_reads:17 negative cases, historical same-key recovery/action mismatch and no event growth. |
| OO-WIRE-4 | actual_graph_owner_decision_unlocks_dependent_summary_with_exact_candidate_input: true final portable Task/GraphDetails/Owner result preserved under runtime-artifacts. Canonical connection DTO. Native final combination remains Root's separate check. |
| OO-DROP-6 | fatal_actor_error_cancels_and_joins_owned_graph_reader_before_writer_lock_handoff: actual fatal actor error, owned synthetic held reader cancelled/joined before rebind. Not a Provider process. |
| IO-DEADLINE-1 / IO-PIN-2 | both absolute_deadline_tests library units, original command regression28 and goal_validator_io11 all passed within full suite; missing-pin/same-SHA inode and postprocessing retain/no-spawn controls included. |

Eleven GRAPH_BUILD_SOURCES records each have15 compile-time source hashes matching the frozen map (embedded-source-hashes.json). Five true generated runtime JSON artifacts copied with hashes; e.g. final Owner portable `actual-task-accepted-83c88405356af86c99c9c976613d4a0dcc0f99de3cecd0da1986d8869ba63360.json`,39593B. These are real local generated-protocol/Core runs, not real paid Provider calls or human Owner approvals.

## Artifacts, limits and lease

Built Linux debug executables match immutable candidate bytes: batond104881288B SHA40322bff82aa8ec0f1c019e5d723f42694a6266bcaa27b153f535d0d21dbadcd; baton76685288B SHA66ca50a6d4a2e633f799c6275d66b9be3fe3aa35f6346649893ad925de2ee0d0. Unsigned Linux executables; not macOS installers. No model calls, credentials, paid resources, CI, PR or merge actions were used.

Output actuals cover selected declared artifacts plus retained streams, not total filesystem writes; output_quota remains false. Null unknowns are not zero/refunds. Strict unsupported enforcement still refuses. Fault controls do not prove OS cleanup cannot ever hang, induce actual thread-spawn exhaustion, or replace macOS/real Provider tests.

Root composition separately differs in five Native files; the95 identical files include all workspace production Rust/CLI/daemon/root lock. This review does not lend results to the different Native sources. Browser, real desktop IPC/UI, Mac compile/NSAlert sheet/architecture builds, code signing, real Provider and remaining full-design graph/factory work stay separate evidence and acceptance gates.

After all child processes/commands completed and source/bin hashes were checked, reviewer explicitly RELEASED the sole Native target to Root and notified author. No further Cargo/target writes by this reviewer; report/manifest only. Nothing here authorizes a release, paid call, account, merge or broader execution scope.
