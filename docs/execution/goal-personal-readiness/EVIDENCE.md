# Personal-task Goal readiness evidence

## Exact scope and semantic baseline

Base `802273c6b262e96e25192b69996ed40c5243fbe2`; source `78badec275ec25c84363834ec73c8ad74e65024f`, branch `codex/goal-personal-readiness`. Only GoalPlanning.tsx, goal-i18n.ts and new GoalPlanning.local-project.test.tsx changed. Allowed execution records are this directory's PLAN/PROGRESS/CHECKPOINT/EVIDENCE. No original test, public contract, App/API/Core/native/lock/CI change.

Core inspected at base: `storage.rs::create_registered` explicitly bypasses registered project lookup only for exact `local`; authenticated `ipc.rs` tasks.create calls that method. `registry.rs` creates imported project IDs with `Uuid::new_v4()` and missing project lookup returns actual NOT_FOUND. The UI guard follows that reserved project ID, **not** the `local` execution host or an arbitrary ID prefix. It never catches NOT_FOUND or network errors and returns fake empty lists.

The existing disclosure stays initially folded. Opening a personal task shows clear bilingual prerequisites and no Goal RPC/actions; all method mocks remain uncalled even through disconnect/reconnect and reopening. Registered project IDs (including a `local-` prefix) still execute their original reads, show genuine errors and keep writes disabled on failure. Successful empty registered reads retain the existing missing-root-grant flow. A registered pending approval survives navigation through a personal task and recovers only through explicit click with exactly the original selection/key. All deeper history/CAS/revocation/plan recovery remain covered by the original unchanged259 tests; the entire registered GoalPanel function is byte-identical.

## Actual commands and results

Working directory `/workspace/.worktrees/goal-personal-readiness/apps/desktop`. Borrowed only existing locked dependencies using a worker-created exact symlink; no install, update or lock edit. The link was removed after commands.

```sh
npm test -- src/GoalPlanning.local-project.test.tsx  # old source, actual negative control
npm test -- src/GoalPlanning.local-project.test.tsx  # repaired source
npm run typecheck
npm test
npm run typecheck
npm run build
```

- Old-source control: **4 failed /3 passed**, seven cases, exit1, 2026-10-27 08:03:25 UTC, 1.46s. Personal prerequisite assertions (both languages, reopening and personal navigation during registered recovery) fail because the old product mounts the Goal panel. Real registered error/empty-read controls pass. This is mocked UI evidence; the primary's previously reproduced real local grants.list NOT_FOUND is external evidence, not a runtime executed by this unit.
- Repaired targeted suite: **7/7 PASS**, 08:04:50 UTC, 1.58s.
- Initial typecheck: **FAIL** in the new fixture (Task.project_id string-or-undefined used where grant needs a string). Fixed the constructor's accurate `Task & {project_id:string}` return type. No product types or checks were weakened; old log retained.
- Final full UI: **266/266 tests in17/17 files PASS**, start08:05:08 UTC, duration12.56s; **typecheck/build PASS**. All original259 tests in16 files byte-identical, no skip/deletion/deadline/config change. `git diff --check` passes.

| Log under `/workspace/.onboarding/goal-personal-readiness/` | SHA-256 |
| --- | --- |
| `old-source-control.log` | `c3e3c4a052b6de85b338e9858743982adcb1014d5349fa3ae8523b9fe54e2106` |
| `full-ui.log` | `58e12761bfa6a3fa94de482e746a7ac3165b0fc81fff7665c4a2040f3d8268d1` |
| `typecheck.log` | `a874de18b22d8b38603d32508bc1ee99ccddc8962619093d012b958fac8949c2` |
| `fixed-targeted.log` | `f66bbead4837ed568d2dd96c3756a21b916570213c61bc1ac3a5614632d4aafe` |
| `final-typecheck.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
| `build.log` | `fceb3c6f9d6e578578bd3c5a9074fe1dbdcf98873f1b7faea4d9bfed46ae23c5` |

The old-control file's physical `<form>` check was tightened after the old failing run (before product repair); it does not weaken or change any reached failing prerequisite assertion. The final full suite runs every acceptance case without a selection filter.

## Source and artifact fingerprints

| Owned source | SHA-256 |
| --- | --- |
| `apps/desktop/src/GoalPlanning.tsx` | `7f3b1de644f84406cf65049951b66d720a0ed5fbf4096bf488c0eaa0f34bf3d9` |
| `apps/desktop/src/goal-i18n.ts` | `1853711d0836001edbcc2a788d46222c51e0220d004bdc0a07828fcbd47cf2e9` |
| `apps/desktop/src/GoalPlanning.local-project.test.tsx` | `b2d1f5e6ecf8865a0d227c1140f7c7f11e83b2d0a89b62d788e95b5b87b4928f` |

| Built frontend file | SHA-256 |
| --- | --- |
| `index.html` | `fd946ac2dcf9de61d7e35ce5e74765ce0eb4718730e130d2f1cc7069df6edee2` |
| `assets/index-B3WfCqru.css` | `3dc2b0859eaa5552936928bc10dbf03b008f52e582a8195a3a1b6466005cf98f` |
| `assets/index-Dq4xVrvp.js` | `16ff5a9a3a53329acf5896ded33cc1eaff11d0bae7b0707c67cbf11eb0438d65` |

Full machine-readable check record `/workspace/.onboarding/goal-personal-readiness/checks.json`, SHA `96f3bf0c5a18931966a76a1c13574ba169d4a730a3a32e2872663a16cd94ea81`.

## Review and evidence limits

Author self-check completed; primary's independent review is pending. No recursive reviewer/agent, extra model API, real vendor/account, Cargo/shared target, native OwnerDialog, Unix daemon/browser run, screenshot, macOS build, CI run, installation artifact, grant creation, dispatch or Task acceptance was performed. Mock UI controls are not real Core or native proof. No new RPC/permission or product scope was introduced. This small readiness correction is not completion of the full Goal or Design v0.2.
