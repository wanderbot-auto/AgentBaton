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

## HR-R2 final source checkpoint

Do not use `81ae125` as validated evidence: its 8-attempt/40000ms limits failed the full three-round reservation precondition. The final test SHA-256 is `7d8952cd888516e0801d528c95607bc76a1e17b94248510b6979ab99fc9bc818`.

Only the new mocked test and these four records changed again. Root-approved fixture limits 12/20000, stage reservations 2000ms and consumed 8/3625/120 are separate; all grant/selection/scope expiry fields use the fixed historical 30s domain. No application authorization was modified. Root should obtain the narrow review closure and run the four new cases plus original TaskIntegration cases on the final immutable revision. No author Node/Cargo/browser/IPC/network execution occurred.

## HR-R3 execution checkpoint

Root actual source `f12c08ff2c8b53a89c568b5067be9b1925c29813` contains exactly b1's test bytes. Typecheck passed (Root-reported 1.2067s). Actual Node 53 cases: 51 passed/2 failed; original 49 and unknown2 passed; normal2 failed on duplicate tree/definition substring matches. Evidence remains `/workspace/.onboarding/integration17-history-rounds-node-tests1`.

Final query-fix test SHA-256: `efee53a8dac53e955595da5f6aabc7d5e0a7bf86238047b7b6488624b1e8b139`. Only the candidate assertion changes, keeping the colliding hashes and all other assertions. Revised source NOT RUN. Root rerun after narrow independent closure:

```sh
npm --prefix apps/desktop test -- src/integration-api.test.ts src/TaskIntegration.test.tsx src/IntegrationEvidence.rounds.test.tsx --maxWorkers=1
```

No author Node, Core, Cargo, browser, IPC or network execution. No new database assertions or runtime evidence are claimed.
