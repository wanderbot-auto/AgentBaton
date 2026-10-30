# Evidence

Commands actually executed in the isolated worktree:

- `python3 -B -m unittest discover -s tests/infrastructure -p test_factory_rework_ci_evidence.py -v`: baseline66PASS/0.216s, then new72PASS/0.323s.
- `python3 -B tests/infrastructure/test_factory_rework_ci_evidence.py -v`: final exact source72PASS/0.284s/exit0 (earlier direct72PASS/0.277s retained).
- Python AST/source comparisons: all old functions except event_fields and all old test classes byte-equal; new event_fields statements removed yields original AST; exit0.
- `git diff --check`: final exit0. Earlier extra EOF blank-line warning was corrected without changing executable test content.

Evidence directory: `/workspace/.onboarding/factory-capture-timing-projection`. These are generated unit SQL/file fixtures, not retained product databases, real Core events or Provider receipts. Secret-canary values and paths must never occur in rendered public projections. All72 methods include the original66 rather than replacing or summing them.

Frozen local evidence metadata:

```json
{
  "SOURCE_CHECKS.json": {
    "bytes": 649,
    "sha256": "3366fc22941ab19aafa5af1ba6e85151361305e346e671f9af7521be3bb2781e"
  },
  "baseline.log": {
    "bytes": 11767,
    "sha256": "8d74f115ce8493f320b39323b776c3e44eaf0a5eebfa2650ed593d0b54c53925"
  },
  "final-pure.log": {
    "bytes": 12885,
    "sha256": "e52e3682f05bf9cb684cd93b28f8679da8271874891460340f9bf1c95327b403"
  },
  "exact-final-pure.log": {
    "bytes": 11229,
    "sha256": "016ac8afecb2d0074b297b618fed37fa9169e118e6aa6b84f599daba6326c2c4"
  },
  "direct-final-pure.log": {
    "bytes": 11229,
    "sha256": "f711486096d46ecd3db468c24d6fe968696cf9d484f31d3f99a5b84760d2c6af"
  }
}
```

No production code, locks, DDL, workflow, timeout, source runtime budget, task acceptance, permission or required check changed. No live daemon/browser, Cargo/Node, real model, CI or macOS validation was run. This is post-owned-shutdown diagnostic projection readiness only; it cannot turn a failed test or unknown stop into success.
