# Checkpoint

- Base `73de1bb86c0777647c53e26e1cf3c5c355cbbb77`; source commits `fbcff12644a40e438a264d8efad7064f315de6ef` then `8366b26b71968b826893eb335ae81e5a411323af`. Branch `codex/task-workspace-binding-ui`; tree `/workspace/.worktrees/task-workspace-binding-ui`. Original shared-budget and Data leaves preserved.
- Only5sourcefiles changed: App.tsx, TaskCodeWorkspace.tsx, TaskCodeWorkspace.test.tsx, task-code-workspace-i18n.ts, style.css. No Core/API/Native/schema/locks/CI changes.
-24 declared mocked-transport component cases are source-only: lazy reads; exact pin/CAS/key; original-vs-current/fresh identity; lost reply/remount; read-failure gate; prior uncertainty; pending/unknown; same-receipt replay; candidates/lease; directory/personal/archived/unknown task; deferred foreign reads;4readonly drifts; false authority; original CAS under newer Task; receipt swap/rollback; foreign project; bilingual keyboard path. No test success claimed.
- Root next: independent review; typecheck -> focused component tests -> full original frontend suite -> build with its CPU lease. Example focused command `npm test -- src/TaskCodeWorkspace.test.tsx --maxWorkers=2`. Real IPC/browser must use exact source/tool/assets and genuine Task binding scenarios; mocks cannot establish worktree isolation, permissions or native macOS behavior.
- Own light evidence `/workspace/.onboarding/task-workspace-binding-ui`; no remote action, Cargo, Node, Actor, daemon/browser, credential or Provider use. Complete product/130 requirements remain unfinished.


## Review follow-up 422dd195c83c7f5b28e1a967a5dec00f3c243f30

Independent source review identified a P2 in the earlier8366/fbc path: after a successful FS identity observation, same-key revalidation could return INPUT_CHANGED/not_started while stored Task binding remained identical. Catch retained saved.observed; later ordinary TaskGet/Projects refresh could display it as revalidated again. This is a source-reachable invalidated-proof issue, not a claimed executed component/browser failure. Original source and sealed phase evidence are retained.

Fix only invalidates saved.observed plus mounted displayed observation in perform.catch. It does not change pending keys, full original payload, receipt, CAS, uncertain recovery or Core behavior. Added1 exact same-key success→FS failure→Task-only refresh remains stored-only→new explicit successful identity check restores proof regression. Original24test specs remain byte unchanged; total25 declared, all NOT_RUN. Python inverse/source checks and diff0 only.

Archived-history confusion in review used old fbc:8366 already avoids resolver on archived projects. Active-project cancelled/unknown/accepted records remain subject to real resolver's non-draft blocked result, and new prepare also requires Task.state=draft. No test-shaped permission bypass or global Unknown exception added. Root owns Node/type/build/browser validation.
