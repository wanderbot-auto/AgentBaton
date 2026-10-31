# Historical Cargo cache isolation

AB-START-001 / Design v0.2; OWNER-001/002 ACK after assigned-baseline AGENTS/gate/inbox reads. Fixed baseline 82773a3b78e9fbf0a5224b054c64d06ef90037f9. Scope is workflow argument isolation and pure guards, not a Cargo repair, product permission or new resource. Root remains sole heavy executor.

Use separate RUNNER_TEMP/agentbaton-legacy14-target, legacy15-target and legacy16-target for the genuine historical builders in Rust and both original Mac lanes. Preserve original current target, source/hash/default-bin/provenance builders and generators, exact tests/Clippy order, action pins, permissions/concurrency/matrix and25/60-minute limits. Add finite inverse normalization for the six argument edits and negative collision/current-target/other-change controls.

Decision: isolate each historical source revision instead of mutating/deleting shared caches or changing compiler flags, dependencies or runtime budgets. This is a reversible engineering choice within the existing CI quota. No new cache service/resource.
