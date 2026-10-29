# Checkpoint

Source `f18d5ff0c2c2c8d48f64dfe76114f9636e64b0ad`, base `3008c8f55eba5921aec9d0f33acaca02076ae693`, branch `codex/task-bound-session-selection`, tree `/workspace/.worktrees/task-bound-session-selection`. Four source files only: ManagedSessions.tsx, session-i18n.ts, ManagedSessionsTaskBinding.test.tsx, original SessionWorkbench mock/reset.

26 new cases source-only; typecheck, focused/full tests, build and actual IPC/browser/Native all NOT_RUN. Next: independent source review, Root exclusive Node window (`npm run typecheck`, focused test with `--maxWorkers=2`, unchanged full suite, build), then genuine current Task binding / lost or stale selection / Native Owner / actual session admission evidence. No mocks as FS or Provider validation.

Evidence `/workspace/.onboarding/task-bound-session-selection`; source hash/inverse/protected map and exact App polling facts retained. Do not infer that recorded ready binding authorizes execution, restart a failed operation under a new key, or skip the current Core/Native checks. Full Design v0.2/130 requirements remain in scope and incomplete; no formal approval or package claim.


## Independent P2 follow-up 0db56038faae62d2b458cd6bc8909d0980d6275c

Original f18 source had a synchronous parent-prop transition gap: same-project initialTaskId A→B could render stateTaskA before its passive synchronization effect; old freshness/ack/Grant/action handler context did not include the new initial props. Independent source/finite model finding is preserved, not claimed as an executed React reproduction. Root separately reported original f18 typecheck and targeted38 (26new+12legacy) PASS; these remain original-source results, not proof that this timing gap was absent.

Narrow fix uses local parent-initial identity plus render epoch (including A→B→A), matched against an applied-state stamp. The stamp enters Task-source and selection context, synchronously invalidating old TaskFresh/ack/Grant/handler context. Until effect applies TaskB and its stamp, refreshTask returns before requesting oldA; B must then complete its own fresh fixed Get. This is local UI provenance, never wire/approval authority. Subsequent manual Task selection is allowed; it does not have to equal initialTaskId. Existing active Session inspection/send/cancel code is byte unchanged and not gated by this new setup epoch.

Added2source-only specs: the actual equality helper's sync/ABA predicate and RTL parentB plus deferred B Get, rejection of a new A request during transition, and later manual A selection. Whole old26test inverse is byte-equal; legacy12 test file unchanged. New declared28, planned targeted40; all new-source type/Node/test/build/actual IPC/browser remain NOT_RUN. Python source/inverse and diff0 only. Root will independently test.
