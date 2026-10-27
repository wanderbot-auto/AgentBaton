# Independent structured Goal editor real IPC review

Status: **PASS for the reviewed harness and actual fresh-store UI/Unix IPC flows on the compatibility-fixed frozen daemon.** Independent default 9/9 and structured 10/10 checks passed; no remaining blocking harness finding. Legacy cached-record compatibility remains a distinct Core regression/review dimension. OWNER-001/002 read and ACK; no authority inferred from fixture data or agent output.

## Exact scope

Harness source `1707ada6a0a11379214af4300c0a23433bc2e807`, documentation head `be2df57e24edc735f3a20fc1e8bcfb966f986c89`, base product `76ec9b1ea4cff3ea09549ee2aa2a1eb414edb61c` (reviewed metadata `5f4b0de` and editor `1751558`). Only the existing `apps/desktop/tests/goal-planning-real-ipc.py` changes; product frontend/native/Core, locks/dependencies, CI and shipping permissions are unchanged. Script SHA-256 `b2bc091e28e88669ef6c85a2e1954bd1c6d25389d73aae53306c2af47ba99018`.

The optional `--structured-editor` mode adds one additional actual UI check before all nine original checks. The default fixture retains one `write-report` node. `node_ids` derives from the same concrete proposal; in default mode the preview membership assertion remains equivalent to the original `{'write-report'}`. The nested duplicate control now uses the actual first node ID, additionally asserts the raw string changed, and still expects HTTP 200 with actual Core `INVALID_REQUEST` and no database changes.

## Source and permission review

- Python AST parse passes. All nine original check names are retained, with ten names in structured mode.
- The source block from `allowed_read` through HTTP server setup, ending immediately before `with sync_playwright()`, is byte-identical to base: SHA `4b2e732464073cd4f79543a512558760413bccdaf8f35dbe5f79eb70f3a29b3f`. This independent extraction includes server setup; the author's shorter adapter extraction uses different endpoints/hash.
- No new HTTP command, approval/finalization route, generic IPC or execution permission was added. Origin/Host/content-type/body-size checks, bounded exact argument sets, exact fixture task/project/scope IDs, non-boolean bounded revisions and opaque raw plan transport remain. Negative 403/400 controls and unchanged-journal assertions remain active.
- Generated local probe is approved outside HTTP and only accepts `--version`. No ACP session, model/vendor account or alternate credential is used. Fixture source/workspace/runtime/token are private temporary data; token is not exposed to browser or evidence. Readonly SQLite snapshots use `mode=ro` and `query_only`; no synthetic SQL rows or fake readiness values are inserted.
- Structured setup approves a second output contract through the actual fixture-owner proposal/finalizer before editing. It verifies actual returned editing metadata against original selected maximum nodes, prefixes and artifact requirements, with rule-executor keys matching scope rules. It does not infer paths from the editor or mint an execution scope.
- Browser opens the initially closed native summary with keyboard Enter, prepares from actual scope metadata, adds the second work node, edits goals/runtime bounds and exact output paths/contracts, and sets an actual dependency. Local edits leave the existing advanced JSON bytes untouched before Apply. Before/after Apply snapshots compare journal, all ledgers, plan rows and execution tables, with no `goal_plans.propose/revise` request before Store.
- Apply must produce the same model with exact two IDs/kinds, 2000/3000ms bounds, dependency, fixed acceptance rules, Goal/scope revision, binding/program, empty recipients and no external action. The separate existing Store button sends actual raw JSON, then direct Core and readonly persisted raw observations must exactly match. Preview admits only the first dependency-ready node, remains readonly and makes no reservations.
- Original raw persistence/CAS/replay/conflict/duplicate/history, strict adapter denial, input drift, revoke/historical recovery and zero-execution assertions remain. Assertion failures propagate through a nonzero process exit; the report starts failed and becomes passed only after all checks. Cleanup shuts down HTTP, terminates/waits/kills its own daemon if needed, and removes its private temporary directory.

## Historical fresh-store evidence examined

Author outputs `/workspace/.onboarding/goal-plan-editor-real-ipc/default-frozen` and `structured-frozen` report default 9/9 and structured 10/10. Their harness SHA matches the reviewed source and daemon SHA is `81e6ec0327f8dda1ff1b721eb381dfdf273e253020b6feed2922e9f13649493f`. These are reviewed author executions, **not this reviewer's executions**. All 15 recorded PNG hashes independently match actual files.

Report hashes: default `0f4f335cadb69f225d351d2fc401797e4cda9408e9a8b657d976b6bbf88e7e24`; structured `0246a17faefccb9246701ff6d8a695116caaeb58fb846cee92f114ae0e298ddd`.

Actually viewed the three structured editor PNGs and all five later structured preview/history/drift/revocation PNGs. Desktop graph shows two light, fine-border cards with the real two goals and `step-1 → step-2`; after Apply it visibly says local draft only. At 390px cards stack into one column, readable wrapping keeps the graph in the content width, and the composer remains below the visible graph. Preview images visibly distinguish candidates from reservations/dispatched work. History visibly lists revisions 1/2 as not dispatched. Drift image shows disabled live controls, but its error alert lies outside the captured viewport. The revocation PNG shows the upper form, not the revoked label; screenshots alone do not prove the API revocation outcome. Runtime assertions and evidence, not those cropped images alone, are required for that claim.

These 81e results are **fresh database only**. Primary separately reproduced a legacy cached-approval comparison defect after optional metadata was introduced. Fresh success does not prove old cached result compatibility; that boundary must not be hidden in the HTTP adapter or relabelled as a successful legacy test.

Independent static checks and all fifteen screenshot hashes are recorded in `/workspace/.onboarding/goal-plan-editor-real-ipc-independent/static-review.json`.

## Evidence limits

This review does not claim native OwnerDialog/WebView, macOS, actual Provider/model, NLP planning, routing, reservation, Goal dispatch or task acceptance. Fixture-owner finalization is actual authenticated Core setup outside HTTP, not OS owner consent. Browser controls are built React over the fixed test-only adapter and real Unix IPC. Full product completion and exact integrated CI remain separate.

## Independent compatible-daemon execution

Frozen Linux debug daemon `/workspace/.onboarding/goal-editor-compatible-binaries/batond`, SHA `2b4a302c4e992df9c01cde404c03ba526cf411eb24ecbc7aadc4e85fb8be771e`, was supplied after the compatibility fix. Its manifest binds metadata `5f4b0de`, editor `1751558`, schema 9, and Core source `589b1a8`. Independently verified exact binary SHA and `git show 589b1a8:crates/core/src/goal_store.rs` SHA `1b040978a6de912a69e6c7d5d82588965fc7291a48774b60e0019157f377d539` match the supplied manifest. This is a source/artifact binding, not a reviewer Cargo execution.

The script was exported directly from immutable `1707ada6` to `/workspace/.onboarding/goal-plan-editor-real-ipc-independent/goal-planning-real-ipc-1707ada6.py`; its exact SHA remains `b2bc091e28e88669ef6c85a2e1954bd1c6d25389d73aae53306c2af47ba99018`. Frontend was explicitly the reviewer's previously independently built, unchanged editor `1751558` tree at `/tmp/agentbaton-goal-editor-independent-1751558/apps/desktop/dist`, which had already passed 259 UI tests/typecheck/build. All three frontend hashes exactly equal the author's fresh-store builds. No frontend rebuild, Node install, adapter change or Cargo run was performed for this execution.

Actual independent commands (new isolated fixture runtime/database/browser context in each process):

```sh
/workspace/AgentBaton/artifacts/browser-venv/bin/python \
  /workspace/.onboarding/goal-plan-editor-real-ipc-independent/goal-planning-real-ipc-1707ada6.py \
  --batond /workspace/.onboarding/goal-editor-compatible-binaries/batond \
  --frontend /tmp/agentbaton-goal-editor-independent-1751558/apps/desktop/dist \
  --output /workspace/.onboarding/goal-plan-editor-compatible-independent/default \
  --chromium /usr/bin/chromium
/workspace/AgentBaton/artifacts/browser-venv/bin/python \
  /workspace/.onboarding/goal-plan-editor-real-ipc-independent/goal-planning-real-ipc-1707ada6.py \
  --structured-editor \
  --batond /workspace/.onboarding/goal-editor-compatible-binaries/batond \
  --frontend /tmp/agentbaton-goal-editor-independent-1751558/apps/desktop/dist \
  --output /workspace/.onboarding/goal-plan-editor-compatible-independent/structured \
  --chromium /usr/bin/chromium
```

Both processes exited **0**. Default **9/9 checks PASS**, **6 PNGs**; structured **10/10 checks PASS**, **9 PNGs**. Every check has passed status, page errors are empty, all fifteen actual PNG hashes match their reports, and exact script/binary/three frontend hashes match input records in both modes. Observed empty history is genuinely `[]` without adapter translation. Structured local-edit and after-Apply snapshots are exactly equal, followed by actual Core storage of the edited original model/raw bytes. Original replay/CAS/duplicate/preview/no-reservation/history/drift/revocation checks run in both modes.

Final database in each mode: zero attempts, dispatch intents, workspace leases, sessions, command validation runs/fences, Goal node evidence and external-action records; one plan head, two immutable plan revisions and two proposal rows, journal sequence 20. No report/details file was created, probe invocations were only `--version`, and the task remained `draft`. These new databases contain genuinely measured zero history; no historical unknown meter was replaced with zero.

Artifacts under `/workspace/.onboarding/goal-plan-editor-compatible-independent/`:

| Artifact | SHA-256 |
| --- | --- |
| `default/report.json` | `35a20a98d6bc3e6eb16bcf413aac3e56cb3271756c016afa5c4bded0d654d1a7` |
| `default-run.log` | `c705dbd015728d3d42ce402e3b1332b5033dc8fd91e2f9316231e4aa105ef95e` |
| `default/daemon.log` | `3a8c14092dac08ec0375c1fdda6dcc8e5b975c8d6cb2a2c7d899cd30269c3953` |
| `structured/report.json` | `7041675cce7b67ae0bae21d0e902b1085f3bce75cfc48a8cdd2660f35fefa6f7` |
| `structured-run.log` | `f186bb336987d39fae25c25c5404d7c4deafd5536e67a88299bf8a5088ae4008` |
| `structured/daemon.log` | `e6241e7df77e2762f90628f467b3a34a05b6a35980fa4f0f6101a1fe89bc64b6` |
| `input-check.log` | `6d40a3d9a190995f576e6b5c75bcf43a1ac8e620036145e8c95fec39afecd245` |
| `input-manifest.json` | `dbe13a4663c6b37ba90207d7df8256aeedfbe3ac0ff517d98224b820ca1414af` |
| `independent-verification.json` | `921ac45a963c64b56ab8ddbd594ac4623cf0e2d33ae66a4b64920c1efa56957c` |

Actual frontend hashes: `index.html` `e072964df52904b36aac43d1a25978e66812d24fb148ddb8849d99a03128e954`; `index-B3WfCqru.css` `3dc2b0859eaa5552936928bc10dbf03b008f52e582a8195a3a1b6466005cf98f`; `index-ojfhYZcd.js` `232b7024a08f05ba75c01ad77232c464e1187bfef647cd0e2eeb80c6c1db32bc`.

Independently viewed this new run's three structured editor screenshots, default desktop/mobile preview and structured two-revision history. The actual graph, local-only Apply message, responsive stacked cards and two not-dispatched history revisions match the reviewed author screenshots. At 390px the graph is readable without horizontal overflow and visible above the composer; desktop is a two-card graph with fixed acceptance/CAS context. Preview explicitly describes candidates rather than reservations/dispatch. These are Chromium product screenshots, not native/macOS proof.

All new screenshot fingerprints:

| Screenshot | SHA-256 |
| --- | --- |
| `default/01-fresh-goal-expanded.png` | `525bd4d0fa3c538a6d98148ae069414b442ff9a58203f9cbec75e83dc6c11c20` |
| `default/02-read-only-preview-desktop.png` | `0721f5428960334d6c43f9937e7802988c70fc09f4792bd74c936f384867c93b` |
| `default/03-read-only-preview-mobile.png` | `11d9e62118f29ead12d409eb60abd58de3c5693f97167e9eb56777dae7acf86f` |
| `default/04-versioned-plan-history.png` | `9c31f7b072c856a31d084a909490543464faaf9df13ae2cbf450d08085579c28` |
| `default/05-input-drift-read-refused.png` | `9a13d835697e31750522ea360c3ae555e8d035c863238f7dfb2b33a2a75a4f82` |
| `default/06-revoked-historical-scope.png` | `1ec06d591de5b9442b57bc035762a6be392a793075354e309409ddfbd90f598d` |
| `structured/01-fresh-goal-expanded.png` | `6e83533f0ba6c8e0f394caee8f000af0ef3fcfdcd6ffc81eec8523ee36fcb90f` |
| `structured/editor-01-local-steps-no-write.png` | `82a8d8a33cf4ec24f67202c3b82de203b33f65e845f5b4a56bcf9500d35eae39` |
| `structured/editor-02-applied-local-draft.png` | `a95ea5800356b0582b6e93469a897003262a2a69d92b73e09f3fbfab6053f62b` |
| `structured/editor-03-mobile-dependency-graph.png` | `6155700b78ab2c98379389ef2c2f5bef7e3c99bd10e6fe2feec715ae4f1cc195` |
| `structured/02-read-only-preview-desktop.png` | `30575e7aad4445b6a0a3d37b6eb610e74fb5ef1a03d316878070482af6b09c82` |
| `structured/03-read-only-preview-mobile.png` | `0e7b286d9ccef824711ca14a78f425fe8ea0bd9d9fc3873e5a0b5b2045bf2bd6` |
| `structured/04-versioned-plan-history.png` | `b3e4cbc806ece901cf3906a8d35f8de4d9440d98acc0101a76aa2e89c350640f` |
| `structured/05-input-drift-read-refused.png` | `1cf0bf58c88d000b14d90d0578e520ca878f5007757c4da68fbdef060e411b45` |
| `structured/06-revoked-historical-scope.png` | `65af2d7d9eef1ac9be2a3637cba2fcb49028cda4250617fb4e02041844e5d2a1` |

The initial input-inspection helper successfully checked binary/script/frontend hashes and wrote `input-manifest.json`, then its generic attempt to display all remaining binary-directory files as UTF-8 raised `UnicodeDecodeError` on the `baton` binary. This was an inspection-helper error after assertions, not a harness/product failure. A subsequent restricted JSON-manifest/source/input check exited 0 and is captured in `input-check.log`; neither immutable harness nor product was changed to recover it. Both actual browser/IPC processes succeeded on their first runs.

### Fresh-flow versus legacy-record evidence

These independent runs instantiate new private databases and establish **fresh UI/Core composition on the compatible binary**. They do not seed or migrate an old cached approval. The primary reported its 34 GoalStore tests (including actual persisted legacy missing-field recovery and explicit-null/present-context tampering refusals), Core lint/fmt/build passing for `589b1a8`; this reviewer did not run those tests or the separate old→new compatibility probe, and does not claim that independent compatibility review was complete when these commands ran. Keep that Core evidence/review alongside, rather than substituting these 9/10 fresh runs for it.

The HTTP adapter stayed byte-identical, returned actual Core responses, and did not remove/replace metadata, null, error or cached confirmation fields. No failure was converted into synthetic success. No source, lock, permission, staging, commit or push operation was performed by this reviewer; only this report and ignored evidence files were written. Native OwnerDialog/WebView, both macOS architectures, Provider/model, NLP, dispatch and task acceptance remain outside the established evidence.
