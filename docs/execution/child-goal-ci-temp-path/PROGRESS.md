# Progress

Implementation commit `ee58cc8fae7af28a466bf9b49d22497cffc449ab`, branch `codex/child-goal-ci-temp-path`, tree `/workspace/.worktrees/child-goal-ci-temp-path`.

Read the actual ARM log and the baseline test/workflow source. The log identifies the assertion and both failing subtests but does not show the temporary directory values; an exact Darwin /var→/private/var mapping is not claimed as observed. Linux symlink-alias execution of the old mocked orchestration reproduces exactly two assertion failures before any real browser or product process. A frozen baseline-source replay retains the same result.

The fixture now resolves its expected root strictly and requires each private output to equal the direct canonical case path. Existing containment remains checked. A helper runs the existing success/failure scenarios under both ordinary and symlink-alias RUNNER_TEMP. All three driver/projection pairs must still be attempted, private root remains0700, all six private logs must be0600, and the public tree remains summary-only without the secret sentinel. An additional negative control removes only workflow canonicalization in memory and requires both failure scenarios to reject output at the exact private-root assertion.

Actual targeted command: `python3 -B -m unittest discover -s tests/infrastructure -p test_child_goal_ci_evidence.py -v` exited0, 13 tests passed in0.102s on Python3.12.14/Linux. Ten preexisting privacy/type/FIFO/publish test methods are AST-identical. No test was skipped, removed or weakened; production source was not changed. Git diff checks exited0. This is author verification, not independent review or Mac acceptance.
