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

## Superseding review checkpoint

The first new fixture in `eb8c67b` was rejected as P2, NOT RUN. Do not run or accept that initial fixture as validated evidence. This revision changes only `IntegrationEvidence.rounds.test.tsx` and the four own records; the production component still exactly matches `eb8c67b`, and all original tests/locales/types/form still match the baseline.

Current new test SHA-256: `0729e720dcabecff6214a9e74d42c9f22768f7eb44357bd3e666a8c969c0a9c5`.

Hand off the fixed commit to the same independent reviewer and Root. The previously listed Node commands apply to the fixed immutable source. No author Node, actual provider, Core, browser or IPC process has run. Positive history is an intended consumer DTO fixture, not observed target-advance or native authority evidence. No permission or production behavior changes are part of this revision.
