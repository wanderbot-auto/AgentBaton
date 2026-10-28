# Limited Factory runtime re-review — fixed137

Subject: `137e24698090c73349c9560868c2a38ad97c627b`, rechecking only FR343-01–04 and the known synchronous artifact-capture handoff. Previous343 review remains unchanged. Source evidence is the independently exported Git objects in `source/`, with hashes in `manifest.json`; no current Data worktree changes are included.

**The four original findings are statically closed for this fixed Git runtime slice. No new blocker was found within this limited scope.** This is not a complete Factory/runtime/Data/accounting review or a claim of independently executed product tests. No Cargo, daemon/companion, source/target write, dependency installation, remote action, Provider, recursive agent or formal approval occurred.

One mixed supporting read accidentally included two relative sed sections from the active author tree. Those sections were discarded and re-read from the137 export before forming conclusions; all source references and provenance below are fixed Git objects, not those accidental reads. The procedural correction is also recorded in `provenance.json`.

## Original finding closure

| Finding | Fixed source | Limited conclusion |
| --- | --- | --- |
| FR343-01 complete original Grant binding authentication | `factory_runtime.rs:175` | Common `live()` now invokes existing `validate_grant_live_from_with_data` on the original root Grant. That implementation revalidates its complete declared connection/workspace binding maps, not merely the narrower Factory selection. All four scoped methods reach this shared authentication boundary. The additional selected full-document comparisons remain. No private Directory witness is invented: this call supplies None, so a Directory workspace requiring a witness is rejected by the existing policy. |
| FR343-02 all sibling results and parent identity | `factory_runtime.rs:1795`, `:1774`, `:852` | `join_parent` selects every child handoff for the exact parent session, sorts by child task ID, requires actual terminal/confirmed-stop results and the parent's owned-stop receipt, binds scope/run/parent/session/all children into one digest, and stores separate `handoff_results/handoff_digest`. It no longer overwrites parent's own result with a child's result. Prompt supplies the complete aggregate and a singleton compatibility field only for one child. `factory_result` explicitly uses the requested task's current handle, preserving parent identity/availability while waiting/resuming. |
| FR343-03 cancelled queued child convergence | `factory_runtime.rs:1347`, `:1795` | Parent settlement calls common join reconciliation once its own actual stop receipt is committed in the transaction; child normal settlement also calls it. A required child that is cancelled and confirmed stopped leads to explicit blocked/CHILD_CANCELLED with no automatic replacement/retry, rather than an impossible waiting state. Unknown child outcome is kept unknown. The exact initial delegate→cancel-undispatched-child→parent-complete trigger is covered by the recorded new real IPC case. |
| FR343-04 expired ready-request routing | `ipc.rs:1606` | The common deadline predicate now precedes parsing/routing either ordinary or scoped requests. Expired envelopes return RESOURCE_LIMIT/revision0 before authentication or mutation. No rekey/replay behavior was added. |

Aggregation is bounded by row/document counts and complete escaped Provider prompt envelope checks. Oversized results block explicitly rather than truncating siblings. Reconciliation preserves consumed budget/lineage, waits for actual parent stop, and does not grant TaskAccepted, correctness or Git integration. The existing53e approval/start findings are outside this re-review and were not reopened.

## Known large capture change

`FactoryCapturePlan` and `FactoryCapture` (`factory_capture.rs:8`, `:16`) are crate-private, have private authority-bearing fields and no Deserialize/Serialize construction path. `DataArtifactPins` (`data_workspaces.rs:1583`) holds real root/leaf File descriptors and private entry metadata; it likewise cannot be reconstructed from renderer/provider JSON. Plans originate in the trusted session launch, and captures are carried in the owned WorkerReceipt.

`run_worker` (`sessions.rs:1569`) takes the private capture plan, runs the Provider lifecycle, and only performs post-stop artifact observation when the process is actually reaped and not never-started. `FactoryCapturePlan::capture` builds the approved selected input/output contracts, caps their count, obtains descriptor pins, hashes/validates bytes in that worker, and verifies pins before returning results. Input hashes are fixed to the approved snapshot. Missing/unsafe/digest-mismatched outputs cannot become complete successful output. No integrity evidence is promoted to Task acceptance.

The supervisor only consumes a finished worker after its true `thread.join()` (`sessions.rs:1505`). The Actor receives the typed capture after that join, then uses finite-count descriptor/path metadata rechecks and database work through `settle_shared_session` and `settle_factory_worker`. Its new settlement path no longer rereads/hashes large selected file bytes. Missing capture, stale pins, changed inputs, missing required outputs or reservation overruns fail closed rather than becoming normal successful settlement. Ledger and result rechecks are separate point observations; there is no claim of continuous filesystem freezing, immutable whole OS process trees or arbitrary-filesystem latency bounds.

This closes the specific structural problem of doing those large result reads on the Actor. It does **not** establish responsive large-capture cancellation/performance: dedicated delayed preflight, large-file cancellation, capture→Actor metadata drift/fault/restart and mixed-accounting regressions remain outstanding. Worker capture still must finish/join before the authority can release its receipt; moving work off the Actor is not evidence that cancellation immediately aborts every read. Existing owner scope/start preflight and shared policy filesystem checks were not audited as a new architecture project.

## Existing source/test/old-control provenance independently checked

`provenance.json` records each observed hash and retained result line. All seven `EVIDENCE.runtime_review_fixes.source` hashes match137 Git objects exactly. Supporting source including DataArtifactPins was also exported from137 and entered in this review's manifest. Every export was rehashed against the manifest.

The existing author logs match their fixed EVIDENCE values:

- `six-runtime-capture-join-final.log`, SHA256 `5c64c929a0f4e9a90b82c1b502bb759d5ef6796ecf6ad2cb31523cff46d1db90`: command `cargo test --locked -p agentbaton-core --test factory_runtime_ipc -- --nocapture`; **6 unique actual IPC tests passed,0 failed/ignored**,8.89s. Includes the former3 runtime cases plus original-Grant pin drift across all four methods, two distinct children/hash-bound handoff plus parent get-result identity, and cancelled queued child→blocked without child dispatch. The four-method drift calls share one actual session and supervisor lifecycle; the log shows all four denials, not four separately isolated sessions.
- `fixed-scoped-queue-deadline-control.log`, SHA256 `899e69d7a1fe5551e321a9c7d0d1c4c225b0b8908656f2accbb0ba1febe5105b`: command `cargo test --locked -p agentbaton-core --lib factory_queue_deadline_tests -- --nocapture`; **1 unit passed,0 failed/ignored,69 filtered**,0.02s. It loops over four expired envelopes with a missing/fake handle and checks zero mutation. This proves deadline-before-auth ordering; it does not dynamically exercise an otherwise-authorized expired delegate.

These are **6+1 directed cases** for this source. The original14 scope/migration cases were not rerun for137 and must not be borrowed into a new21-test claim. The old17 total is a historical343 snapshot; its three IPC cases overlap the new6, not an additional17. This reviewer executed zero Rust/product tests and did not rebuild or execute any saved binary.

All four old failure logs, three old source/binary manifests and every declared manifest entry independently match their recorded SHA values. Source files/manifests and raw logs were copied unchanged beneath `evidence/`; binaries were hashed in place only, never run. The old logs show:

| Old control | Observed failure |
| --- | --- |
| Original Grant omitted connection pin | Four methods returned success/result despite drift; test failed expecting AUTH_REQUIRED. |
| Multiple child results | Test failed with outcome_unknown instead of completed after the broken aggregate handoff. |
| Cancelled queued child | Parent remained waiting_child past the controlled deadline; no runnable child. |
| Queue deadline ordering | Missing-handle envelope reached authentication/AUTH_REQUIRED instead of deadline RESOURCE_LIMIT. |

Each retained log reports0 passed/1 failed. Numeric101 was reported from the author's actual shell execution; the retained text logs do not separately persist that shell status, so this reviewer independently verifies failed test outcomes rather than inventing a rerun/exit observation. These manifests intentionally represent intermediate pre-fix source/binary controls, including already-added capture or partial fixes; they are not a byte-exact replay of the entire343 commit. The scoped-deadline manifest even distinguishes its unrelated preexisting lib-test binary from the actual old-deadline control binary.

## Remaining WIP / no expanded acceptance

Directory/Data support is not passed here. Full original Grant validation currently provides no request-private Directory witness and therefore refuses it; real Data privateTicket/witness integration belongs to the next author phase and is **not_run** for137. No active Data worktree implementation is imported into this result.

Full workspace/migrations/lints, large preflight/capture cancel/fault/identity races, comprehensive negative handle/TTL/fencing isolation, ordinary/Goal/Factory mixed shared accounting and global/ancestor stress, native UI/OS owner dialogs, real Provider/accounts, macOS builds/installers and complete Design remain unverified by this re-review. Public scope remains bounded; no new permissions, signing, resources, production or formal approval follows. Later changes require their own fixed source/evidence and appropriate review.
