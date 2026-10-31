# Progress

Source frozen at `9f412458d782d9513427c2575cc7d234918a2f66`.

- Added a private parent history (at most eight entries), recorded only after the original full first verification and ACK path succeeds.
- Added bounded, typed read-only cancellation proof. A synthetic WAL test commits cancellation after the original read snapshot and proves the single fresh snapshot can observe it.
- New 11 pure controls PASS (2.256s). They call the actual extracted collector with generated SQLite/files and injected proc/argv/UID observations. They cover refusal boundaries, unchanged bytes/verified/ACK state, companion rejection and final live-empty rejection versus actual injected absence.
- Original 39 controls PASS (4.737s), original source byte-identical. Existing Root finite timing injection and late-MCP logic remain intact.
- Collector-excluded module AST is identical to base. Both changed Python files compile as source without executing their mains; diff check passed.

The first observed new-test run had two **test fixture errors**: missing traceback dependency and duplicate PID argument. Their raw log is embedded in EVIDENCE.json. Correcting those fixtures did not change product or original assertions. An intermediate 11 PASS preceded the final added raw-DB byte equality assertion; final 11 PASS is the source-attributed result above.

Original CI failure and all original artifacts are retained. Its later cancel reply does not prove a cancellation commit existed at the first empty read. This patch does not claim that old CI moment necessarily qualifies for DEFER or that the actual cancel case is now green.
