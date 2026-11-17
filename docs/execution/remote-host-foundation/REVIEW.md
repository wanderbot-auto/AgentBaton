# Independent review dispositions

Reviewer: native read-only subagent `/root/remote_review`; no product source writes, no test runs or run-lock use. Parent implemented all fixes; leader retains final independent integration acceptance.

| Finding | Disposition / evidence |
| --- | --- |
| Old intent could adopt renewed grant | Jobs persist dispatch fence + exact grant hash; new fence invalidates unresolved old jobs. Rust reserved-dispatch and Python actual-worker rotation tests. |
| Lock wait caused false clock rollback | Sample wall clock after writer lock. Rust deliberate lock-wait/highwater test and six parallel real workers. |
| SSH registration lacked expected Host ID | SshConfig trusted expected_host_identity; Client rejects wrong Host/recipient before sending data. Registration contract and negative test; actual handshake untested. |
| Mapping replacement / permissive journals | fstat opened root owner/type/mode, bind dev/ino, compare path; private regular owned single-link files. Root replacement, symlink and permission tests. |
| Unbounded cleanup wait | Six-second child drain then kill/wait; eight-second control transport drain/group fallback. Kernel uninterruptible IO limitation explicit in ADR. |
| Invalid snapshot retained reconciliation | Response binding validation inside invalidating error path; reconcile binds binding/fence/controller and final page. |
| Idle two seconds closed session | Idle poll separate from partial-frame deadline; real idle >2 seconds succeeds, half-frame remains bounded. |
| Spawn error falsely labeled wait | Separate spawn_failed_no_child status; actual wait evidence reserved for observed child exits. |
| Old companion deadline paused new epoch | db_pause scoped to original fence+grant hash; worker metadata writes fenced. Dual real companion regression covers old expiry/new successful record. |
| Inherited pipe FDs, known_hosts bound, config hard links | F_DUPFD_CLOEXEC; 65537-byte bounded known_hosts read; nlink on actual config FD. |
| Future schema mutated before rejection | Store and Ledger validate user_version before write pragmas. Both future-WAL journals remain WAL/version99 in regression. |

Final targeted reviewer disposition: all listed issues fixed, no remaining source-review blocker. This is source/ADR review, not a claim that reviewer independently executed dynamic tests. Parent explicit tests below remain subject to leader's review against final submitted SHA.

## Leader P2 capacity follow-up

Leader identified that first Cancel/Revoke was rejected at event9904 by the 96-slot new-effect gate. The equivalent legal9903 provisioning/reopen seed reproduced the failure on 4a2 product code ([red log](review-capacity/red.log)). The fix separates new-effect admission from settlement/control debt and coalesces only unchanged control/reopen projections.

Follow-up native reviewer caught over-counting worker-terminal debt after terminal events were already durable. Final formula is pending-wait + unsettled-terminal + first controls + necessary reopen. Tests cover both first-control orders and 32 actual owned workers with terminal events before recorded wait; metadata counts progress9904→9936→9938→9970→9971, preserving actual stop evidence and budgets. Independent targeted disposition: formula/accounting correct, no remaining capacity blocker; no reviewer test execution.

The failed intermediate verifier 20261117T152431Z-547280000 remains evidence, not a pass: one rollback fixture observed outcome_unknown before a running worker had been established. The fixture now asserts actual running before injecting its specified in-flight clock fault, preserving the original strict stop-state assertion and failing explicitly on startup failure. No unsupported claim about the failure's underlying OS cause is made.

## PR30 readiness single-test scope review

Leader separately reserved GoalPlanDraft.integration.test.tsx. Native read-only reviewer confirmed the independent second scopesList deferred plus2scopes/1plans count covers the actual scopeId-induced second refresh; pending stays disabled, only valid release enables, and reject/foreign scope stay blocked. Async act only drains fulfilled mocks; original assertions/limits remain unchanged. Reviewer did not run tests/install dependencies/hold the run lock. B subsequently ran target5/typecheck/full108files1531tests successfully, with exact UI commit/source provenance separate from remote9457.
