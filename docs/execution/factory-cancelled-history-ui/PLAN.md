# Cancelled derived-child history

Requirements: AB-TSK-009/011/012; SEC002/003. OWNER-001/002 ACK; gate AB-START-001 applies.

Base: 5fd328f5267141a0779ccc3b2773e2c83e5f340d. Keep the successful exact cancel/drop/same-key recovery and all mutation/CAS/Unknown/stop/budget rules unchanged. Fix only structural reading of the terminal cancelled derived child, then independent source review and Root-controlled types/tests/build/real browser.

Allowed source: factory-guards.ts and dedicated FactoryRuntime.cancelled-history.test.tsx. No Core/Native/API/driver/locks/STATUS changes, no heavy execution or credentials.

## Supported continuation follow-up — 2026-10-29T18:12:03.606088+00:00

Independent review requires known continuation semantics for the cancellation read exception, matching the existing accepted-result branch. Keep the fix to one supported-contract predicate and one distinct regression.
