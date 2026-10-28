# Evidence

Baseline source: 87b4434a032fc2995facdee4761d877fa79b7770.
Actual source artifact: /workspace/.worktrees/goal-validation-graph/artifacts/goal-validation-graph/actual-integrity-graph-7cc70a97cac37d97967a292e519e5d6a5eaecf2267eec9a449d8f0a43e39a0f6.json.
SHA-256 7cc70a97cac37d97967a292e519e5d6a5eaecf2267eec9a449d8f0a43e39a0f6; 71,707 bytes. No fields were edited.

Actual local commands:
- vitest baseline-consistency.test.ts using isolated baseline config: 1 passed.
- vitest run src/goal-graph-integrity-observation.test.ts using private config: exit 1, 1 passed / 3 failed, before synchronization. Missing/null/unsafe observation incorrectly remained supported.
- Initial invocation used the wrong relative path and found no tests; that tool error log is preserved separately and is not a product test failure.

Unrun: new Native callbacks (no Cargo lease), macOS WebView/OS dialog, Data/Command full graphs, real Provider and overall product acceptance.
