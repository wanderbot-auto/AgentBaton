# Isolate the polling clock from RTL's asynchronous drain

Base `1270c8c6875ad4ceecd373a77cf33260f2ac2e18`; isolated sparse branch `codex/child-goal-poll-test-clock` in `/tmp/child-goal-poll-test-clock`. Root owns Node/typecheck/browser/Core/SQL execution. Do not change the 127 product/API source, old cases, product 20x1000ms budget, test 5000ms timeout or failed-run evidence.

The installed RTL `react/dist/pure.js` asyncWrapper awaits `setTimeout(resolve,0)` after its callback. It advances that drain only when global Jest is detected. The new Vitest-only partial fake setTimeout does not supply global Jest; user-event's internal advance callback runs before the later RTL drain is scheduled. This gives a concrete fixture deadlock path, consistent with all new cases timing out. It does not identify an old CI product failure cause.

Replace global fake timers only in the new temporal controls with an explicitly advanced local 1000ms timeout scheduler. Keep user-event/RTL zero-delay drains and native SHA work on real timers. Use bounded real observation points instead of RTL default 1000ms wait timers in that scoped block, so observing UI never advances a polling slot. Root should run typecheck and one positive case before repeating the full focused suite.
