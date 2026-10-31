# Outer-pass service for committed owned Goal terminal turns

Baseline: `52dd3463efa3c104c58cf5e82bf752302384c189`; Design v0.2, AB-START-001. Requirements: `AB-TSK-007`, `AB-TSK-010`, `AB-TSK-012`, `AB-ACP-005`.

1. Retain exact PR18 Product41149970320/Intel124383908298 failure and inspect its finite evidence.
2. Distinguish the already-exhausted original 4000ms producer/evidence reservation from missing per-segment causal evidence.
3. Service the existing committed Goal stop hint immediately after the original outer Supervisor tick; preserve fresh validation, real owned cancellation/reap and original full sweep.
4. Independently review two real-Actor fault-boundary test sources; Root compiles and runs them plus original stop/Core/Factory regressions and exact-candidate CI.

No reservation, budget, TTL, deadline, retry count, channel, worker, identity or dispatch authorization is changed.
