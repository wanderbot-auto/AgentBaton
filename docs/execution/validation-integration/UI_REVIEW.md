# Artifact validation frontend independent review

Baseline HEAD `914ed5fab9d718f9eae0759cdc99332dd9cb75d4`, reviewed uncommitted UI snapshot fingerprinted below. Scope: AB-VAL-001/002/006/007 and AB-UX-009/010. Read current gate AB-START-001 and OWNER-001/002, selected requirement acceptance criteria, actual UI transport/components/tests and the corresponding native/Core DTO and validation binding implementations. Core, schema6/migrations, native implementation and adoption require their separate reviews. This record does not certify those units.

## Finding F1 — closed after independent verification

F1 — A successful reconciliation remains reusable after a subsequent failed or mismatched refresh. `refresh()` sets pending `refreshed: true` on success, but an error leaves a previous true flag intact. Thus unknown approval -> successful read refresh -> failed/foreign-scope refresh can leave the explicit retry enabled despite the newest read having failed. The exact key/payload are still retained and there is no automatic mutation or authority expansion. Nevertheless, the integration contract requires failed/mismatched reads to keep retry blocked. Reported to primary; no reviewer source modification. The UI owner fixed it by resetting the saved and local pending `refreshed` flag at the beginning of every reconciliation, before the connected check. Only an actual successful matching policy+record read can set it true again. The two new success→failure / success→foreign-scope regressions prove retry becomes disabled, no extra mutation occurs, and a later successful read plus explicit retry reuses the exact original selection and key. Independently reviewed final component SHA-256 `b2b344cd45e8c2a078ca3aaaa25c928e0c3e965d8af1858dfb48773c7acb0a9d` and test SHA-256 `de3bdef94d94f226cb4b25a7668c49438cbbd3097863926a3c32930fd86209f6`. No unresolved blocking frontend finding remains in this reviewed scope.

Primary clarified that an actual successful read-only reconciliation on remount may enable a subsequent explicit same-request retry. It need not require an extra manual Refresh click. This does not permit automatic mutation, a replacement key or a new grant.

## Independently verified behavior

- Typed frontend wrapper sends relative requirements, session ID and `analysis: null`. It never roundtrips canonical roots, directory device/inode identity, resolved task/attempt/spec/input binding, approval booleans, finalize/accept/adopt/CAS operations. Directory IDs are read-only numeric display data and are not compared or submitted.
- `validation_policy_approve` is a Rust-hosted propose/native owner dialog/finalize sequence. Renderer permissions remain empty. No renderer finalize route was added. This was inspected, not exercised in a native GUI.
- Eligibility checks task and project and limits selection to stopped/failed sessions. Server contract independently requires the matching stopped intent, attempt, released lease/fencing token, nonarchived project and exact workspace/input/spec binding; a failed label is explicitly insufficient. UI mock denial regression preserves the selection.
- Approve and run are distinct user operations. Opening Artifacts reads actual per-task policy/record APIs; the updated QuietCanvas test asserts exact selected task IDs and no implicit approval/run. Removal of the obsolete unavailable-placeholder assertion is justified by real implementation.
- Integrity receipts expose hashes, sizes, file outcomes and exact attempt/spec/input/evidence digest with explicit historical-data warnings. No code correctness, task acceptance or integration authorization is inferred. Scope checks reject elevated or foreign records; unknown enum values render as unknown, not success.
- Unsafe absolute/traversal/backslash/control/oversize paths, malformed/missing fixed-input digest, duplicate paths, count and byte caps are rejected before native calls. UI checks are usability hints; filesystem/symlink enforcement belongs to the server. Returned malicious markup is text, not HTML/link/file execution.
- Unknown approval/run outcomes do not automatically retry. Same payload, original revision and idempotency key are reused only on explicit retry after reconciliation. Map storage survives navigation within the UI process, is bounded at32 unresolved tasks, and is not durable across application restart. No localStorage/sessionStorage pending queue or model/account call was introduced.
- Details are folded, controls labeled and warnings localized in English/Chinese. Raw paths, IDs, hashes and recorded evidence remain unchanged. Shared light theme tokens are reused; no homepage or native owner-flow replacement. No browser/native visual acceptance for this new panel is claimed.

## Independent commands and limits

Cwd `/workspace/.worktrees/validation-integration/apps/desktop`:

- `npm run typecheck`: exit0.
- Initial `npm test`: exit0, 73 tests /9 files (48 baseline +25 new parameterized UI/transport/parser tests), retained as historical pre-F1-correction evidence.
- Final frozen snapshot: independently reran `npm run typecheck`, `npm test` (78 tests /9 files: all73 retained +5 focused adversarial cases) and `npm run build`; all exit0. Source hashes were unchanged across this final run.
- `npm run build`: exit0.

Mocks prove frontend behavior/transport arguments only; no real Unix validation IPC/native owner click, macOS, symlink filesystem security, Core migration or full GOAL acceptance is inferred. No real vendor/model/account call, recursive agent, paid resource, source/lock edit, commit or push by reviewer.

Coverage improvement requested during review: the previous combined foreign/elevated fixture was split. Final tests independently reject same-task integration authorization and same-task task acceptance, reject foreign task history, and show unknown record/individual artifact enums as unknown without a passing badge or task authorization. All passed in the final78 run. This reviewer did not add or modify test source.

Logs: `/workspace/.onboarding/validation-ui-independent/`. The initial nine UI hashes were stable across the independent73-test run and retained in `source-before.json`. Only the two F1-owned files changed for the final78 run; final hashes below were independently checked against the frozen snapshot after the run. Unrelated primary native/Core/lock changes were preserved. The reviewer removed only the exact temporary symlink they created, to the existing locked main node_modules; actual dependency files were untouched.

## Final reviewed source snapshot SHA-256

| File | SHA-256 |
| --- | --- |
| `apps/desktop/src/App.tsx` | `f3883a1493c6a7b9ed463062ea6344b94b9ab0a441ce67af81d4f11ec089f11d` |
| `apps/desktop/src/QuietCanvas.test.tsx` | `d3f7b87d45c2bc11419f3a055d2942590dfdce9e0c8e1624a32c60b9030b0e3b` |
| `apps/desktop/src/api.ts` | `c3d84571403475c1d8dcba72ceefba3e3b5f13dc2e46de5880f8d35eee732de2` |
| `apps/desktop/src/style.css` | `bbe5c13607ba4a979ecf05b5c6a1417b3b92807e5715c391e5e4a5dd345789df` |
| `apps/desktop/src/ArtifactValidation.tsx` | `b2b344cd45e8c2a078ca3aaaa25c928e0c3e965d8af1858dfb48773c7acb0a9d` |
| `apps/desktop/src/ArtifactValidation.test.tsx` | `de3bdef94d94f226cb4b25a7668c49438cbbd3097863926a3c32930fd86209f6` |
| `apps/desktop/src/validation-api.ts` | `8f12a282955f13ac68c5d0db45588ed019fed1d1d377a3325287d865bada42c3` |
| `apps/desktop/src/validation-api.test.ts` | `b3913d3bfa2cc210de3218acc7d19f0055f9d7644ad76ac80be844ddb7dac916` |
| `apps/desktop/src/validation-i18n.ts` | `3ff5ea10d8dbd6faccf940c0bbbd3ecb74c480069e486ebafccdcf919b4a1c04` |

## Independent evidence SHA-256

| Log | SHA-256 |
| --- | --- |
| `build-final.log` | `53e04c42fbd67813f4b3a1258e5277856a80c0047b5eccb9a165ef89edacb744` |
| `build.log` | `964dc940f685af06b37a8921915f25c40482f58278b2cde137f51f50e277f32c` |
| `source-before.json` | `d4eccaeedcab2da8a16187119edc808cec417a7b42e666011fb54f8bfd9ce924` |
| `source-final.json` | `da6057415858013630e9bcdd5dd0a16a2d093cd0bc7e56257d53c6106f301343` |
| `tests-final.log` | `c211c21e601849f45ab4bf13c09ba028d623e9d0cdd3c3a49d73c60d35f35e63` |
| `tests.log` | `7232e91ee07b6fd657b923933689064f2ddf527466c1fe3e830d5d3495945bbb` |
| `typecheck-final.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
| `typecheck.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
