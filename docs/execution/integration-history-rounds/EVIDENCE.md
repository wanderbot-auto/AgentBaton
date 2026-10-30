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

## P2 correction evidence (supersedes initial test model only)

The table above records the initial sealed source. Initial new test SHA `ca58cf455125109e308b58b4aab5179c8e997386ea27d9a35b71d27ceabdabb7` is retained as the rejected fixture, not a successful execution. Current test SHA-256: `0729e720dcabecff6214a9e74d42c9f22768f7eb44357bd3e666a8c969c0a9c5`.

Fixed baseline source anchors checked statically:

- `integration_store.rs:77`: integration attempt state allowlist; finished, not passed/failed.
- `integration_budget.rs:130`: settled attempts require finished plus joined/not_scheduled and a separate attempt receipt; unsettled attempts do not have settled measurements or receipts.
- `integration_settlement.rs:251`: unknown path retains reservation, sets interrupted/no-join and unknown fence, then returns before creating a validation receipt.
- `integration_settlement.rs:410`: observed validator receipt has Some runtime/output; its validator-observed event precedes the attempt-joined receipt written after line 497.
- `goal_runtime.rs:2841`: root/project historical shared usage includes the original shared ledger; it is not an empty per-integration counter.

Actually executed pure Python source checks: seven explicit stage rows; kinds prep/validator repeated three times then CAS; total stage runtime plus declared earlier Graph Work = 3625ms and output = 120 bytes; all observed stages fit explicit 5000ms/1024B reservations; max_rounds 3; known observed receipt sequences precede distinct joined sequences; no fabricated unknown receipt; unknown attempt keeps no measurements/joined receipt. These are finite source consistency checks, not a TypeScript execution or native approval verifier.

Production component compared byte-for-byte with `eb8c67b`; existing TaskIntegration tests, locale, DTO types and form compared byte-for-byte with `f193882`. All unchanged. `git diff --check` passed. Four EN/CN mocked cases remain authored NOT RUN. Core target-drift/reprepare implementation, actual runtime, actual ledger settlement, Native approval and macOS are not claimed by this fixture repair.

## HR-R2 bounded-policy correction

`81ae125985dc5a68b667a5a916dba2f5d1daf73f` and its test SHA `0729e720dcabecff6214a9e74d42c9f22768f7eb44357bd3e666a8c969c0a9c5` retain the second P2 history, NOT RUN. The completed-attempt/receipt/unknown corrections in that revision remain; its budget limits are superseded. Final new test SHA-256: `7d8952cd888516e0801d528c95607bc76a1e17b94248510b6979ab99fc9bc818`.

Fixed Core `integration_approval.rs:90` through line 110 multiplies all stage reservations and `(validators.len()+2)` by max_rounds before checking both budgets. Pure source/arithmetic checks actually executed for the correction:

- `(1 validator + 2 stages) * 3 rounds = 9` reserved attempts, plus prior Graph1 <= root/project LIMITS12.
- `(2000 + 2000 + 2000) * 3 = 18000ms` reserved, plus prior Graph200 <= LIMITS20000ms.
- `(1024 + 1024 + 1024) * 3 = 9216B`, plus prior Graph64 <= unchanged LIMIT1048576B.
- Seven explicit settled integration stages plus Graph1 still produce USAGE8, active0, 3625ms and 120B; no usage is increased to the new limits, and unknown token/cost remain null.
- Every known 1025ms validator is below its selected 2000ms reservation; all positive stages remain finished and the unknown branch remains unsettled/no-new-receipt.
- Selection TTL, original grant duration and policy/scope/grant expiry consistently use the historical creation/approval timestamp plus 30000ms. No wall-clock freshness or live authority is claimed.
- Production component remains byte-identical to `eb8c67b`; original TaskIntegration tests, locale and DTO types remain byte-identical to `f193882`. `git diff --check` passes.

These checks are static, not TypeScript/Vitest or Core approval execution. Four mocked renderer cases remain authored NOT RUN. Original sealed commits, the two review failures and their evidence remain available; no actual UI/IPC/macOS result is transferred to this revision.
