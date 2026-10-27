# Progress

OWNER-001/002 ACK at API-verified protected mainc4e72c6; gate remains AB-START-001 with real Provider/models, production and new spending closed. Isolated from all shared Data/Core work.

Actual baseline correction: db_error was already INTERNAL_ERROR+unknown. Connection/grant approval-confirmation trigger controls pass the safe unknown expectation and replay the original durable effect after removing the trigger; do not label these a historical not_started failure.

Actual probe scheduling fault: authenticated finalization persisted connection, owner confirmation and original registration idempotency record, then private environment rejection returned PERMISSION_DENIED+not_started with no object reference. The new test first proves durable state/executable-not-run, then fails101 against baseline. Raw log retained. After the minimal IPC-only mapping, all3 new regressions pass. Same proposal replay preserves original connection/grant IDs and confirmation journal once, with no replacement key or new owner proposal. Existing before-effect NOT_FOUND remains not_started.

Source32b2dc58d23205350aa54878463b8f824997dc02 frozen locally. Core83 top-level tests pass (one nested FIFO child excluded), Clippy/fmt/diff checks pass. No changes to sessions.rs, storage, schema, policy, dependencies/locks, UI/native, permissions or thresholds. Independent parent review and integration remain next; no push/merge performed.
