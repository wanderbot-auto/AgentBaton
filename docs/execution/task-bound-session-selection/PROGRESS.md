# Progress

2026-10-29T22:04:10.022720+00:00, frozen source `f18d5ff0c2c2c8d48f64dfe76114f9636e64b0ad`.

ManagedSessions now reads the selected Task through fixed taskGet, uses existing strict Task/binding parser, checks current spec/project revision/local host and ready binding, then locates its exact ready workspace. Bound mode cannot select a different workspace. Existing candidate paths/metadata are never promoted into filesystem freshness or permissions. The original Core fresh admission and Native Owner Grant route remain unchanged.

Confirmed App source at this base: App.tsx109–114 returns from the1500ms polling effect while dialog is open. TaskCodeWorkspace uses its own taskGet and no App callback. A freshly prepared binding can therefore be newer than the tasks prop when execution dialog opens. This unit does not guess an event subscription: it explicitly taskGet-checks selection/refresh with response epoch and task/project context; parent props may be older while the fixed read returns the newer persisted binding. No App edit or periodic background task added.

Old project responses are rejected by request sequence/current project context. Old task replies are rejected by selected context; task/binding revision regression and lost/changed immutable receipt refs fail closed. Task/binding/source-context changes clear acknowledgement/selected Grant; synchronous contexts also prevent use before React effects run. A delayed native Grant reply is not installed into a changed task scope. Binding/read failures do not inherit another Task workspace.

Unbound Task manual workspace remains explicit and survives unrelated same-Task readonly revisions/refresh. With no selected Task, existing global manual Grant/Native Owner flow remains available. No automatic Grant, Session, prompt, retry, worktree create or execution permission. Existing mutate/send/session merge/event projection functions stay byte unchanged.

Root explicitly allowed the old SessionWorkbench mock update required by the new production fixed read: add api.taskGet mock and initializer returning the requested original fixture with spec_revision1, rejecting foreign IDs. Full inverse restores the original test file; no original assertions, limits or outcomes changed. No runtime mock fallback exists in production.

26 new mocked-transport component cases written but NOT_RUN: normal location; parent staleNone; exact explicit Session call/Core refusal; pending/unknown/input_changed; missing/creating/unknown/foreign/wrong-root/head/duplicate workspaces; spec/project/host drift; late Task/project reads; delayed Grant; binding invalidation; failed/regressing reads; legacy/manual/global Grant; Chinese boundary.

Actual verification only Python stdlib source/inverse/protected-file assertions and git diffcheck0. Not TypeScript syntax/compiler/runtime proof. No Node/Cargo/browser/Actor/Provider/remote execution. Root-reported earlier four TaskCodeWorkspace flows are a different unit, not green evidence for this new selection.


## Independent P2 follow-up 0db56038faae62d2b458cd6bc8909d0980d6275c

Original f18 source had a synchronous parent-prop transition gap: same-project initialTaskId A→B could render stateTaskA before its passive synchronization effect; old freshness/ack/Grant/action handler context did not include the new initial props. Independent source/finite model finding is preserved, not claimed as an executed React reproduction. Root separately reported original f18 typecheck and targeted38 (26new+12legacy) PASS; these remain original-source results, not proof that this timing gap was absent.

Narrow fix uses local parent-initial identity plus render epoch (including A→B→A), matched against an applied-state stamp. The stamp enters Task-source and selection context, synchronously invalidating old TaskFresh/ack/Grant/handler context. Until effect applies TaskB and its stamp, refreshTask returns before requesting oldA; B must then complete its own fresh fixed Get. This is local UI provenance, never wire/approval authority. Subsequent manual Task selection is allowed; it does not have to equal initialTaskId. Existing active Session inspection/send/cancel code is byte unchanged and not gated by this new setup epoch.

Added2source-only specs: the actual equality helper's sync/ABA predicate and RTL parentB plus deferred B Get, rejection of a new A request during transition, and later manual A selection. Whole old26test inverse is byte-equal; legacy12 test file unchanged. New declared28, planned targeted40; all new-source type/Node/test/build/actual IPC/browser remain NOT_RUN. Python source/inverse and diff0 only. Root will independently test.
