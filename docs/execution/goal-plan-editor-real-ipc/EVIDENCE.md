# Goal editor actual browser and daemon evidence

## Exact inputs and dimensional boundary

- Base product source `76ec9b1ea4cff3ea09549ee2aa2a1eb414edb61c`, containing reviewed metadata `5f4b0de` and editor `1751558feeaeb51bf3604be5dd6c2ed7acb21887`. No product change in this unit.
- Harness source `1707ada6a0a11379214af4300c0a23433bc2e807`, SHA `b2bc091e28e88669ef6c85a2e1954bd1c6d25389d73aae53306c2af47ba99018`.
- Real daemon `/workspace/.onboarding/goal-editor-composition-binaries/batond`, SHA `81e6ec0327f8dda1ff1b721eb381dfdf273e253020b6feed2922e9f13649493f`. Primary's same-directory manifest records schema9/Linux debug source `76ec9b1`; this is not release/Mac evidence.
- Actual built React: `index-ojfhYZcd.js` SHA `232b7024a08f05ba75c01ad77232c464e1187bfef647cd0e2eeb80c6c1db32bc`; CSS `index-B3WfCqru.css` SHA `3dc2b0859eaa5552936928bc10dbf03b008f52e582a8195a3a1b6466005cf98f`; index SHA `e072964df52904b36aac43d1a25978e66812d24fb148ddb8849d99a03128e954`.
- Dimension: built React + actual Chromium + unchanged fixed loopback test adapter + authenticated actual daemon Unix IPC + real private filesystem/database. Actual separate `kind=goal` proposal/finalizer is fixture owner setup outside HTTP, **not** an actual native OwnerDialog. Generated local executable only receives `--version`; no account, session, model, external host or SQL state insertion.

## Commands and results

Own worktree frontend build, `npm run build`: **PASS** (includes TypeScript). Existing locked dependencies borrowed via exact worker-created symlink, removed after build. Build log `/workspace/.onboarding/goal-plan-editor-real-ipc/build.log`, SHA `1632081b76e7a0c195858295ef4aa6662ed9f28643071b1828df5387464505d3`. No install/Cargo/full-unit-suite rerun is claimed for this test-only change; parent previously validated unchanged 259 UI tests.

```sh
python3 apps/desktop/tests/goal-planning-real-ipc.py \
  --batond /workspace/.onboarding/goal-editor-composition-binaries/batond \
  --output /workspace/.onboarding/goal-plan-editor-real-ipc/default-frozen \
  --chromium /usr/bin/chromium
python3 apps/desktop/tests/goal-planning-real-ipc.py --structured-editor \
  --batond /workspace/.onboarding/goal-editor-composition-binaries/batond \
  --output /workspace/.onboarding/goal-plan-editor-real-ipc/structured-frozen \
  --chromium /usr/bin/chromium
```

Both exit **0**. Default **9 checks PASS**, 6 screenshots. Structured **10 checks PASS**, 9 screenshots. Python syntax and `git diff --check` PASS. Exact adapter section compared byte-for-byte with base, SHA `473f3e44be40fdecd2338238c53c70d6249f6e03567ea455460fd856fe9578e4`; all original 9 check names retained. No added HTTP method or approval/finalization/generic/execution route. Node IDs for downstream duplicate/preview checks come from actual proposal (default retains its original `write-report` behavior).

| Final run artifact | SHA-256 |
| --- | --- |
| `default-frozen/report.json` | `0f4f335cadb69f225d351d2fc401797e4cda9408e9a8b657d976b6bbf88e7e24` |
| `default-frozen/run.log` | `bd2e5e0bc9c871327f73b74bc08145c47ad6dd579038529a72bf00ff487f7590` |
| `structured-frozen/report.json` | `0246a17faefccb9246701ff6d8a695116caaeb58fb846cee92f114ae0e298ddd` |
| `structured-frozen/run.log` | `1a11c042f38ef2bd5a01e791aa42154b1f7033d60590733f943c942c55e1c57c` |

All relative artifact paths above resolve under `/workspace/.onboarding/goal-plan-editor-real-ipc/`. Daemon logs and every PNG persist in the corresponding directory. No failed run occurred in this unit. The earlier passing `default-1`/`structured-1` outputs and their exact script hashes remain; final reruns add focused screenshot/metadata observation only.

## Actual structured behavior

The fixture owner approves two actual output requirements (`report` and `details`) and shared budget once, outside HTTP. Real `edit_context` exactly equals the original scope selection for paths/max_nodes/artifacts; rule-kind keys exactly match approved rules. Complete Goal snapshot is 9975 UTF-8 bytes (default one-output snapshot is 9608), below 16KiB; nothing truncated.

Actual browser opens collapsed editor with native summary **Enter** (covering the jsdom-only limitation). UI prepares the plan from native-approved scope, moves the separately approved `details` output to a new Work step, edits goals and runtime budgets to 2000/3000ms and sets the second step's dependency on the first. IDs are read from actual DOM/model, not substituted server records.

Before Apply the advanced raw textarea remains exactly its original bytes. Read-only SQL observations of journal sequence, budgets, goal ledgers, plan heads/proposals/revisions, attempts, sessions, leases, command records/fences and node/external-action records are unchanged during local editing. Apply changes only the existing raw draft. The same complete snapshots still match after Apply. The original separate **Store initial plan proposal** button invokes actual `goal_plans.propose`; stored raw `plan_json` bytes equal the UI draft and stored typed model exactly equals the edited proposal.

All original nine checks then run: original raw persistence, readonly previews, exact-key historical replay, same-key whitespace conflict, stale CAS, raw nested duplicate rejection by Core, exact revision 2/history 1+2, fixed-adapter denial, actual input drift clearing preview/disabling writes, and revoked history with denied new plan/preview. Preview's only admission is the actual first step; the dependent second remains unadmitted. Repeating preview leaves journal/budget/attempt/lease snapshots identical.

Final observed actual counts: **zero** attempts, sessions, dispatch intents, workspace leases, command runs/fences, Goal node evidence and external-action records; **1** plan head, **2** immutable plan revisions/proposals. No report/details output files are created. Task remains `draft`. Shared consumed/active are real zero in this genuinely new project/root fixture; this does not manufacture zero for historical meters.

## Actual screenshots and remaining limits

Viewed actual `structured-frozen/editor-02-applied-local-draft.png` (1488×981) and `editor-03-mobile-dependency-graph.png` (390×844): cold white/gray, hairline 6px cards, two diagram columns desktop/one mobile, explicit proposal-only status, no horizontal overflow. These are product React/Chromium screenshots, not prototype/native/macOS screenshots. Mobile editor and preview overflow assertions pass.

The report contains every PNG hash. Key editor screenshots:

| Path under `structured-frozen/` | SHA-256 |
| --- | --- |
| `editor-01-local-steps-no-write.png` | `82a8d8a33cf4ec24f67202c3b82de203b33f65e845f5b4a56bcf9500d35eae39` |
| `editor-02-applied-local-draft.png` | `a95ea5800356b0582b6e93469a897003262a2a69d92b73e09f3fbfab6053f62b` |
| `editor-03-mobile-dependency-graph.png` | `6155700b78ab2c98379389ef2c2f5bef7e3c99bd10e6fe2feec715ae4f1cc195` |

**Fresh database only.** Primary separately reported a legacy cached-approval/metadata compatibility investigation for old `2cad` records. This candidate's successful fresh Some-metadata runs do not prove that migration/replay boundary; no Core errors are ignored in the adapter. Final compatibility-fixed binary needs new exact outputs and affected regressions. Independent harness review, native owner dialog/WebView, macOS CI, NL planning, routing, reservation, actual Goal execution and Task acceptance remain separate pending dimensions.
