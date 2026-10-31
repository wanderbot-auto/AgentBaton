# ADR: Bound domain selection before applying Integration-only checks

A missing or foreign approval must reach the original owner-request policy. A new domain cannot impose its record lookup error ahead of that policy. Use a fixed optional one-MiB SQL read, strict typed decoding of original bytes (never a `Value` projection), and select only a genuine stored Integration kind. Reapply the original strict Integration lookup after selection. Every fallback continues through existing guards and finalizers; it cannot create authority.

Do not relax the existing concurrent-review assertions. Separately, schema17 cold-open Task-binding consistency is an additional refusal boundary: execute the original historical evidence guards on an already-valid Store before testing that startup refusal. Keep unchanged cold-open historical guards for all cases that do not violate this new startup invariant.
