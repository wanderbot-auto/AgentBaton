# INT-READ-OPT-001

Exact implementation base: 499b7aa7151820310460f8acd0ff4c7e00a2caf6. The task was initially created at stale local tracking cbf; no implementation or testing happened there. Leader normally fast-forwarded the clean task and tracking ref to499 before implementation, preserving creation history. Baseline source SHA-256: 4d2f9fb5eaf86dbde7ebe403ce252d64f5dc25a8e4bb150565fecd891a860778.

Only integration_reads.rs and this evidence directory may change. Compute local immutable material digest at its original comparison, reject mismatch first, then compute/compare available digest; reuse those two validated values at proof construction. Keep the canonical sha algorithm, all fresh reads/Grant/Git/Fs seals, deadlines, budgets, unknown/no-refund and original assertions unchanged. No scheduling/SQL/IPC/native/fixture change.

First prepare a local implementation commit and independent static review. Resource slot is now released. On clean bound source, record complete before/after tracked input hashes; build same-source normal tools, freeze and verify them; run the two original failed integration scenarios once each with original budgets, plus nonzero existing read/witness/Grant rejection controls and appropriate format/clippy checks. Preserve any failures. No additional mirror test is needed for pure elimination of repeated computation.

A same-input cost observation is optional and must be bounded and honestly scoped. No measurement is attributed to the full191ms read phase or the old unmeasured409/425ms; do not claim this fixes11ms overrun. No push, parent-ref change, CI, merge or GUI in this package.
