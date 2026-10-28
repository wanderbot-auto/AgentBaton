# Independent limited re-review: Factory Data controlled restoration (0ef)

**Disposition:** no new must-fix found within this finite delta review. The two 053 gaps have source-level repairs. Cooperative dataset-read cancellation/deadline has two author-executed library controls; the elapsed-footer repair has no metadata-stall experiment. This is not full Factory, accounting, UI, macOS, security certification, or product acceptance.

Fixed source: `0efda24e092e630d84e227d06d43b38442672ffe`. Baseline: `053b4327bb580cf42cb0fe28688f1465d6c8ce22`. All code citations below refer to the fixed export under `source/`, not the author's subsequent worktree. The prior 053 report and its manifest remain unchanged. Their fixed Git copies are included in this export; the 053 report SHA-256 remains `57a6febb842545f0925c010f1b8b54077063253eb5d237990fd059195eff0b71`.

Review scope: TSK002/003/004/007/008/012 and SEC003/010 only insofar as the original private-ticket/Data freshness, cancellation/join and elapsed-receipt findings. No Cargo, daemon, shared target, provider, remote, source, STATUS, permission, or public DTO changes were made. No product tests were independently executed by this reviewer. This review does not reopen the earlier 53e Scope admission or 137 FR01–04 audits.

## Scoped findings and closure evidence

| Item | Fixed source and trigger | Static conclusion | Experimental boundary |
|---|---|---|---|
| 053 known restoration cancellation/deadline gap | `crates/workspaces/src/data_workspace.rs:1429`, `:1436`, `:620`; `crates/core/src/data_workspaces.rs:1088`; `crates/core/src/data_workspace_workers.rs:421` | The new controlled entry shares the original restore implementation, contract validation, managed/destination pins, publication checks and copy verification. It passes the existing `CopyControl` into the original 64KiB reader. Checkpoints run before each read and immediately after it; deadline/cancel refusal precedes returning a verified workspace. Core passes the same atomic cancellation flag and `min(original request deadline, bounded worker deadline)` across contexts; no refreshed deadline or caller-selected root is introduced. | Author's two corrected controls cancel/expire at the first actual 64KiB data read of a generated 192KiB dataset, return the expected errors, and retain the original bytes and valid ordinary restore contract. They do not interrupt blocked kernel I/O or prove every metadata-read boundary is preemptible. |
| Private pending-watch and Owner withdrawal | `data_workspace_workers.rs:144`, `:309`, `:364`, `:421`; `factory_runtime.rs:1936`, `:1950`; `ipc.rs:576`, `:585`, `:1647`, `:1673`, `:1743` | Scope ID/revision comes from an authenticated private preparation or non-deserializable ticket; it is not accepted from an added wire field. The watch only signals stop. Owner cancellation/revocation and expiry/current-scope checks do not need fresh Directory bytes. A pending worker remains owned until `is_finished()` and `join()`, then its result/request is retained. Admission still checks exact raw-request digest/original deadline and re-enters full scoped authorization or typed dispatch; the watch does not grant dispatch, approval or an Agent stop-only bypass. | The new real-IPC input-drift test verifies all four scoped methods refuse, no child is dispatched, the existing attempt is preserved, and Owner cancellation reaches confirmed stop and zero held workspace leases while the source remains unchanged. It is **not** a large hash actively in-flight cancellation/revoke/expiry matrix. |
| **FD053-01 (P2): elapsed dropped on boundary early return** | `crates/core/src/sessions.rs:1578`–`:1619` | The boundary check and inner worker/capture paths now live inside a receipt-returning closure. Every ordinary return from that closure, including `data_observation_expired_before_worker` with `never_started=true`, passes the single footer computing actual `started.elapsed()` plus prior prelaunch time with saturating arithmetic. Inner worker early returns also pass this footer. A panic is not a returned receipt and retains the existing unknown/join handling rather than being certified by this observation. | **Static repair only. No independent or author synthetic metadata-stall accounting control was run.** This report does not claim experimental closure of that scenario. |

No new exposed ticket endpoint, deserialization implementation or public authority constructor was introduced by this delta. The original typed raw/nonce/deadline binding, launch plus fixed-prompt authorization, FD-bound witness, full original Grant checks and fixed selected-artifact result semantics are retained. Neither discovery nor an integrity result grants TaskAccepted.

Five apparently large accounting/policy changes are relocations, not a newly reviewed budget policy: the signature/body bytes of `reserve_shared_execution_meter`, `settle_shared_execution_meter`, `remaining_shared_runtime`, `charge_factory_preflight` and `observe_grant_bindings` are identical in 053 and 0ef. Independent comparison results and hashes are in `provenance.json`. No conclusion about untested mixed ordinary/Goal/Factory accounting follows from these moves.

## Evidence actually checked

The reviewer independently compared all **20 exported files** against fixed Git objects, all **9 recorded changed source hashes**, **13 preserved author logs**, **45 tool-manifest source hashes** against 0ef, and both frozen tool files against their manifest hashes. The verification script and captured command/stdout/exit are preserved; the final verification command exited **0**. These are static/evidence checks, not another product-test execution.

Author logs support **25 distinct cases**, not 25 newly added tests and not the sum of repeated runs:

- `data-controlled-twenty-two-fixed.log`: migration **3**, runtime IPC **8**, Scope **11**; all passed (0.04s / 12.83s / 9.31s).
- `controlled-restore-unit-fixed.log`: **2** passed (1.09s).
- `data-controlled-queue-unit.log`: **1** passed (0.02s). This is the existing expired **fake-handle** gate-order test; it is not an authorized real scoped-request expiry control.
- `data-input-drift-first.log` repeats one of the 8 runtime tests and is not counted twice. Earlier 14/17 cases or wider regression totals are not borrowed as extra cases for this source.
- The author records all-target Core/workspaces Clippy with `-D warnings` and formatting success. Existing logs were hash-verified; formatting stdout is empty. This reviewer did not rerun those commands.

Failures are preserved without being relabeled as old-product controls:

- `controlled-restore-unit-first.log`: **0 passed / 2 failed**. The initial hook expected 65536 bytes but first observed a 1314-byte publication marker. The fixed hook skips marker-sized reads and only triggers on the actual 64KiB dataset read; neither test nor its expected cancellation/deadline error was removed.
- `data-controlled-twenty-two.log`: two `u64: FromSql` compilation errors in the new test; no tests executed in that attempt. The test query uses `i64` afterward.
- Initial Clippy findings and the corrected/final logs remain. Helper relocation and Option/type-alias changes are not evidence of a lower acceptance threshold.

## Tool provenance and prior misattribution

The frozen current candidate manifest SHA-256 is `69b6744dcf85b26ed7b7308c0f6c7d04ee45f409545db40034724b22bd87dca2`. Its 45 source bindings match fixed 0ef; its build-log SHA matches `controlled-current-forced-binaries.log`, which records compilation of Core, `batond`, and `baton` before the author copied the tools. The reviewer hashed the frozen files without executing them:

- `batond`: `081a1e489952a2dc71ecd034930e1ce8cb3927f639a29f625116e101df0f0307` (111270496 bytes).
- `baton`: `89eb42f559f0df9e4958495a8b9235da7fb54c23727d9bbc079f55d853812a13` (80838848 bytes).

These are WIP Linux runtime candidates, not release/Mac packages, signatures or proof of reproducible builds. The tools remain in the author's candidate directory; this review copies their manifest and logs, not a second large binary set.

The earlier later-frozen `fc5d33…`/`9065fa…` pair belongs to old 5541 assets copied by Native's tauri-build sidecar step, **not** to 053/schema13. The original incorrect manifest and correction are retained by the author. The seven 053 runtime logs predate the overwrite, but the original raw 053 tools were not frozen before replacement. This review does not retroactively authenticate those old tools as 053 or borrow them as 0ef evidence.

## Remaining boundary

Not run or not established here: an actual metadata-stall elapsed regression; Owner cancel/revoke while a large Data hash is actually in flight; authorized real scoped queue expiry; large capture/preflight cancellation/fault matrix; mixed ordinary/Goal/Factory accounting and full workspace regressions; macOS/native Owner UI flow; complete Factory runtime/requirement acceptance. Bounded Owner propose/finalize/start Git/companion reads still need the separately recorded worker work. Cooperative checks do not guarantee cancellation of an uninterruptible filesystem syscall, full filesystem quota, continuous content freeze, escaped-process containment or native context continuation.

Within the finite source delta, the original cancellation and elapsed implementations are repaired as described. The remaining experimental dimensions must stay explicit rather than being marked passed.
