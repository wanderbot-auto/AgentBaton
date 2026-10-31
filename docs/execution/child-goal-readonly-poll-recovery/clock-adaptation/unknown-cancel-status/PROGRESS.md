# Test selector correction authored; Root execution pending

The unknown-cancel case now confirms its complete-history fixture initially has one status and an enabled cancel button, then observes exactly one cancel call with the original scope/task/Goal/revision/key shape. It requires two status nodes after the unknown reply, exactly one whose text equals the pending message, the original pending-text assertion and the unknown-state alert. All later automatic-read/no-repeat-write assertions remain unchanged.

The old fixed timeout error did not retain the last assertion, so Root's 172 failure does not independently establish whether its cancel call was issued or status count was two. These new finite assertions and the preserved assertion cause establish that on the next execution. The source-established ambiguous-selector path is not evidence of a product cancellation defect or old remote-CI root cause.

Source guards passed: original case names/order, product/API/API-test bytes, both deadline guards and1000ms bound are unchanged. `git diff --check` passed. This test-only correction and new cause handling are NOT_RUN here; Root should execute this one case before all93 focused tests.
