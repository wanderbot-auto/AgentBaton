# Progress

Source commit: `eb8c67bf4ab686ab51b39e224f06665267b146b4`.

- Added a visible round term/definition and a named accessible group to each existing validation receipt and attempt.
- Kept receipt candidate commit/tree, original validator identity/digest, status, joined/stop flags and actual runtime/output expressions unchanged.
- Added four authored mocked component cases (two `it.each` cases over EN/CN). Earlier failed candidates coexist with the confirmed third candidate; record IDs contain no round number. Other candidates must not appear inside a receipt group. Tests also retain unsettled/unknown values, measured zero and unchanged input DTO bytes.
- Static exact-inverse check passed: removing only the two group attributes and direct round term/definition additions recreates the baseline component byte for byte.
- Baseline TaskIntegration tests, DTO types, locale and form were independently compared byte for byte and remain unchanged. `git diff --check` passed.

TypeScript typecheck, Vitest, frontend build, actual UI/IPC, browser and macOS: NOT RUN by this author. Source inspection does not establish runtime success. Root will run the applicable checks and arrange independent review.

## P2 revision (source-only; not runtime results)

Independent review found an invalid DTO model in the initial new test; the static checks above did not establish fixture validity. No initial component cases were executed, and no failing runtime result is reclassified as success. The initial commit and evidence remain in history.

- Completed preparation/validator/CAS attempts now use Core's `finished`; pass/fail remains solely in the validation receipts. The false/false/true assertions remain.
- An independent mocked policy selects max_rounds=3 and explicit 5000ms stage reservations (1025ms would exceed the old inherited 1000ms). Root/project limits allow 8 attempts, 40000ms and two reworks. This does not change any application policy or scope.
- Positive view has captured source commit/tree, final worker_joined, released fence and seven settled stages. Shared ledger includes those seven stages plus earlier Graph Work: 8 attempts, 0 active, 3625ms, 120 bytes; token/cost remain null.
- Validator-observed receipt sequences 11/21/31 precede their attempt-joined sequences 12/22/32. No unknown validation receipt is synthesized: unknown has the two old false receipts, an unsettled outcome_unknown third validator, interrupted_no_join_receipt, reserved budget, unknown fence, active=1 and unknown measurement dimensions.
- Source-only stage arithmetic/shape checks and baseline/component byte comparisons passed. All four modified component cases remain NOT RUN; Node/typecheck/actual Core/IPC/browser/macOS remain Root's separate execution responsibility.

## HR-R2 follow-up (supersedes 81ae125 budget model)

The first P2 repair closed the illegal attempt-state and missing-history issues but still had insufficient worst-case policy limits. `81ae125` was independently rejected, NOT RUN. No Root component test had used it.

Final mocked LIMITS: 12 attempts and 20000ms in each root/project budget; PREAUTHORIZED stage RESERVATIONS: 2000ms each, max_rounds=3; observed USAGE remains 8 attempts, 0 active, 3625ms/120B. Worst case 9 reserved attempts plus earlier Graph Work fits 12; 18000ms plus Graph's 200ms fits 20000ms. Worst output reservation 9216B plus Graph64B fits the unchanged 1048576B limit. Existing 1025ms validators fit each 2000ms reservation. These quantities remain distinct.

Fixture selection TTL, policy expiry, Goal scope expiry and grant duration/expiry now share the original historical created/approved time plus 30000ms. This is a historical mocked policy domain, not a fresh live grant. Remaining round, candidate, unknown/no-new-receipt, sequence, ledger and null/zero assertions are preserved. Product and original tests are unchanged. Static bound/arithmetic checks and diff checks pass; all four mocked component cases remain NOT RUN.

## HR-R3 actual test failure and narrow fix

Root's exact f12 integration typecheck passed in 1.2067s (Root-reported). Its Node command covered the original integration API/TaskIntegration tests and the four new cases: 53 total, 51 passed/2 failed. All original 49 and the new two unknown cases passed. Only the normal-history EN/CN cases failed at the ambiguous tree substring query; this is a real retained test failure, not evidence of an incorrect product candidate mapping.

The revision keeps the existing round-2 `d*40` tree and `d*64` definition digest, scopes to the candidate label's associated definition, and asserts exact complete commit+tree text. All other new assertions/fixture values and production/original tests are unchanged. Source-only exact-inverse and diff checks passed; the revised four cases have NOT RUN. The prior 51/2 result is not a pass for this new source.
