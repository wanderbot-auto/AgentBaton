# Evidence

Source validation parsed current/baseline workflow with PyYAML; all jobs except the existing frontend run body compare equal. Its original inline Python AST nodes are an identical prefix. The only appended nodes initialize child_goal_failures, run normal/lost-start/cancel with check=True and aggregate their nonzero exits. Bash `-n` and Python AST parse pass; job names, matrix, security pins and20-minute timeout remain unchanged.

Two pure orchestration controls execute only the extracted new AST against a fake subprocess object: all-success returns without raising; all-three-fail still invokes each case and raises all three case/exit-code pairs. They assert `--run`, the same manifest/Chromium/frontend/tools and unique output paths. No Core, browser, real provider process or Cargo executed.

Actual destinations:

- artifacts/ui-browser/factory-child-goal-normal
- artifacts/ui-browser/factory-child-goal-lost-start
- artifacts/ui-browser/factory-child-goal-cancel

The real harness owns each report/preflight failure record. Existing always UI artifact retention keeps those reports; this wrapper neither invents pass files nor masks original Factory/planning failures. The unchanged original schema15 planning_candidate includes source_head/compiled_core_source_head/binary SHA/frontend_assets; historical14 snapshots are untouched.

Actual normal/lost-start/cancel browser outcomes, reports, candidate-linked CI and timing are NOT_RUN. Three added fixtures consume the existing20-minute budget; elapsed fit is unknown until real CI. Earlier existing failure or global timeout can prevent case execution and must remain failed/not-run.
