# Checkpoint

Branch/worktree: `codex/integration-history-rounds`, `/workspace/.worktrees/integration-history-rounds`.
Base: `f1938820728bcae3ecf011d3ab710dab1e77e48c`.
Source: `eb8c67bf4ab686ab51b39e224f06665267b146b4`.

Allowed source paths only:

- `apps/desktop/src/IntegrationEvidence.tsx`
- `apps/desktop/src/IntegrationEvidence.rounds.test.tsx` (new)

The other four changed paths are this work unit's PLAN, PROGRESS, CHECKPOINT and EVIDENCE. No Core, Native, schema, locale, lock, CI, existing test or driver changes. No network, Node, browser, Cargo, IPC or product process was run.

Root next steps, on an immutable integrated source with matching existing dependencies:

```sh
npm --prefix apps/desktop run typecheck
npm --prefix apps/desktop test -- src/IntegrationEvidence.rounds.test.tsx src/TaskIntegration.test.tsx
```

Run the relevant wider frontend checks according to integration policy, and obtain independent source review. The four new cases are mocked renderer records only. Backend preservation of failed rounds, actual Core IPC/UI history and macOS remain separately evidenced by Root; no old runtime success is transferred to this source.
