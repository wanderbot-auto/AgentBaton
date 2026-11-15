# Source work completed; runtime validation pending

The Root17 rework fixture before this change was byte-identical to the parent of reviewed `58fad19`. Its `Ok(reply)` path asserted no error, while only transport `Err` consumed the existing bounded reconciliation pool. Intel returned an actual `WORKSPACE_BUSY`, `retryable=true`, `side_effect_state=not_started` reply at old line 368.

The single-file patch now matches the reviewed source byte for byte: SHA256 `d71d3c5a259b8d1f80a3cc7ab5e04fc0ad7ac7ab0d9434f04675ec6027aba7a2`. It permits only the three original bound readonly methods, records finite Busy facts, and shares the original two retries with transport errors. Other errors, unknown effects, late replies and writes remain refused. Three pure controls cover those limits; they have not been executed here.

`git diff --check` passed. `git diff 58fad19 -- crates/core/tests/fixtures/factory_child_goal_rework.rs` was empty. No other source changed. The existing Root integration fixture and all runtime assertions remain unchanged.
