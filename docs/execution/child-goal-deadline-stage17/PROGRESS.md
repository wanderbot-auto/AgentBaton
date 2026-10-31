# Diagnostic implementation and pure validation

`factory_child_goal_runtime.rs` now injects the diagnostic helper only for the existing `deadline_probe`. Before/after boundaries and three distinct reply layers use at most 32 records. Each record contains only a fixed stage/event/layer/class, relative monotonic elapsed milliseconds and closed error-code/flag/effect values. Unknown strings reduce to `other`/`invalid`; original assertions still throw and original calls run once. Diagnostic write errors do not replace the underlying result or failure.

Drop retains the atomically written marker only after owned daemon join, using a 16KiB read bound and private mode 0600. An absent marker is explicitly unobserved and proves no stage or process outcome. The fixture's historical schema header, Root17 integration fixture, budget values, Owner behavior and original acceptance/deadline assertions were not edited.

Pure Python validation passed 7/7 in 0.021s. Tests compile the actual source replacement operands, exercise instrumented MCP/tool definitions with in-memory streams, preserve one wire call and original error assertions, refuse malformed-result classification, omit private canaries and cover bounded writes and diagnostic I/O errors. These are source/metadata controls, not a Core/process deadline result.
