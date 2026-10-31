# Progress

Recorded 2026-10-29T20:44:09.756461+00:00. Source e8adb9fe2ea68b0eccfb600d896b57573b62391d.

Original Product40947058067/UI123705085439 reached Factory completed/cancelled/outcome_unknown with 3/1/1 real attempts, then all failed the old blanket legacy Goal-table zero assertion at line1127. These remain failures. Official original ZIP/report bytes are read-only and were not downloaded again or copied here. Original diagnostics omit goal_* tables and the ZIP contains no SQLite: exact original nonzero table counts cannot be directly queried.

Fixed Core factory_runtime::reserve_attempt calls reserve_shared_execution_meter, which initializes and reserves goal_ledgers for Project and original RootGrant. Settlement adds actual runtime/selected-artifact bytes without refunding observed attempts. This path does not create Goal scopes/plans/runs/capabilities or grant history. Goal-grant revisions are written by separate Native Goal approval, absent from these fixtures. The stale test assumption equated a shared meter with Goal authority.

The new terminal proof accepts exactly those two meter keys only with approved limits, typed known cumulative usage, exact expected nonrefunded attempts and active0, and real same-scope Factory reservation/session/attempt/intent/lineage/config bindings plus ordered owned-stop then settlement events. Grant bytes unchanged; all other16 legacy Goal tables including grant history stay0. Failed preflight/unknown usage/foreign rows refuse. The existing classic budget verifier remains intact.

Legacy approval-before-start now explicitly checks all17 Goal tables0 in a read-only transaction; planning's existing all17-zero approval check stays byte unchanged. Planning's terminal global comparison exempts only goal_ledgers after the same verifier. Source/global set/old pure functions/timeout/retry/unknown paths stay unchanged. A new finite legacy table-count observation captures future failures precisely.

Actual source checks: 18 inert SQLite verifier tests PASS, existing63 legacy boundary checks PASS, existing46 planning checks PASS, three source files compiled in memory, diff check0. Original diagnostic finite bindings for all5 attempts match the verifier's config/input/policy/ancestor assumptions; this is not a reconstructed DB or product rerun. Two initial diagnostic self-check commands omitted required --output and returned argparse2 before tests; corrected commands passed.

No Cargo/Node/browser/Actor/daemon/Core execution, real Provider, remote action or product rerun performed. Data/Graph successes from the same CI are not transferred to Factory or later unrun child workflows.
