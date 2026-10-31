# Preserve both actual failed runs and validate the remaining case

Worktree `/tmp/child-goal-poll-test-clock`, base `1722a8aaac44e2bce888e921c21bd92a0af41154`. Existing Root-owned `apps/desktop/node_modules` is untouched. The 127 source tree, product component, API and runtime configuration are unchanged.

Root's 172 typecheck and single positive case passed; positive log SHA256 `5b5fc3c120599a05504f7292aa385b6ffbc9fd71e91a7141a207a417ce14e0f3`. Its full focused run returned92 PASS/1 FAIL out of93 (Vitest7.00s, wrapper7.620s), with only the unknown-cancel case exceeding its1000ms observation. Full log SHA256 `c3135fafbc90dbf123be7513a0844770ec040c599a0b311f524ecdd237a9d9fe` is preserved. The earlier 12765 PASS/28 timeout run and SHA03f74... remain separately preserved.

Root should independently review, then run only `does not restart reads or repeat a write while cancellation outcome is unknown` with verbose output before the complete two-file focused run. This candidate has no new runtime result. No SQL/Core/browser/Provider/native/network/CI execution or process interruption occurred here.
