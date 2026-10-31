# Observe the unknown cancellation state in a complete-history fixture

Base `1722a8aaac44e2bce888e921c21bd92a0af41154`; same isolated clock worktree. Only change the component test and this unit's documents. Keep product/API source, cancellation semantics, original case count, one-second observation guards and5000ms test timeout unchanged. Root runs Node/typecheck and actual tests.

The fixture first waits for complete `goal-graph-history`. That component always renders an acceptance-pending role=status. Unknown cancellation renders a second role=status in the parent. A global unique `getByRole('status')` therefore cannot observe the expected pending text once both exist. Earlier deferred-full-view cancel controls did not have the history status yet. Do not change the fixture's business behavior or make cancellation more permissive.

Assert cancel-button readiness, the exact one original cancellation request, both status roles and exactly one pending-text status. Preserve the original pending text and subsequent no-read/no-repeat-write assertions. Retain the last failed assertion as the fixed deadline error's cause so a future failure identifies the actual call/DOM disagreement without increasing the observation budget.
