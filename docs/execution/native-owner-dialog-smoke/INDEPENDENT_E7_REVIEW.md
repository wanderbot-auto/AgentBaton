# OS driver fix independent review — e7f3265

Locked source: `e7f3265d4da38fc772193dfcfc0b35fff327d3ac`. Prior bc0 report and failures remain unchanged. Scope is the three-file fix and its frozen production/test boundaries. No Cargo, target writes, CI, macOS GUI, Provider/model calls, private accounts or authorization actions were used.

No blocking finding remains demonstrated in the reviewed Python/Linux control scope. This does not sign off the uncompiled Rust/macOS driver or claim an OS Grant dialog passed.

## Actual independent verification

From the immutable review snapshot `source/`, ran:

`AGENTBATON_OWNER_FIXTURE_DAEMON=/workspace/.onboarding/goal-graph-owner5541-native94157/batond python3 -m unittest discover -s apps/desktop/src-tauri/tests -p 'test_native*.py' -v`

Result: **36 tests passed, 0 failures/errors/skips, 5.132s**. Log: python-tests.log. This includes the prior 25 preserved tests plus 11 current controls. The real-Core test requires an actual daemon and fails rather than silently skipping without it. The controls exercise actual private process groups, forced SIGKILL rejection, exited app with a live owned child, injected observation failure and already-reaped identity no-signal, exact event sequences, and real migration DB backups with changed schema/job/table controls. Some field validator controls use explicitly synthetic JSON; these are not OS or production Grant acceptance evidence.

Additionally independently invoked the extracted same bootstrap context with frozen schema12 daemon SHA `fc5d33dca48d4b46dc7c5aa8eaf2235be8222f04ea959ba139561c0fa380d77e`. Actual read-only database: schema12, sequence8, 19 exact zero tables and pending unconsumed Grant. Bootstrap finalized only the generated local --version connection, no Grant. Original bc0 database reader actually failed `no such table: goal_plans`. Mutating only backups of the real database: a running Graph job, schema13, or a missing goal_plan_heads all failed closed. The original live DB stayed pending and unchanged. Daemon was actually running before cleanup, SIGTERM to its original private group, exit0, group quiescent before leader reap, no forced signal/errors. Evidence: actual-bootstrap/result.json, controls.json, pending-readonly.json, core-bootstrap-transcript.json, daemon-cleanup.json. This is true Linux Core bootstrap; no dialog was opened.

## Findings closure

* **OS-HARNESS-1:** database now enumerates the exact real 17 Goal tables plus attempts/sessions (19). No fake goal_plans DDL and no if-exists fallback. The permanent test uses a real Core-created migration database, verifies all actual goal_ tables match, and corrupts only a backup. Independently reproduced old failure and new negative controls.
* **OS-HARNESS-2:** success result is written after app cleanup and after the bootstrap context finishes daemon cleanup. An app must already have exited0 with its whole original group quiescent; a daemon must have been alive before expected graceful shutdown. Unexpected pre-exit, nonzero, SIGKILL, observation errors or leftover live descendants deny success. The unreaped original leader reserves process identity until group signals end; the already-reaped path refuses signals. Rust screenshot child now has an owned RAII guard across try_wait errors. That Rust path is statically reviewed only and remains uncompiled/unexecuted here.
* **OS-HISTORY-3:** exact persisted execution_grant.approved sequence is baseline+1; Owner confirmation is baseline+2, with original source/revision/payload binding. Missing, stale, swapped and Boolean sequence controls reject. These field controls are synthetic; actual macOS Grant commit sequence remains to be tested on macOS.

## Boundary preservation

Seven whole frozen files (production lib/approval/native_parent, old smoke, old runner/script/tests) are byte-identical to bc0; the existing old Python25 file is unchanged. Cargo manifest and lock are also byte-identical to bc0. Therefore the earlier two optional locked macOS dependency edges have no new versions/checksum/package changes in this fix. source-binding.json records all hashes and frozen daemon identity. No production default artifact/approval path was broadened.

Typed own-sheet APIs, exact full message/button/layout checks, host read-only DB permit before real public button click, production grants_create, clipping failure and default artifact exclusion remain as previously reviewed. The screenshot RAII fix does not prove macOS capture succeeds or cleanup never hangs under an OS fault. No new alias or direct Grant finalize was added. Pending proposals are not described as rolled back when private finalization never starts.

## Still not run

Four Rust controller tests and all macOS compilation/GUI/window retention, screenshot readability/clipping and actual Cancel→same-key OK Grant acceptance remain NOT RUN in this review. Linux tests, cached API inspection and synthetic JSON are not substitutes. Root's eventual existing CI/macOS evidence must bind the exact final source and artifacts. There is no final Graph/full product acceptance here.
