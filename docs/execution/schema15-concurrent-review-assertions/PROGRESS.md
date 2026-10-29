# Progress

Source committed as `1109d1ae87b00ba2aa3358da671a6acde80525b6`, based on exact Root `0b5a88a572d70aa87ef4aab481a62ec2bfeb0af4`.

The original serial `sessions_ipc` command exited101: 56 passed, 2 failed, 0 ignored (195.11s test /198.086837s wrapper). Both failed in `oversize` assertions, original lines3241/5974. Actual replies were `NOT_FOUND`, `not_started`, with `Goal record is unavailable or exceeds its raw-byte bound`; original log SHA256 `3aa67f9dbcdf8fbb66dd133bb9b058b348e12463a27ac9bcbb9c8c15c6e1b732`. The original failing ELF remains owned by Root; it was neither copied nor executed here.

Diagnosis: `ipc::factory_context_for_request` invokes `prepare_factory_goal_read_for_request` first. For fixed finalize/decline proposals that helper checks existence, then calls the existing `goal_store::raw` before kind classification. `raw` selects only document octet_length≤1,048,576; the appended-space document exceeds that bound, so no row is materialized/decoded and `NOT_FOUND/not_started` is returned. This happens before the existing concurrent fixed-review route and its `RESOURCE_LIMIT` classifier. It is an earlier fail-closed bounded-read result, not accepted authority or a demonstrated production guard defect.

Only `mode == oversize` now requires exact `NOT_FOUND`, the fixed bound message and `not_started`. All non-oversize errors retain the original allowlist; `NOT_FOUND` was not added to it. Original proposal-corruption writes, all row/count/result assertions, other modes, budgets, workspaces, process release/stop and Task-not-accepted checks remain byte-identical outside the two assertion blocks.

Static proof exit0, standalone rustfmt parse/check exit0 and git diff --check exit0. No compilation or actual test rerun. Later loop modes after the original oversize panic have not been dynamically verified on the fixed source. The sparse worktree avoids materializing repository archives on the low-free-space filesystem; no unknown files or caches were deleted.
