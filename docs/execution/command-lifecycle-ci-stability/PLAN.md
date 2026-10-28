# Command lifecycle CI stability

Baseline: `652c5f0bc4983f53375ccb10d897c1088cd0e57f`; branch `codex/command-lifecycle-ci-stability`. This unit changes only the named lifecycle fixture in `crates/core/tests/command_validation.rs` and these four records. Product logic, authorization, locks, migrations and deadlines are unchanged.

Requirements: existing fixed-command independent validation / unknown-result / trusted owned-stop contracts (AB-VAL-004/006/007 and AB-TSK-003/008 partial). A command result cannot establish Task acceptance. This is a fixture repair, not a new product feature or a complete requirement sign-off.

1. Preserve actual Intel CI failure and add bounded diagnostics before assertions.
2. Run the original 1000ms positive grant locally, then reproduce expiry with a real generated ACP session/new response delay.
3. Keep the original expired negative grant1000ms + wait1100ms byte-identical. Separate the malformed-protocol positive with an approved 10000ms grant, retaining provider startup/stop and observer3s limits.
4. Require actual initialize/session-new, malformed response marker, failure reason, executor stop/reap, unknown Task, settled non-refunded budget and no new attempts. Revoke the same runtime grant after owned stop; independent command approval/execution must still succeed without accepting the Task.
5. Run full14 command integration tests, Core all-target Clippy, formatting and diff checks. Freeze source/hash evidence; release cache for independent review and later real dual-architecture CI.

Reversible engineering decision: expiry and malformed protocol are distinct fixtures. A short positive TTL must not silently convert the test into a policy-stop test. An explicit post-stop runtime grant revocation preserves the independent-command-under-invalid-runtime-grant boundary. No test accepts both cancelled and unknown states.
