# Goal UI real IPC evidence

## Exact inputs

- UI worktree base: `4953abe4e9607551d0694016be35b1d52158fac8`; reviewed product source: `98a38d4cd486afa2a4fe476d0f464f1763d0805a` (unchanged).
- Frozen backend source: `1a3236a94b1b6a6346d1ee604bb23b1a595425bc`, docs `a70139d`, provided by primary. Old daemon: `/workspace/.onboarding/goal-composition-old-binaries/batond`; SHA-256 `66663629d716ec8dbd999aa45b375a9c6100a3392dc80ae9a7f92fa12144f303`.
- Harness SHA-256: `e54a3cbeb5c6facb969e5a34a4b35c04cd648c02007c828e93783372d538acf7`.
- Actual built React assets: `assets/index-BR4yu0OJ.js` SHA `24c4d859c877d16fd058454fe8e2e6d1235f9f12ce6dd980346bb1f7606b3a76`; `assets/index-CDZLaXF_.css` SHA `7f9270521b6dee457e9bf9cb85f3269ed5b8079d0cae9f34fe5cdf06e92508dd`; `index.html` SHA `502ad14607feb22d03e2e3803517805bdf7a76113ddf952c2cb5d8dd4c7675fb`. Every run's report records its exact assets.

## Commands and observed results

```sh
python3 -c 'from pathlib import Path; p=Path("apps/desktop/tests/goal-planning-real-ipc.py"); compile(p.read_text(), str(p), "exec"); print("syntax PASS")'
python3 apps/desktop/tests/goal-planning-real-ipc.py \
  --batond /workspace/.onboarding/goal-composition-old-binaries/batond \
  --output /workspace/.onboarding/goal-ui-real-ipc-old-1a3236a-frozen \
  --chromium /usr/bin/chromium
```

Syntax: **PASS**, no daemon/browser involved. Real old-daemon command: **FAIL, exit 1**, report status `failed_or_interrupted`, **1 prerequisite check passed**. Real generated local connection passed `--version` probing; no SQL connection insertion, ACP session/model or external account. Authenticated owner fixture approved actual separate `kind=goal` proposal outside HTTP. Root grant changed only revision/max_children/max_depth; task remained `draft`.

Actual built UI starts with Goal collapsed, expands and invokes fixed Goal list methods. `goal_plans.list` on the approved scope returns `NOT_FOUND`, message `Goal has no persisted plan`, `retryable=false`, `side_effect_state=not_started`. React displays the error. The assertion requires genuine `result=[]`; no error normalization, pre-seeded plan or changed requirement. Later raw-plan/preview/history assertions were **not run** because the initial contract fails.

Artifacts (absolute paths):

- `/workspace/.onboarding/goal-ui-real-ipc-old-1a3236a-frozen/report.json`, SHA `637106fcc3cd49fffc8a751c777dee0534a42ddb91b47f5b50078050df5eb46c`.
- Same directory `run.log`, SHA `e11b20e50fd83104be9b9468264abb6331bbe32820560fa7d8b74872ed7cc3e4`, plus `daemon.log` and actual UI `01-fresh-goal-expanded.png` / `failure-current-ui.png`. Screenshot hashes are in the report. Viewport 1488×981; full-page PNG; visually inspected: actual Goal error in Quiet Canvas Details, no fake running/success.
- `/workspace/.onboarding/goal-ui-real-ipc-old-1a3236a/`: preserved initial harness route-pumping failure, not a backend defect claim. `/workspace/.onboarding/goal-ui-real-ipc-old-1a3236a-control2/`: corrected scheduling and genuine same `NOT_FOUND` negative control. Each report retains the exact harness hash for that run.

## Boundaries and pending verification

The test adapter binds IPv4 loopback, exact Origin/Host/JSON/body limits, fixed fixture IDs, exact argument keys and safe integer CAS; inner `plan_json` is never parsed/normalized in the adapter. It exposes no approval/finalize, permission, generic IPC, session creation, dispatch, reservation, task acceptance, or file/shell capability. Daemon token remains in Python closure, never browser/report. All SQL statements are readonly observations. Temporary generated resources are removed; run evidence persists.

Scripted but **not yet executed**: original raw string persistence through actual UI; valid plan version CAS/history; repeated readonly preview with identical journal/budget/attempt/lease snapshots; same-key immutable result and whitespace conflict; nested duplicate JSON rejected by Core; HTTP authority rejection; input drift disabling live preview/new UI writes; revocation retaining immutable historical reads while blocking new actions; mobile preview screenshot and overflow assertion.

No new full UI/types/build/native tests were necessary for this script-only change; no such rerun is claimed. A repaired schema9 binary, full real-script run and independent harness review are pending. This evidence does not establish native OwnerDialog, macOS/WebView, real Provider, NL planner, Goal execution/reservation or Task acceptance.

## Final real history run

Rootcurrentharness f7d13bbb uses exact accessible roles for select/textarea without dropping value/raw/CAS assertions and scrolls actual previewheading for desktop/mobile capture. OriginaloldCoreNOT_FOUND and newharnesslabel/textarea locator failures retained separately. Primary and independent samefrozenBackend2cad/Linuxdebugdaemon dcbab69 eachactually PASS9checks/6captures. Primary report f349971f037f253a2fbdc307888e35791213fd7abd3cfdb2f2937c981b45c6f6; independentreport7d6f789f3c7fd744f5bc9f502eed6e52c9a5abf5256dac787fa99a67dd1f066e; independentREVIEW7331dfca04a424042cda8b3b7d620a6f4610a63a8f5c9e26e3271bd1d6bb3d1c. Exact6PNG hashes inreport; 06upperform notstandalone revokedvisualproof, actualUI/Core assertions provebehavior. Addedsameharness toexistingCIfrontendstep only; no newaction/deps/timeout/permission/Environment change. No nativeOwner/Mac/NLP/dispatch/TaskAccepted/vendor claim.
