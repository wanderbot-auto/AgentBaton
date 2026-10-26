# Project registry execution unit

Full goal: codex/GOAL.md (130 requirements, WP01–14), AB-START-001. OWNER-001 ACK; latest merged inbox has no later sequence at unit start. Base03878121b649bcc90d0f35ace740f5bfbc10fe82, pending prerequisite PR#3; branch codex/project-registry. Goal unavailable, native workers reused.

Scope: connect existing read-only import, canonical identity, exact-head isolated worktree and safe file primitives to persistent projects/workspaces, revisions/idempotency/archive, managed-root selection and leases in the daemon; expose narrow desktop and CLI workflows. Source directories are never deleted by archive. SQLite+Git side effects require durable intent/reconciliation rather than false cross-system atomicity.

Ownership: core worker owns schema3/migrations, IPC, root Cargo/locks and CLI; workspace worker owns registry.rs and registry integration tests; desktop worker owns apps/desktop; primary owns integration/CI/status/evidence; independent reviewer verifies final diffs and tests. Shared contracts fixed before UI implementation.

Verification: import must not execute project content; dirty source/untracked files preserved; restart/reassociate identity; archive versus source preservation; stale revisions/expected HEAD; conflicting/concurrent leases/fencing; interrupted worktree creation outcome; real IPC and UI tests with fixture dimensions identified. Mac tests/builds are actual CI, no Provider model calls authorized.

Continue remaining full-goal dependencies after integration, not package/page-count completion. No expansion of final license, paid resources, identities, production or signing authority.
