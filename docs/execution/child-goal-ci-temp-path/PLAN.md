# CGCI private temp-path fixture repair

Baseline `79d73e3e56e07a3c96fbfb0962415fda5ee8e05f` (PR #18). AB-START-001 / Design v0.2; OWNER-001/002 ACK, no003 in this baseline. This unit repairs only `tests/infrastructure/test_child_goal_ci_evidence.py` and these four records. Core/UI/workflow/projector, policies, locks, DDL, CI job identities, timeouts and permissions remain outside the edit scope.

The actual Environment run40929728562 passed Ubuntu but failed both Mac lanes. The retained ARM log's two failures occur in the pure orchestration test's private-output containment assertion for fail=False/True. The production inline workflow resolves RUNNER_TEMP strictly, while the test compares that output with an unresolved TemporaryDirectory path.

Reproduce the old assertion on Linux with a real temporary symlink alias and the existing mocked workflow; canonicalize the fixture's expected root; retain exact private case containment, permissions and all-case failure collection. Add a positive alias case and a negative test which removes only the inline resolve call in memory and must reject its unresolved output. No actual browser/Core/Provider/DB invocation is necessary or authorized for this unit.

Independent review and exact new-candidate remote Mac checks are required after this source/pure-test delivery. Old Mac failures are preserved and cannot be relabeled as passes.
