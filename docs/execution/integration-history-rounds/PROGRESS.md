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
