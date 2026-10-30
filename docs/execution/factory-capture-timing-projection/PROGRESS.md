# Progress

Implemented a fixed six-name tuple and nullable strict `integer` calls inside the existing capture event branch. Historical missing fields become null. Real zero remains zero. Any present boolean, negative, float, oversized integer, string, list or object uses the existing invalid-evidence path; an invalid partial field is not erased into null. Unknown payload keys are never forwarded.

A fixed `capture_timing_status` is observed when at least one field has an actual integer (including0), otherwise not_available. This says nothing about completeness or causal attribution of the other segments. Partial fields remain independently unknown; no unsupported ordering or all-or-none production constraint is added.

All previous test classes, containing66 test methods, remain byte-identical. Six added methods check exact u64/zero projection and immutable input bytes, historical missing/null/partial fields, strict malformed-field rejection through the real collector, nested secret exclusion, failed/not-started receipts without fabricated joins, and no whitelist expansion on other event kinds. Existing actual_runtime_ms/accepted/io_completion/worker-join values remain separately projected unchanged.

Baseline dedicated suite:66PASS in0.216s. New dedicated suite:72PASS in0.323s; direct invocation after placing new classes before the unchanged unittest main:72PASS in0.277s. A whitespace-only EOF warning from git diff --check was corrected, then exact final source direct invocation passed72 in0.284s/exit0. Original logs are retained. These are repeated executions of72 unique methods, not cumulative pass counts.

Only generated own files/SQLite were read by tests. No product process, Cargo, Node, browser, CI run, real Provider, network mutation or external credential was used.
