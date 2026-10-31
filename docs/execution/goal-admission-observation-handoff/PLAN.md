# Owned Goal admission observation handoff

Partial requirements AB-TSK-008, AB-TSK-009 and AB-TSK-010 under AB-START-001/WP08. Preserve full Design v0.2/130 and all approval/resource boundaries.

The original ancestor-budget guard can produce a bounded typed sample in CoreError.details_ref. The subsequent recorded Goal dispatch error wrapper replaces details_ref with its mandatory recorded-goal-advance recovery reference; the original blocked event therefore cannot recover that sample on this path. Preserve the external recovery reference and all error/state/budget semantics while carrying the optional diagnostic in the existing private owned reply.

Use a non-serializable opaque per-reply observation, exact current run/revision/plan validation, the original blocked transaction, and no crossrequest cache, new channel, DB write, authority, budget, clock or IPC field. Root independently reviews then compiles/tests the three new Core cases, full applicable Core/Factory/strict/IPC regressions and exact dual-platform CI. This unit does not establish why Intel's original guard refused or that its performance is fixed.
