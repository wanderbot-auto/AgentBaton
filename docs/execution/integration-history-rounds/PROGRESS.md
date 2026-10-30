# Progress

Source commit: `eb8c67bf4ab686ab51b39e224f06665267b146b4`.

- Added a visible round term/definition and a named accessible group to each existing validation receipt and attempt.
- Kept receipt candidate commit/tree, original validator identity/digest, status, joined/stop flags and actual runtime/output expressions unchanged.
- Added four authored mocked component cases (two `it.each` cases over EN/CN). Earlier failed candidates coexist with the confirmed third candidate; record IDs contain no round number. Other candidates must not appear inside a receipt group. Tests also retain unsettled/unknown values, measured zero and unchanged input DTO bytes.
- Static exact-inverse check passed: removing only the two group attributes and direct round term/definition additions recreates the baseline component byte for byte.
- Baseline TaskIntegration tests, DTO types, locale and form were independently compared byte for byte and remain unchanged. `git diff --check` passed.

TypeScript typecheck, Vitest, frontend build, actual UI/IPC, browser and macOS: NOT RUN by this author. Source inspection does not establish runtime success. Root will run the applicable checks and arrange independent review.
