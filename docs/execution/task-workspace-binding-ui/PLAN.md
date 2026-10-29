# Task-bound code workspace UI

Requirements AB-PRJ-002/003/005, AB-SEC-007, OWNER-002. ACK OWNER-001/002; AB-START-001/current product gate. Base `73de1bb86c0777647c53e26e1cf3c5c355cbbb77`, isolated `codex/task-workspace-binding-ui`. Full Design v0.2 scope retained; this is one dependent UI unit.

Use existing TaskGitWorkspacePreparation, CodeWorkspacePreparation and typed Task binding parsers/fixed APIs. Fold workspace controls into Task Details; preserve the quiet-canvas2-column homepage. Explicit read task/project/source/local-host preconditions before a new user-requested isolated Git worktree. Server chooses destination; no directory copy/init, arbitrary path/shell or Provider permission.

Keep original reservation receipt, current Task binding and freshly revalidated managed identity separate. A ready record/candidate is not execution authorization. Preserve exact original selection/Task CAS/key under uncertainty, require a successful explicit Task read, then user-triggered same-key recovery. Never automatic write retry/new key. Same-document remount memory is bounded; full browser restart may recover an already persisted receipt through tasks.get, but uncommitted client memory is not durable.

Allowed paths: new TaskCodeWorkspace component/bilingual strings/tests, minimum App Details insertion/local CSS, these4records. Core/API wire/Native/schema/locks/CI untouched. Root owns actual Node/type/build/browser/Core execution and integration.
