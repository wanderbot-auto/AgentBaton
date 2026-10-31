# Root's bounded validation handoff

Worktree `/tmp/child-goal-poll-test-clock`, branch `codex/child-goal-poll-test-clock`, base `1270c8c6875ad4ceecd373a77cf33260f2ac2e18`. Only `FactoryChildGoals.test.tsx` and this unit's documents change. The original `/tmp/child-goal-readonly-poll-recovery` tree, its installed dependency link, product source and failed run are untouched.

Preserved Root run `/workspace/.onboarding/pr18-poll-recovery-127-type-focused1/command.log`: typecheck passed; focused tests naturally exited 1 after wrapper146.821s / Vitest145.07s. Of 93 tests, 65 passed and 28 newly added component cases failed at the existing5000ms timeout; old55 component cases and all10 API cases passed. Log SHA256 `03f74b8cc6bfbd368b24ce1579d87dbf2528019322731d1e1ad0b4658c40bdc6`. No kill, skipped test or extended timeout was used.

Root should review this test-only source, typecheck, then run only the positive `recovers a typed not-started readonly Busy` case with a verbose reporter before the full two-file focused run. Source analysis establishes the frozen RTL0ms drain path; actual success of this adaptation remains NOT_RUN. No real Core/browser/SQL/Provider/native/CI result is implied.

The fixed-hash follow-up to `edbacf4` adds the review-required observer deadline checks before attempting an assertion and before accepting success, preserving strict1000ms observation. Source-order validation passed; all test counts and product source remain unchanged. Await final source re-review and bounded Root execution.
