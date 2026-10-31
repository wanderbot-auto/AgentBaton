## Prior settlement association extension — 2026-10-31

The latest Intel observation is a capture settlement at run revision12 after a separate node settlement at11. Reuse the existing strict reservation/session correlation for that previous node as well; keep it inside previous_node_settlement and retain both historical/not-same-plan labels. The original helper, event bounds, redaction, source predicates and runtime budgets remain unchanged. No causal or authority claim follows from a stop reason or completed turn.

Root103 generated-SQLite tests pass, including6 new prior-binding/missing/duplicate/foreign/late/numeric controls. Independent source review passes64 checks and independently runs102 cases; its one guard-blocked old FIFO method and original attempt are separate, never counted passed. All original17 stop methods/80 collector methods remain; actual enhanced Core/Mac/CI projection is not run. See the appended EVIDENCE.json entry; old failures and source pins remain historical.

# Checkpoint

Frozen source28773f2105ed2a3a12fb0ccabfe2eae0ac3059b9 in /workspace/.worktrees/factory-rework-stop-correlation, basead400. Only own records may change while Root reviews the fixed source. No shared target or heavy execution lease is held.

Actual lightweight commands: `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests/infrastructure -p test_factory_rework_ci_evidence.py -v` (original80PASS) and same command for test_factory_rework_stop_correlation.py (new17PASS). Tests use privately generated SQLite/filesystem fixtures, not a Core authority or runtime. Original tests were not edited.

New settled_session_stops entries carry only original event ordinal, fixed availability status/reason category, nullable strict flags, nullable completed-turn count/bool and fixed derivation dimension. Internal run/node/session/attempt values, event text, DB/path and parameters never leave. Unknown or ambiguous association is not zero. Data describes post-owned-shutdown historical association, not a live deadline snapshot, TaskAccepted, causality or a permission to retry/replay.

Pending: independent source/pure review; actual collector on a future exact CI candidate's retained DB; any real Core/runtime/OS/Mac execution. Original Intel415 lacked the needed raw event data in its official ZIP; do not invent the missing stop/turn reason from its finite overrun/tag. Root owns actual binding and integration. No old PASS or budget threshold is changed to obtain a result.
