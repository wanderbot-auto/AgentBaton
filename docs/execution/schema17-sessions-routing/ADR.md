# ADR: Bound domain selection before applying Integration-only checks

A missing or foreign approval must reach the original owner-request policy. A new domain cannot impose its record lookup error ahead of that policy. Use a fixed optional one-MiB SQL read, strict typed decoding of original bytes (never a `Value` projection), and select only a genuine stored Integration kind. Reapply the original strict Integration lookup after selection. Every fallback continues through existing guards and finalizers; it cannot create authority.

Do not relax the existing concurrent-review assertions. Separately, schema17 cold-open Task-binding consistency is an additional refusal boundary: execute the original historical evidence guards on an already-valid Store before testing that startup refusal. Keep unchanged cold-open historical guards for all cases that do not violate this new startup invariant.

Fixture correction: this test intentionally uses a legacy direct workspace without a Task-managed binding. Assert that precondition on the valid accepted Task. A changed project must still fail every original historical acceptance guard after both valid startup and cold reopen; only the changed Task row ID triggers the unconditional startup invariant. The previous expectation of a project startup refusal was incorrect and is retained as a real failed run in the progress/evidence history.
