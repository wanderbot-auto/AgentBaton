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
