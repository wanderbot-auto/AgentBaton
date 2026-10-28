# Independent Goal plan editor review

Status: **PASS for the reviewed frontend slice at `1751558feeaeb51bf3604be5dd6c2ed7acb21887`; F1 closed after independent regression and full checks. No remaining blocking finding in this scope.** F1 was a local draft-intent defect, not a Core authority bypass. OWNER-001/002 read and ACK; no authority was inferred from tests or agent output.

## Immutable sources and scope

Reviewed UI source `92416daa28174ffeb735cd72e31c911af59112dd` against `d30bb9e`. Exactly six owned frontend files changed. All 15 baseline test files (229 tests) are byte-identical; the new editor file adds 29 tests. No native/Core/dependency/lock/permission change in this UI source.

Separate metadata source `5f4b0dea34d04d9bc9ea6f1e17305aee87f3b2c1` was reviewed statically: `edit_context` is an optional, default-None view of recorded original approval fields. It projects maximum nodes, original writable prefixes, output artifact requirements, and existing host-policy rule executor kinds. It does not mint a scope or change the approval or Store write path. Its two added Store tests cover old cached results, readonly revoked history, and rejection of a forged display projection. This reviewer did not run Cargo or those tests.

## Findings

F1 — numeric draft cache key prefix collision (small fix required in this slice). Both `a` and `a.b` are legal node IDs. With unrelated owner node `a` and work producer `a.b`, entering unsafe runtime text `9007199254740993` for `a.b` correctly disables Apply. Removing `a` then uses `startsWith(id + '.')` to remove numeric cache entries, also deleting `a.b`'s input. The displayed runtime silently returns to its old valid value (`60000`). This loses unrelated user intent and can unblock Apply. It does not widen Core limits or permit unsafe numeric values through the protocol.

Independent adversarial test was appended only in the reviewer's temporary exported copy, not the author's test file. Its exact old-source run exited 1 (one selected failure, 29 unselected cases) at the retained unsafe-text assertion, expected `9007199254740993`, received `60000`. Original full 258-test pass was run before this additional control. Parent and author were notified; parent requested a precise-key fix while retaining legal dotted IDs and unrelated field state.

## Reviewed behavior

- Uses the existing `GoalPlanProposal`, strict raw JSON parser, typed native API, plan ID, scope/goal binding and version CAS. No second workflow DSL, new IPC command or renderer authority field was introduced.
- Expanding or reading an unchanged raw draft does not call `onApply`, normalize its bytes or store it. Explicit prepare/load, actual edits, Apply-to-local-JSON, and separate Store remain distinguishable. Local Apply clears a stale readiness observation without dispatching or reserving anything.
- Prepared next revisions retain plan identity and fixed acceptance rules. Changes in latest revision/digest, raw JSON, scope digest or editing context block the existing editor draft until explicit preparation/load. Existing unknown-request recovery still freezes raw bytes, key, CAS and scope; the editor is disabled during pending requests and stale/disconnected/unfresh authority.
- Missing/invalid editing metadata disables structured changes without inventing paths or rule kinds. Fixed execution binding/program/workspace, exact approved input SHA, output contracts and unique producers are checked. Nodes cannot add recipients/external effects; executor kinds and owner/validator rule mappings are restricted; node/parent budgets, reservation, retry, prefix boundaries and maximum nodes are bounded. Core still checks real authority, paths, current input versions, complete frame and shared consumption.
- Dependencies, parent lineage and upstream input references share one cycle check; dangling references and deleting a referenced node are refused. Owner/validation nodes request evidence and do not assert passing rules or task acceptance. Dependency cards describe proposals rather than running work.
- Editor is a closed native disclosure inside the existing focused task Goal plan section. Namespaced styles use existing light tokens, 6px cards and responsive two/one-column graph cards. Labels, fieldsets, ordinary buttons and keyboard editing are tested in jsdom; actual native summary Enter, mobile layout and new screenshots remain for real-browser composition.

## Independently executed checks, initial source

Exact `apps/desktop` tree was exported with `git archive 92416daa28174ffeb735cd72e31c911af59112dd apps/desktop` to `/tmp/agentbaton-goal-editor-independent-92416daa`. Only that copy borrowed `/workspace/AgentBaton/apps/desktop/node_modules` via a reviewer-created symlink. No author dependency directory was changed and no dependency was installed.

Commands in the temporary copy's `apps/desktop`:

```sh
npm test
npm run typecheck
npm run build
npm test -- src/GoalPlanEditor.test.tsx -t 'independent:'
```

- Full unmodified source: **258/258 tests, 16/16 files PASS**, 12.34s, start 2026-10-27 05:16:48 UTC.
- Typecheck and production build: **PASS**.
- Additional F1 control against original source: **FAIL**, as detailed above; it is not counted as a passing product test and its name filter does not skip any acceptance test in the full run.

Logs under `/workspace/.onboarding/goal-plan-editor-independent/`:

| Log | SHA-256 |
| --- | --- |
| `tests.log` | `cf51ebbce8fc8e2b6ceffa4a0ee698c34b08c7fd64944329dbbe76b8db6c905f` |
| `typecheck.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
| `build.log` | `caa77eb1ff67d650fdcbb054d089ad8c88a8a78fa3f2baac234a70cf5e8bdf73` |
| `numeric-id-collision-control.log` | `65e1d210e9e391cb6a4c4719294aa7d5d97c685423c15eee1b93252374811a43` |

Initial source fingerprints:

| File | SHA-256 |
| --- | --- |
| `GoalPlanEditor.tsx` | `07a46d96b8cf4e24609fe4bf4ffa9b39dac7613d22baf997abc639382cd5e15f` |
| `GoalPlanEditor.test.tsx` | `37e7bf5fcab76b91247e404c807ff2c351e0a66b78bcbc194f5e07f43fa58b46` |
| `GoalPlanning.tsx` | `0bf8b6effc873dccd2eadb9082823eb79fb19a1694b970315cd5b9258e34b38e` |
| `goal-api.ts` | `ccf231baae4da45a9cb1ba527b527365e6d98e07b7e0833b6736e4dd7af879b1` |
| `goal-i18n.ts` | `165b83dfdfc13d220e0be116f84e56a1821929e582068f624a4e6e3b4932040a` |
| `style.css` | `dd0d110af7bfd0243698c624477e98ac2abe59259c0b3d3ee65de1ce57904092` |

The independent initial JS (`3e73a74cd82e2686870686fce9b7a84c044babfce357fc27587aaa17ddf9bc03`) and CSS (`3dc2b0859eaa5552936928bc10dbf03b008f52e582a8195a3a1b6466005cf98f`) exactly match the author's assets. Metadata source SHA-256: `goal_store.rs` `6bb8f618d7c56a47855b431e3e908250a2b3abc441a1c68b8ccad48a8e5fa2e0`; `goal_store.rs` tests `d2181ef025a3b98f6ff4ddb94329970da8776469b33616b13d2ff6a5da4e8228`.

## Evidence limits

UI transport tests are mocked jsdom tests. No editor real Unix IPC/browser/OS Owner dialog/macOS build or screenshot was executed by this review. No new Core test execution, Provider account/model, natural-language planner, graph scheduling, reservation, dispatch or task acceptance is proved here. New metadata UI/Core composition and exact integrated CI remain separate checks. Full Design v0.2 delivery is not complete.

## F1 closure and final immutable source

Final source `1751558feeaeb51bf3604be5dd6c2ed7acb21887` changes only `GoalPlanEditor.tsx` and appends one regression to `GoalPlanEditor.test.tsx` relative to the initially reviewed source. The original 29 editor cases are a byte-identical prefix of the final test file; all original 15 baseline test files remain byte-identical to `d30bb9e`. No timeout/config/dependency/authority restriction was weakened, and dotted node IDs stay legal.

The numeric input cache is now `Map<exactNodeId, Record<fixedGroupField, text>>`. Updating, displaying, checking all invalid input, resetting an explicit load, and removing exactly one node use this same structure. Removing `a` uses `Map.delete('a')` and cannot affect `a.b`. The regression retains the unsafe text, disabled Apply and no callback assertions. Its remove button locator uses an anchored exact name, preserving the independent control's selection semantics with the installed TypeScript definitions.

The author's old-source reproduction was read and hash-verified: `/workspace/.onboarding/goal-plan-editor/f1/old-numeric-collision.log`, SHA `6b9a614aebce1666e95c9865780a447b35924a9ef9d0acba28954c5ff5aecca0`. This is distinct from the independent failure log above. Old failures were preserved. The reported author's checks are not counted as this reviewer's execution.

A second immutable `git archive` export to `/tmp/agentbaton-goal-editor-independent-1751558` ran the full unmodified final suite, typecheck and production build independently:

```sh
npm test
npm run typecheck
npm run build
```

Actual results: **259/259 tests in 16/16 files PASS**, including the new collision regression, start 2026-10-27 05:20:37 UTC, duration 11.67s; **typecheck PASS; build PASS**. Final source fingerprints:

| File | SHA-256 |
| --- | --- |
| `GoalPlanEditor.tsx` | `5dc2a7a31290eaa5f315fafe7678e9d62ed6f7205cc8a3e4ae6b15eb7a87198a` |
| `GoalPlanEditor.test.tsx` | `d29288f62757b237ee480aff7527d3a579c1c1ed11959224cd8bcdfbf906e26e` |

The other four frontend source hashes are unchanged from the initial table. Independent final log and asset hashes:

| Artifact | SHA-256 |
| --- | --- |
| `/workspace/.onboarding/goal-plan-editor-independent/final-tests.log` | `f930045510e3300ec1433fb960a43dc656a2e4366d643db1c139a0c7465f5836` |
| `/workspace/.onboarding/goal-plan-editor-independent/final-typecheck.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
| `/workspace/.onboarding/goal-plan-editor-independent/final-build.log` | `ddb5c26af1d39069077aca2e07302b63ec55a6d808e9f8ed6e6f8b1c8484de31` |
| `index-B3WfCqru.css` | `3dc2b0859eaa5552936928bc10dbf03b008f52e582a8195a3a1b6466005cf98f` |
| `index-ojfhYZcd.js` | `232b7024a08f05ba75c01ad77232c464e1187bfef647cd0e2eeb80c6c1db32bc` |

Both reviewer-created temporary dependency symlinks were verified to target exactly `/workspace/AgentBaton/apps/desktop/node_modules` and removed after their checks; the installed dependencies and author worktree dependency path were untouched. Only this review report was written in the repository. No staging, commit, source/lock change, Cargo test or subagent was performed.

Final acceptance here covers the reviewed frontend source and mocked UI behavior. The separate metadata Store tests, actual editor UI→Core IPC, browser captures, native dialog and macOS evidence remain explicitly outside this execution. The graph is a proposal representation, not dispatch proof.
