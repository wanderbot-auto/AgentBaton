# Source-only evidence

Base: `f1938820728bcae3ecf011d3ab710dab1e77e48c`.
Source: `eb8c67bf4ab686ab51b39e224f06665267b146b4`.

| File | SHA-256 |
| --- | --- |
| IntegrationEvidence.tsx | `5ab422d83a45f85a9af3a9f159c0355ce85b3e9feae1f28876c3e8cac2bb349f` |
| IntegrationEvidence.rounds.test.tsx | `ca58cf455125109e308b58b4aab5179c8e997386ea27d9a35b71d27ceabdabb7` |
| TaskIntegration.test.tsx (unchanged) | `c860df5c006ca1e2e0ed2b1dd47dc502d3a3913dbc164c2d6a16a106efa3b577` |
| integration-types.ts (unchanged) | `edb981ef68b61920315d84fa7bd5655b1536b413b76572311d856f82f9bb5005` |
| integration-i18n.ts (unchanged) | `0a6fdc21d2b113274492619b5ba5e6ae4c5ef38c4fd75752cc4ab0b7e0fc5e87` |
| integration-form.ts (unchanged) | `1c8002112f4a196603bd012de23f4323d22258d533976acdec02b4b3c556330d` |

Actually executed: Git status/base/remote reads, isolated feature worktree creation, fixed-source/DTO inspection, Python byte comparison and exact component inverse checks, SHA-256 calculation, `git diff --check`; all completed with exit 0. These are static source checks, not TypeScript or component execution.

The exact-inverse check replaces the single receipt group opening and the single attempt group opening (including only their added direct `round` term/definition) with their original opening tags, then compares the entire component against `git show f193882:apps/desktop/src/IntegrationEvidence.tsx`. Equality preserves all original receipt/status/candidate/actual-null/zero logic and other exported components, rather than treating a new assertion as proof of unchanged behavior.

Four authored component cases: two languages each for retained 1/2-failed plus 3-passed candidates, and unsettled/null measurements alongside actual zero. NOT RUN. No test failure is hidden or reclassified; there is no author runtime result for these cases. Original existing tests were not edited. Mock records do not prove actual backend retention, Git CAS, native approval, private-owner IPC, real providers, browser pixels or macOS desktop behavior.
