# Final source8af test-code correction

8af8ade4ef298f677b821a920b6771017351bf11 changes only the new expired-deadline assertion to TIMEOUT, the exact existing goal_evidence::check return. Independent reviewer identified the mismatch by source; a645 tests were NOT_RUN, so this is not an actual failing-test record. Production still diagnostics-only, original scheduler order/fees/lifecycle unchanged. Source formatter/diff checks0, no Cargo. New immutable diagnostics-only-8af static record supersedes current binding; older manifests and source commits remain preserved.

# Current progress — diagnostics only

Final corrected code a6450f43877bd3039858fe6b28051ec16ba84e40. Candidate priority is DEFERRED pending actual numeric phase evidence and an independently justified scheduling change. No optimization is implemented or claimed in this final source. Original full reconcile again runs before prepared-job lookup/start, with identical no-job/missing-clock/prepare/spawn failure branches. Inverting only the real join-result wrapper and real thread-execute wrapper makes the entire tick function byte-identical to baseline34f. All7 original verifier/deadline/reconcile/shutdown/Drop method bodies and the original billing block remain byte-identical.

Independent static review found2 new cfg(test) count reads as u64 unsupported by rusqlite0.38 FromSql. Final a645 changes those reads to i64 and retains exact zero expectations. This is a source-identified type issue, NOT an actual compiler failure. A first local editing script failed an exact source assertion because rustfmt split r.get across lines: it restored ordering before stopping but did not change counts. b830 consequently contains only the sequencing restoration despite its overly broad commit title; a645 separately contains the complete2-site type correction. No Cargo ran during either step.

New private timing wrapper and six numeric/null future settlement fields remain. Nine test functions remain written but NOT_RUN. Direct rustfmt --check and git diff --check pass. New diagnostics-only STATIC_CHECKS and later manifest live in a new subdirectory; original a067 artifacts, reviews, failures and manifests are not overwritten.

Below is retained earlier implementation history, superseded regarding scheduling priority/type readiness; it is not the final current behavior.

---

# Progress

2026-10-30: isolated codex/graph-capture-timing-service baseline34f8b23d4fdd529caf4f7da6f949604f2746e0e9. Remote verified wanderbot-auto/AgentBaton. Root source tree was clean and untouched. Read origin/main653679db60d5c68f7e3c5b5e146ef58aed10de23 OWNER-001/002 (latest inbox change2e0383c78d3482a0dbdea1f2e61982aabfbf91de); ACK both without changing product scope/permissions.

Code frozen a067d9123b790b6ba6793d7cc40e036ccc98618e. Only goal_graph.rs plus one private cfg(test) module; no lib/Cargo/lock/DDL/publicDTO change. Actual thread wrapper observes start and execute-return against the original queued_at, preserving result/error. Real panic join has no returned start/complete/elapsed evidence; those fields remain null. Not-started has no join metric. Known zero durations remain valid observed numbers rather than fabricated missing-value defaults.

Added the six Root-fixed numeric/null fields to future goal.capture_settled payloads. Original charge expression is byte-identical and still uses producer+max(queue-to-join observation, original captured.elapsed); diagnostic wrapper elapsed does not replace it. Earlier events are not rewritten.

Prepared capture priority is a scheduling reorder only. With an empty owned slot, a real bounded prepared-job query and original exact origin/clock/root/fence/stop checks precede starting the owned read. Successful startup then runs the original complete frontier reconciliation. No-job, missing-clock and prepare/spawn failure paths retain reconciliation/refusal. Already owned/unknown/retained handles and stopping still take their original path. All7 checked authority/deadline/reconcile/shutdown/Drop method bodies match baseline exactly. Reconciliation may now detect an unrelated damaged frontier after the separately authorized read starts; any error still fails the Actor and its unchanged fatal Drop joins the held reader before writer release. This ordering does not authorize new Work/Validator dispatch, command execution or acceptance.

Nine tests written: actual FD read with controlled pre-start/completed-thread join waits; actual cancellation; original expired deadline; byte-identical symlink replacement; real panic/join/null metrics; no-start null fields; local-clock without persistent authority cannot start; shutdown cannot start from a hint; Drop truly cancels/joins its owned wrapped reader. These tests have NOT been compiled or executed in this leaf. Existing actual IPC/Factory regressions remain necessary to prove complete queue charging, authority/unknown/recovery and positive scheduling behavior.

Actual checks only: direct rustfmt --check exit0; git diff --check exit0; seven unchanged method-body comparisons and byte-identical billing block comparison PASS. Static source hashes bound below. No Cargo, rustc, Actor, Node, browser, Provider, CI, shared-target or network execution. Performance not established. Original34f Intel capture4231ms/20B failure and all previous Mac failures retain their original evidence, unmodified.
