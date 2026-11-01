# Schema17 sessions routing and corruption coverage

Baseline: `4f962940533241fdec74b96a87c01a99788a62ee`, Design v0.2 and AB-START-001. Requirements: `AB-SEC-001`, `AB-SEC-007`. This is a compatibility/safety repair within existing sessions and Integration contracts.

1. Diagnose the actual 55-pass/3-fail sessions run without changing safety expectations.
2. Restrict Integration approval interception to a bounded, strictly typed stored Integration proposal; leave missing and other domains to their original owner guards.
3. Exercise all four historical corruption guards after valid startup, and retain cold-open coverage: exact Task row-ID startup refusal, and original historical refusals for the other 16 cases with the valid fixture explicitly unbound.
4. Obtain independent source review, then Root runs actual focused/full Core and sessions regressions with source-bound evidence.

No Provider calls, added resources, altered retry/budget/deadline, new queue, or finalization authority.
