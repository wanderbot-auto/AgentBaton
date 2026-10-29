# Progress

2026-10-29T22:04:10.022720+00:00, frozen source `f18d5ff0c2c2c8d48f64dfe76114f9636e64b0ad`.

ManagedSessions now reads the selected Task through fixed taskGet, uses existing strict Task/binding parser, checks current spec/project revision/local host and ready binding, then locates its exact ready workspace. Bound mode cannot select a different workspace. Existing candidate paths/metadata are never promoted into filesystem freshness or permissions. The original Core fresh admission and Native Owner Grant route remain unchanged.

Confirmed App source at this base: App.tsx109–114 returns from the1500ms polling effect while dialog is open. TaskCodeWorkspace uses its own taskGet and no App callback. A freshly prepared binding can therefore be newer than the tasks prop when execution dialog opens. This unit does not guess an event subscription: it explicitly taskGet-checks selection/refresh with response epoch and task/project context; parent props may be older while the fixed read returns the newer persisted binding. No App edit or periodic background task added.

Old project responses are rejected by request sequence/current project context. Old task replies are rejected by selected context; task/binding revision regression and lost/changed immutable receipt refs fail closed. Task/binding/source-context changes clear acknowledgement/selected Grant; synchronous contexts also prevent use before React effects run. A delayed native Grant reply is not installed into a changed task scope. Binding/read failures do not inherit another Task workspace.

Unbound Task manual workspace remains explicit and survives unrelated same-Task readonly revisions/refresh. With no selected Task, existing global manual Grant/Native Owner flow remains available. No automatic Grant, Session, prompt, retry, worktree create or execution permission. Existing mutate/send/session merge/event projection functions stay byte unchanged.

Root explicitly allowed the old SessionWorkbench mock update required by the new production fixed read: add api.taskGet mock and initializer returning the requested original fixture with spec_revision1, rejecting foreign IDs. Full inverse restores the original test file; no original assertions, limits or outcomes changed. No runtime mock fallback exists in production.

26 new mocked-transport component cases written but NOT_RUN: normal location; parent staleNone; exact explicit Session call/Core refusal; pending/unknown/input_changed; missing/creating/unknown/foreign/wrong-root/head/duplicate workspaces; spec/project/host drift; late Task/project reads; delayed Grant; binding invalidation; failed/regressing reads; legacy/manual/global Grant; Chinese boundary.

Actual verification only Python stdlib source/inverse/protected-file assertions and git diffcheck0. Not TypeScript syntax/compiler/runtime proof. No Node/Cargo/browser/Actor/Provider/remote execution. Root-reported earlier four TaskCodeWorkspace flows are a different unit, not green evidence for this new selection.
