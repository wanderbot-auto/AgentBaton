Next: retain actual0b64714/143/30/strictClippy/fmt plus source review; execute exact remote dualMac/native/UI/build/audit/package gates and diagnose any realfailure without changing budgets. Continue full130 and separateCore17 integration.

# Capture completion service — bounded post-start poll

Scope: Goal/Factory bounded rework and original stop/cancel/budget safeguards (AB-TSK-009/011, AB-VAL-003/007, AB-SEC-007 source slice). Base030a4f5ab406eec26adcd81463fa1c82d9752360; isolated tree/branch capture-completion-service. Only goal_graph.rs, its existing capture_service_tests file and these four records may change. No schema, public IPC, Cargo, lock, permission, reservation or timeout changes.

## ADR: one additional nonblocking service opportunity

Intel head030 finite evidence proves Work runtime4063 against original4000: producer3434 plus observed capture queue629. The actual capture thread starts/completes at queue356, leaving a reported273ms completion-to-observed-join interval. The interval has not been separated into mark-started validation/commit and later Actor work. This is not proof of CPU, CLI size, a particular request, or a general latency cause. The unrelated old60ba ancestor admission candidate remains separate.

Extract the existing finished-handle poll loop verbatim. Keep its original tick position after validator service. Keep full frontier reconciliation → exact capture preparation → spawn → owned handle insertion → successful durable mark-started. Then poll finished owned captures once again in the same tick, using exactly the same real join and Store settlement path. An unfinished handle remains owned without a blocking join. A failed mark propagates before the additional poll; fatal Drop still owns and joins its handle. Existing cancellation, missing process-local clock, panic/unknown, shutdown and all fresh origin/plan/lease/fence/stop checks remain intact.

This observes a real completed read sooner if it has already finished. It never records an earlier fictional join, discounts queue time, changes elapsed arithmetic or renews a deadline. Producer + max(observed queue-to-join, worker elapsed), all six nullable timing fields, actual fees and overrun rejection remain byte-identical. Successful start must dominate the additional poll. The previously withdrawn a067 spawn-before-full-reconcile ordering is not reused.

## Validation boundary

Four new private real-thread/FD controls directly exercise the shared production helpers. Existing ten controls remain byte-identical. They use an intentionally unapproved origin and empty Store, so they prove service/ownership and verifier refusal, not successful Graph authorization or accepted candidate creation. Root must run actual same-start-receipt→candidate normal/rework/Owner/parent chains, cancellation and overrun regressions before acceptance; a synthetic callback/forged Graph/stop row is not used to claim this dimension. No measured performance gain is claimed before an actual controlled run and both remote platform checks.
