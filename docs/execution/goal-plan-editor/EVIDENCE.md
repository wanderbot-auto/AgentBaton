# Goal plan editor evidence

## Exact source and requirements

Source commit `92416daa28174ffeb735cd72e31c911af59112dd`, base `d30bb9e`, branch `codex/goal-plan-editor`. OWNER-001/002 ACK. No modified baseline requirements or approval records.

| Requirement | Implemented evidence | Actual verification / limits |
| --- | --- | --- |
| AB-TSK-011 manual versioned steps | `GoalPlanEditor`, one existing `GoalPlanning` plan-section mount | 29 new tests plus original 229; initial/latest revision, same plan ID, fixed acceptance, no overwrite; mocked native transport |
| docs/06 §6 same workflow model | Existing `GoalPlanProposal` only; dependency/input/lineage adjacency, Work/Summary/Validate/Owner nodes | Local DAG/reference/producer/contract/budget failures tested; no second DSL, runtime scheduling or generated planner claim |
| OWNER-002 progressive task UI | Editor is a nested closed disclosure in existing focused Details plan; namespaced cold white/gray hairline/6px/14px styles | Semantic/labeled form and keyboard controls tested in jsdom; screenshots/mobile/native summary Enter are not run here |
| No authority/acceptance expansion | Fixed scope binding/rules/contracts; optional readonly original-approval editing projection | Missing/malformed metadata disables structured changes; Core/native authoritative; real metadata backend/browser integration pending |

SHA-256:

| File | SHA-256 |
| --- | --- |
| `GoalPlanEditor.tsx` | `07a46d96b8cf4e24609fe4bf4ffa9b39dac7613d22baf997abc639382cd5e15f` |
| `GoalPlanEditor.test.tsx` | `37e7bf5fcab76b91247e404c807ff2c351e0a66b78bcbc194f5e07f43fa58b46` |
| `GoalPlanning.tsx` | `0bf8b6effc873dccd2eadb9082823eb79fb19a1694b970315cd5b9258e34b38e` |
| `goal-api.ts` | `ccf231baae4da45a9cb1ba527b527365e6d98e07b7e0833b6736e4dd7af879b1` |
| `goal-i18n.ts` | `165b83dfdfc13d220e0be116f84e56a1821929e582068f624a4e6e3b4932040a` |
| `style.css` | `dd0d110af7bfd0243698c624477e98ac2abe59259c0b3d3ee65de1ce57904092` |

## Actual commands

Working directory `/workspace/.worktrees/goal-plan-editor/apps/desktop`. Existing locked dependencies were borrowed via a worker-created symlink; no install or dependency edits.

```sh
npm run typecheck
npm test -- src/GoalPlanEditor.test.tsx
npm test
npm run build
```

Final exact-source full run began 2026-10-27 05:11:55 UTC: typecheck **PASS**; UI **258/258 in 16 files PASS**, 10.52s; build **PASS**. Targeted editor test **29/29 PASS**. Original 229 tests were not edited, deleted, skipped or weakened. No timeout/config change.

Exact-source logs in `/workspace/.onboarding/goal-plan-editor/92416da/`:

- `typecheck.log` SHA `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e`.
- `ui.log` SHA `a4fd4737da454a808b704dbb0ada7bb3e22f468beefba9ee96ba58706dd953d1`.
- `build.log` SHA `692d89aafc7e884d57d6e8d350b59c412ebb916761f0c4d10f42d7a7602f5a92`.
- Earlier actual 256/16 full pass and final-editor 29 pass logs remain under `/workspace/.onboarding/goal-plan-editor/` and `final/`; they are not overwritten by the exact-source run.

Build assets: `index-B3WfCqru.css` SHA `3dc2b0859eaa5552936928bc10dbf03b008f52e582a8195a3a1b6466005cf98f`; `index-DU2dWPVh.js` SHA `3e73a74cd82e2686870686fce9b7a84c044babfce357fc27587aaa17ddf9bc03`.

## Retained initial failure

Actual targeted run at 05:09:44 UTC returned exit 1: **28 passed / 1 failed**, 29 tests, duration 2.11s. New test `supports keyboard expansion and ordinary labeled form editing` expected `<details open>` after keyboard Enter on summary, but jsdom returned `open=null` (`GoalPlanEditor.test.tsx:74:364`). This is the actual tool-captured output; no filesystem raw log was captured for that exploratory run. It is not claimed a browser/native product failure.

The fixture now uses the supported native click disclosure and preserves an open assertion, then checks real focus/Tab/Enter button activation and keyboard text editing. No fake event is added to production, default browser behavior is not overridden, and native summary Enter remains for real Chromium validation. The final test count remains 29.

## Pending dimensions

Independent review, primary's readonly metadata backend tests/integration, actual editor/Unix IPC/browser operation and screenshots, native OwnerDialog/WebView, both macOS architectures and exact integrated CI are **not run by this unit**. No Provider/session/model, paid resource or private credential was used. No NL planner, routing, reservation, dispatch or Task acceptance is established by plan editing or passing mock transport tests.

## F1 exact-node numeric-cache correction

Current source `1751558feeaeb51bf3604be5dd6c2ed7acb21887`; initial `92416da` source and its evidence above are retained as history. The independent reviewer first ran the original full 258/types/build successfully and then found an adversarial local draft loss. Independent old failure: `/workspace/.onboarding/goal-plan-editor-independent/numeric-id-collision-control.log`.

Author copied the exact control semantics read-only: nodes `a` and `a.b` are legitimate independent nodes, `a.b` owns the output, `a` is a separate Owner input-review node with no dependencies. Set `a.b` runtime to unsafe `9007199254740993`, assert Apply disabled, remove exact `a`, then assert the same unsafe text remains, Apply stays disabled and no `onApply` occurs. On old product source the author's run actually **failed, exit 1**, received `60000`, at test line 101. Filtered execution intentionally selected the new control only (1 failed, 29 outside-filter tests not run), not a weakened full-suite acceptance.

```sh
npm test -- src/GoalPlanEditor.test.tsx -t 'removing node a retains'
npm run typecheck
npm test -- src/GoalPlanEditor.test.tsx
npm test
npm run build
```

The fix replaces delimiter/prefix numeric storage with `Map<exactNodeId, fixedGroupFieldRecord>`, exact lookup and exact `delete(id)`. It preserves dotted node IDs, the untouched node's entered text, and blocked Apply. No raw JSON, CAS, approval, metadata, old test, model, backend, native, lock, harness or timeout change.

Initial copied control used `getByRole(...,{exact:true})`, an unsupported option in the installed TypeScript `ByRoleOptions`; its type-check failure is retained. Replaced only that selector with the equivalent anchored accessible name `/^Remove step a$/`; all behavioral assertions and exact matching remain.

Final: **typecheck PASS**, **30/30 editor tests PASS** (05:19:54 UTC), **259/259 full tests in 16 files PASS** (05:19:57 UTC, 10.92s), **production build PASS**. All previous 229 tests are byte-identical to base. No new real backend/Chromium/native verification is implied.

| F1 artifact | SHA-256 |
| --- | --- |
| `GoalPlanEditor.tsx` | `5dc2a7a31290eaa5f315fafe7678e9d62ed6f7205cc8a3e4ae6b15eb7a87198a` |
| `GoalPlanEditor.test.tsx` | `d29288f62757b237ee480aff7527d3a579c1c1ed11959224cd8bcdfbf906e26e` |
| `f1/old-numeric-collision.log` | `6b9a614aebce1666e95c9865780a447b35924a9ef9d0acba28954c5ff5aecca0` |
| `f1/typecheck.log` (initial copied-test type error) | `945506c56b29177f7fc607635f6ee9280a1a70e8d7b157b9b09da736dfa15607` |
| `f1/typecheck-final.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
| `f1/editor.log` | `68560af609942cb544eaf70e0c7e62362304d59276fbfce59df533c8bf81c867` |
| `f1/ui.log` | `5e7d91807772c204e114c334137196c176d17e3dedf1179c5e9cfea05685123c` |
| `f1/build.log` | `7763a8ef000414766f8fb5b08bef50eb6a9fb2bca8083640886135eb19f77e4e` |

All F1 logs are under `/workspace/.onboarding/goal-plan-editor/f1/`. Other four product files retain their `92416da` hashes above. New built JS `index-ojfhYZcd.js` SHA `232b7024a08f05ba75c01ad77232c464e1187bfef647cd0e2eeb80c6c1db32bc`; CSS is unchanged. The worker-created exact dependency symlink is removed after checks, with the original dependency directory untouched. Independent F1 re-review and primary's real metadata/editor IPC/browser validation remain pending.
