# Data-copy cancellation receipt evidence

## Exact source and confirmed contract

Base `aa1c2814a30f43405f3c545ec178cf665f96e673` (reviewed DataUI a436); source `52a06c9c44814217d0734f21be319af43765ccb5`, branch `codex/data-copy-cancel-receipts`. Only three existing frontend files and a new test change; original325 tests/18files, test fixtures, Core/native/App/locks/CI/harness/review files are unchanged. This branch does not integrate the separately reviewed personal Goal readiness fix.

Core owner `/root/core` confirmed the formal nullable/defaulted `creation.io_completion` projection in task communication and current producer code was read statically. The accompanying backend was not built or executed by this unit:

| Completion/state | Meaning accepted by UI |
| --- | --- |
| missing or null | Legacy/unobserved completion; never infer join |
| cancelled + worker_joined | cancel_requested true, contract null, Core error CANCEL_REQUESTED/confirmed; observation null or failed/cancel_requested |
| ready + worker_joined | Existing full Ready proof still required; cancel_requested may be true and Ready wins the race |
| legacy ready + missing/null | Retain valid historical Ready; no inferred join |
| failed + not_scheduled | Null observation/contract; no claim destination is empty |
| failed/outcome_unknown + worker_joined | Worker returned, not proof of successful copy or filesystem state |
| outcome_unknown + interrupted_no_join_receipt | No joined worker receipt; null contract |

`io_completion` is a readonly optional TypeScript property. Unknown/non-string values or contradictory active/Ready/cancelled combinations fail the defensive shape check, invalidate freshness and cannot invoke onReady or clear pending floors. The complete outgoing typed API object is byte-identical, so client selection/cancel/approval payloads do not include completion metadata. No private host witness, contract digest, paths or server state is reconstructed.

## UI and recovery behavior

Only a fresh record with matching ID/project version and revision floor can reconcile cancellation. A valid new cancelled record has a separate owned-copy-worker-stopped label, readable completion projection and clear warning that files may already have been written and partial output remains. It is neither a Ready workspace nor artifact acceptance. Its polling stops when no other operation is active; a new preview can occur only after a user click and any subsequent copy still needs separate native approval. The same old preview cannot be approved again.

Legacy failed/CANCEL_REQUESTED rows remain real failure data and show unconfirmed cancellation rather than fabricated stopped state. They do not settle an uncertain cancel, including when an explicit same-key replay returns an acknowledged legacy result. Ordinary failed handling and original cancelled-preview wording remain. Ready/cancel-requested races remain Ready with the original race warning, valid full proof and normal onReady; no cancellation success is claimed.

The new35 tests cover positive/null/historical DTO combinations, twelve invalid cancelled combinations, five other contradictory metadata cases, real component pending/floor/malformed-record behavior under mocked transport, explicit next preview/native-approval separation, no automatic mutations, polling cessation, legacy same-key retention, foreign/stale/project-version reads, Ready race and preview wording.

## Actual commands, failures retained, final results

Working directory `/workspace/.worktrees/data-copy-cancel-receipts/apps/desktop` for valid UI commands. Existing installed locked dependencies were borrowed through an exact worker-created node_modules symlink and unlinked afterward; no install/lock change.

```sh
npm test -- src/DataWorkspaces.cancelled.test.tsx   # original aa1 production, new draft controls
npm test -- src/DataWorkspaces.cancelled.test.tsx   # repaired production, correct desktop cwd
npm run typecheck
npm test
npm run typecheck
npm run build
```

- Actual old control: **24 failed /11 passed in35**, exit1, start2026-10-27 08:45:15 UTC, duration3.91s. Original product accepts contradictory IO/cancelled records and lacks the new cancellation UI/reconciliation. These are mocked UI/DTO failures, not a newly executed real IO regression.
- First repaired command accidentally used repository-root cwd and ran **four infrastructure Node tests**, exit0; log `fixed-targeted.log` is explicitly **not UI evidence**. It was corrected without changing assertions or script configuration.
- Correct desktop targeted run: **35/35 PASS**, start08:48:02 UTC, duration1.96s.
- Initial typecheck rejected deliberately invalid future_receipt cast in the new transport fixture. It now explicitly passes through unknown for that malformed-wire control; production union stays closed. Initial type failure remains in `typecheck.log`.
- Final complete source: **360/360 UI tests in19/19 files PASS**, start08:48:35 UTC, duration13.94s; **typecheck/build PASS**, `git diff --check` PASS. Original325 tests/18files byte-identical. All new acceptance cases run in the complete suite, no skip/deletion/deadline/config weakening.

After the old run, legacy same-key post-ack retention assertions were added within its existing test and the deliberate malformed-wire cast was clarified. Old reached failing assertions/case names were retained; no failed acceptance check was removed.

| Log under `/workspace/.onboarding/data-copy-cancel-receipts/` | SHA-256 |
| --- | --- |
| `full-ui.log` | `194b405bfbffecd86e3ba639ff97eb9b97aa3d6874798cb9d4f6bd90ce81a594` |
| `typecheck.log` | `5c6a3a54799d613b0f9c88f4c874f6fd784df1fff71739f9555082294cd4b8c0` |
| `fixed-targeted.log` | `8341c26319dca4155842b7bd05e61e7a6b57402950fc43505e2256cf2aa38ecc` |
| `corrected-targeted.log` | `dfe7a735ee4eef47ea079887ac5e30a2bc7f679869a13e556b32204d3062e7ff` |
| `final-typecheck.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
| `old-control.log` | `21d78bbfa9414c2280beb23c209bfb0089d50254581eec0100e61d2f239366e9` |
| `build.log` | `a427b793177771a4fa1e7ec9683ba7828a264a4d76f6fde6e85d97a5e438627d` |

## Source and built artifacts

| Owned source | SHA-256 |
| --- | --- |
| `apps/desktop/src/DataWorkspaces.tsx` | `89aaaedf8b1efa8d9a61e9f433fc780e4eaf1186437f82e1b2e141fae3af5664` |
| `apps/desktop/src/data-workspace-api.ts` | `5e081778fd8404c3db5011a688a2fa9bbf8fb9459d103356fcea4aa1a0da0f9f` |
| `apps/desktop/src/data-workspace-i18n.ts` | `a774914cc7b1fed5a0d1942410e9f1d21902602c6fae86d95a994ecd6c58feb8` |
| `apps/desktop/src/DataWorkspaces.cancelled.test.tsx` | `23426e31607c17959af62d873a0c14607ef10b2fdc26afac31e39ae3b156c7c3` |

| Frontend artifact | SHA-256 |
| --- | --- |
| `index.html` | `888240bdf3ce3890dc8fcad0ecd02378ff24c5e0b326a9cf69d688263b5e3d0c` |
| `assets/index-D6UB0Jo6.css` | `1208bc54eb8aeea54bca803a4d78dc7435688106f6b6eb0224dfbf1b5b390595` |
| `assets/index-sgW3oBcZ.js` | `2f801d59c1f8b33d24cc804b758b23958b2c0eab4bc4efd98d79e223d453ba08` |

Machine-readable `/workspace/.onboarding/data-copy-cancel-receipts/checks.json` SHA `bc543193a99e7eb223bb0f40f38adf8a5dfe1f7ac6b620516f6023da06ca7771`.

## Evidence limits and review status

This is author self-check only; primary independent review is pending. Mock records/closures do not prove a worker really joined, a filesystem copy stopped, partial output was retained, or a native owner approved it. No new Core/schema10 binary, actual IPC/browser, screenshot, OS OwnerDialog/WebView, macOS, Provider/model, Task acceptance, signature/install package or full-product completion is claimed. Root's old636e real harness success did not cover this new optional completion contract and remains separate historical evidence. Root/other authors own actual composed IO tests after final backend freeze.

No new scope/permission, API transport command, source/destination root, automatic mutation, new private witness or integration acceptance was introduced. Four source files and these four local records were the only edits; no source in original DataUI tree/review or any shared build target was changed. No push/merge/recursive agent.
