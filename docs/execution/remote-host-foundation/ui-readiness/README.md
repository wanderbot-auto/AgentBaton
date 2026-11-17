# PR30 UI readiness test follow-up

The leader added exactly `apps/desktop/src/GoalPlanDraft.integration.test.tsx` to B's shared claim; written scope and hash are in PLAN.json. A owns other overlapping desktop paths and its current fullRust test slot. B waits before Node load, does not copy other worktrees' node_modules or manipulate their processes.

Reported CI failure: PR30/e75 Product macOS job134152899605, unchanged original line33 expected generation enabled;1527 passed/1failed, later native steps skipped. B is fixing only test preparation for the already-fulfilled mock/effect chain. GoalPlanning's chosen scopeId triggers a second refresh that sets fresh=false; test readiness must include that refresh. Expiry remains existing Date.now()+600000. No production, fixture, limit, timeout or approval behavior changes.

Original second render is wrapped in existing async act, preserving every original expectation including line33. Added deferred second scopesList refresh with exact2 scopes/1 plans call count while pending; generation stays disabled and both draft generation/propose/approve/preview stay uncalled. Valid release alone permits enable; reject or foreign scope stays blocked. The review confirms this tests the unfinished second chain and does not hide a permission bug.

Local React/Vitest tests use mocked transport and do not prove native/Core/remote operation. Full UI and typecheck evidence is separate from the unchanged remote foundation commit9457/source hashdad334.... Once this UI test commits, later delivery heads must not be described as docs-only descendants of9457.
