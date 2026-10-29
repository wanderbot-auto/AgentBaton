# Independent Graph driver review — final a941

Verdict: PASS for the reviewed Linux fixture/adapter changes and the precisely attributed retained local runtime evidence. No remaining blocking finding in this scope. This is an independent code/evidence review, not a formal GitHub Approve or proof that the complete product is accepted.

Final source: `a94140ebebf22624899b8b815d868f2971bb51f4`, `apps/desktop/tests/goal-graph-real-ipc.py`, SHA-256 `4bc91fde76a61c4fdb829f3844f06cca20f94238add92d5b60bedb0edd00adc5`. Author documentation: `bd64120a81a8bbd933847e8a36cbc965f08c9e57`; exact four documents are copied under `final-a941/author-docs/`. The Root integration script was independently hashed and has the same final bytes. Initial subject `90d12ff1d60534f53ff45733a3ccf44322f5cc83` was BLOCKED; its original 17-file manifest/review remain unchanged and do not become green retrospectively.

## Scope and resolved findings

The driver retains the exact 15-field legacy TaskRequest contract and the paired 17-field Graph extension (`validation_contracts`, `task_binding`). Private fixture proposals/Owner decisions stay outside the finite HTTP bridge. The browser cannot call Graph approval, node accept/reject, generic IPC or approval finalize/decline. Limits, exact action-specific replay/conflict, scoped identifiers/CAS, complete review snapshots and read-only SQL provenance checks remain mandatory. Read-only BrokenPipe may be recorded as an aborted reply; mutation/other handler failures cannot produce a passing report. No product, default budget, authorization, Provider account or credentials were changed by this reviewer.

Four substantive initial90d findings were independently reproduced with actual retained Data97c/Command526 packets and fixed narrowly in 90b/7d80:

1. Strip only the known derived `observation_json` field when comparing the complete original stored receipt. Hash the original observation UTF-8 directly and compare the full typed parsed Value. Do not claim JSON parse/stringify generally reproduces Rust f64 bytes; preserve opaque u64 values as Python integers and exact original text.
2. Owner-only Graph external input is `file`; validation/bootstrap remains `inputs/file`. The generated provider's unit outputs were separately repaired to contain the actual validator-reference shape rather than loosening the runtime path guard.
3. Owner Summary references are selected from the approved node's actual required producers, including Data `independent-4`, rather than a hard-coded `independent-2`. The first Owner sees complete original nonempty validation receipts; the final Summary Owner sees the correct empty validation list for its distinct producer.
4. Meter all output artifacts, keeping individual pins/FD/SHA checks and ledgers. Actual Data has 633 + 58 + 43 = 734 bytes in the first producer, not only its first 633-byte artifact.

Further real-run failures were preserved and corrected separately:

- 7d80's `output: Path` was shadowed by a loop variable, causing screenshot/finally TypeError and masking the first exception. c3e changes only that local name; AST verifies no assignment to the function's output parameter. The original first exception remains unknown.
- c3e Integrity correctly failed closed when the private fixture's 10-second Grant expired after 13.34 seconds. fd165 aligns only that fixture's initial lifetime with the existing Data/Command 30 seconds. Goal/runtime 10 seconds, node limits, normal500ms/timeout1500ms/cleanup500ms, observers and resource controls are unchanged.
- fd165 then reached TaskAccepted but failed the whole run on an incorrect budget expectation. Diagnostic6a8 exposed newroot4/project4/oldroot1, active0. Retained actual seq48 `goal.budget_history_reconciled` seeds1 from shared persisted root-Grant history (`reservation_made=false`). Core's existing same-root-Grant test/source confirms bootstrap history must carry across Tasks. 160e corrects exact fixture expectations to normal4, negative3, Owner-only2, always active0; it does not accept alternatives or change Core. Summary does not run in the negative cases.
- 160e Command-timeout failed its observer even though actual Core events showed a started worker and failed settled receipt. Two independent real reviewer-owned Popen probes proved this managed proc mount lacks `/proc/<pid>/task/<tid>/children`; no kernel-build or permission explanation was invented.

## Final observer repair and independent execution

A941 prefers private task-children files. Only when that feature is absent does it enumerate at most8192 numeric PIDs and read at most4096 bytes of stat metadata to filter PPID. It reads command line/PGID/full start identity only after a matching private parent; before/after identity and live-state checks reject reuse/reparenting/zombies, exact argv must be `/usr/bin/sleep`, `3`, and PGID must equal child PID. The parent start identity is rechecked. Private argv reads are bounded1024 bytes, diagnostics retain at most64 samples/16 private candidates. Permission/unexpected errors propagate and are recorded; they do not mean no child. Cleanup observer errors explicitly fail the report while allowing daemon cleanup to continue. Original five-second observation and product1500ms deadline, actual original-identity-gone assertion and no-kill successful cleanup are preserved.

Independent commands, all in reviewer evidence directories, no Core/Chromium/Cargo:

- `python3 observer-a941/controls.py`: exit0, 13 checks. Four actual short-lived private Popen cases cover matching sleep3, wrong argument, wrong argv0, missing private process group; each process is terminated/reaped in finally. Positive detection57.7ms, actual3877 PID metadata records. Wrong parent/foreign PPID does not open argv; injected PermissionError propagates with diagnostics. All35 other preexisting top-level declarations retain identical AST.
- `python3 observer-a941/guard_controls.py`: exit0, 4 checks. Explicit synthetic8193-PID and4097-byte stat cases fail closed. Synthetic task-children discovery takes precedence over fallback, with an actual private child's identity still verified and reaped. These are boundary units, not actual daemon/validator runs.
- `python3 final-160e/run_light_checks.py`: exit0, actual58 boundary /15 driver-FD /9 transport /35 validation-plan /37 supplied real-packet controls. These counts are source160e, not a fabricated a941 rerun. Provider5 synthetic unit was actually run at7d80; the generated program/test function was mechanically byte-identical at160e. Original provider90b unit failure is retained honestly as an observed-error record; no original shell log existed.
- `python3 final-a941/decode_runtime.py`: final exit0; independently decodes six copied Root reports,34 runtime checks/14 PNGs. Initial decoder incorrectly assumed negative reports retained reserved/intent/stop event objects and raised KeyError. `decode_runtime-initial.py`/`runtime-decoder-initial.log` are retained. Only the reviewer decoder changed to validate negative retained capture event and candidate stop sequence; product/driver/raw reports were unchanged. The actual driver still checks the stronger SQL stop provenance during the run.

## Precisely attributed actual Linux runs

Root executed the real newly frozen Core70a binary and composed UI, with fixture-generated local ACP processes, not a live/private Provider account. This reviewer independently decoded the retained results and source/packet hashes; did not rerun these browser scenarios.

| Mode | Actual driver SHA subject | Checks | PNGs | Result |
|---|---|---:|---:|---|
| Integrity | 160e / `9b1bc1…c452` | 6 | 3 | PASS |
| Data | 160e / `9b1bc1…c452` | 6 | 3 | PASS |
| Command | 160e / `9b1bc1…c452` | 6 | 3 | PASS |
| Data-range negative | 160e / `9b1bc1…c452` | 5 | 1 | PASS: actual validation failed, Owner/Summary/acceptance blocked |
| Command-timeout negative | a941 / `4bc91f…adc5` | 5 | 1 | PASS: actual owned command timeout/stop/join/gone |
| Owner-only | a941 / `4bc91f…adc5` | 6 | 3 | PASS: four nodes, zero validators |

Final observer-only changes preserve the other35 declarations and unrelated branches. The four160e passes remain attributed to160e; this table does not claim a full six-mode a941 local replay. Future CI must run all six at its one actual checkout. No old5b/Core5541 green substitutes for this Core70a run.

The retained candidate manifest SHA is `c9fe6d7a517b15baf1386fc6111bc7413983c22e9f299c35ece242afdea9ba06`; actual batond SHA `40322bff82aa8ec0f1c019e5d723f42694a6266bcaa27b153f535d0d21dbadcd`,104881288 bytes, was independently rehashed. The manifest's original creation-time verification fields remain as written; they are not evidence of later full-workspace results by themselves.

The independent decoder rehashes raw TaskRequest UTF-8, validates15/17 exact keys/task binding, direct observation text/result_digest and complete typed projection, actual validation worker prepared→started→attempt-stopped→settled sequence/digest binding, absence of validator Provider session/legacy policy execution, full original Owner review refs and opposite-action conflict/replay, durable TaskAccepted refs/sequences/event digest, exact4/3/2 budgets and total artifacts/retained validator bytes in ledgers. Command normal retains22 stdout bytes; Integrity total output38, Command60. Actual timeout child814635/parent814349/PGID814635/startticks7698052/argv sleep3 was observed; its original identity is gone without reviewer/driver cleanup signaling it. Product wall1500/cleanup500 remains in its actual receipt. Runtime1870ms includes actual dispatch/settlement accounting; it is not a changed product threshold.

All six reports independently show thread_alive=false, joined request handlers, daemon wait exit0/no SIGKILL, and the exact generated worker identities matched to real TaskRequests gone. Negative cases have no Owner decisions/acceptance, no downstream sessions. All14 PNGs were actually opened with original `view_image` and forwarded through `image()`. The Owner buttons are visible within the viewport and match unobscured DOM samples; accepted screens show all-required completion, negative screens show Blocked/Failed Validate with acceptance pending. These are Linux Chromium screenshots, not macOS/WebView or OS dialog proof.

Temporary runtime DBs/workspace output files were removed by the driver's normal cleanup. SQL original serialized receipt/candidate/acceptance hashes are cross-bound to retained event/receipt hashes; this reviewer does not pretend to rehash missing original SQL bytes by re-encoding parsed JSON. Retained TaskRequest raw UTF-8 and original observation text can and were independently rehashed. No SQL write/receipt forgery is used by the harness.

## Supplemental Native/CI scope and limits

The separately frozen `native-ci-followup/REVIEW.md` and20-file manifest remain unchanged (report `4c85952dcd07b61eba0cd459fb287a90ae7ed74966b87ce1ee487a5f8d2839ee`, manifest `8c51df9f4379b344ce48b37e7d5796b2e57199d4dd328da4b22516699eeaf524`). It independently checks Root's exact94 source-before/after entries and actual Linux Native57lib+4Ownercontroller+22readonly /Clippy/fmt logs, UI564/37+build logs, initial missing-daemon-env Python error plus configured36 PASS, and the four-line CI unit step. The earlier20-line CI wiring review preserves all old workflows/check names/default release audit/packages/permissions/runners/25-20-45-minute budgets; the four-line addition only binds the11 new Python Owner-runner units to freshly built batond.

NOT verified here: actual new macOS NSAlert/Owner interaction, dual-architecture Graph Native and lifecycle runs, CI seven checks for a final new checkout, signed installers, new Apple Silicon/Intel DMGs, minimum macOS compatibility, human acceptance, real paid/live Provider calls, mutation lost-reply behavior, the remaining broader Graph drift/cancellation/rejection matrices or full Goal acceptance. No formal GitHub Approve, push, CI mutation, merge, production release, credential use or spending occurred in this review. Those limitations remain explicit prerequisites for their respective acceptance claims.
