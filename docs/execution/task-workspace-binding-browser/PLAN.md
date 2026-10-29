# Task workspace binding browser / real Unix IPC acceptance

## Current revision — independent F1 correction

Current source `27e39defdf49bb8133577a65ed94515065fe4687` (parent3a1d52c). The original3a source is **BLOCKED**, not runtime-ready: an independent exact-AST own-surrogate probe proved that trusting Core-returned canonical_root could write/rename another legitimate directory. The probe used only the reviewer's generated temporary directories; it did not run the product. Original18 pure PASS did not establish FS confinement. Original evidence remains unchanged; the historical sections below retain original source timing and checks.

New ManagedFixturePaths independently pins this fixture's temp/runtime/workspaces parents (canonical non-symlink directories, mode0700, lstat device/inode), then requires receipt Task/project/host/source/managed-root, bounded single workspace component and actual workspace canonical/lstat identity matching original decimal string pin. No daemon-returned path alone authorizes a test FS effect. Managed input leaf reads are NOFOLLOW; output uses a revalidated directory descriptor and O_EXCL/NOFOLLOW. Drift injection is Linux-only descriptor-relative `renameat2(RENAME_NOREPLACE)` with no overwrite fallback, verified moved inode and exclusive mkdir. Existing or after-precheck retained destinations are refused; all retained content remains under this generated namespace. This is a test harness guard, not a new product OS sandbox or permission.

Author actually ran27 tests /0.018s /exit0: original18 plus9 actual self-owned tempFS cases. These cover external Core-shaped paths, wrong Task/project/host/source/component, wrong or numeric/noncanonical identity, symlink workspace/parent/input/output, exclusive output, normal private drift, existing retained directory/symlink and injected destination creation before the actual NOREPLACE syscall. Surrogate external file bytes remain unchanged. No child process, Git execution, Core, daemon, Node or browser is involved in these tests. Two AST parses and diff-check0; seven original top-level boundary/protocol/status/preflight functions remain AST-identical. Do not sum original18 and27 as45 unique tests.

Independent new-source review pending when these author records were frozen. All four real Core/browser scenarios and all PNGs remain NOT_RUN. No runtime or product approval is implied by27 lightweight controls. Root owns the subsequent exact-source run after independent clearance. The original run command below is unchanged.

Baseline: `840334e2507c7b2b39e89917b491e75c1ca3de18` (Root WSI integration, including Component 422 P2 proof invalidation and Root locator fix). Branch `codex/task-workspace-binding-browser`; isolated tree `/workspace/.worktrees/task-workspace-binding-browser`. ACK OWNER-001/002; no authority expansion. Gate remains AB-START-001 / Design v0.2 with real Provider, new spending, production, signing and license decisions closed.

Allowed edits: new `apps/desktop/tests/task-workspace-binding-real-ipc.py`, its new pure boundary test, and this unit's four records. Existing support, Core, schema, Native, TS/API/UI, CI, locks and global STATUS are unchanged. No new management platform or product endpoint.

| Requirement | This slice | Completion boundary |
| --- | --- | --- |
| AB-PRJ-002 | Real registered local Git source / isolated managed worktrees | Directory and explicit Dataset flow not exercised by this browser unit |
| AB-PRJ-003 | Actual managed-root replacement refuses fresh proof; persisted receipt remains history | Reassociation/moving an actual user project not tested |
| AB-PRJ-005 | Two Task-bound worktrees, source dirty/index/HEAD preserved, distinct output | No Session launch; complete default coding admission remains separate |
| AB-SEC-007 | Strict candidate schema16 and actual runtime PRAGMA16 / future manifests refused | This is not genuine15→16 migration, backup or old-consumer validation |
| OWNER-002 | Built actual App, collapsed task Details affordance, desktop/mobile evidence | Not Native WebView/macOS or human/OS Owner confirmation |

Single execution path: actual built App → test-only strict localhost HTTP → fixed authenticated Core16 Unix methods. Fixture Owner calls projects.import/tasks.create outside HTTP solely for two generated tasks and one generated Git source. No Provider/Agent, connections, approval proposals, Grants or sessions.

HTTP fixed reads: system_snapshot, projects_list, connections_list, sessions_list, bounded events_subscribe, fixture-scoped tasks_get and workspaces_resolve. One write: workspaces_prepare_task_git, strict selection/key, server-known source pin, fixture Task/spec/project CAS, local host. The body maps exactly six Core params and original Task CAS/key to envelope. Unknown/Owner/approval/create/session/Agent/generic actions are refused. Same Task recovery requires its exact original body/key, not upgraded CAS or a new key.

Planned runtime sequence (all NOT_RUN at source freeze):

1. WSI-B01: actual UI prepares Task1, real Core ready receipt and filesystem checks; tracked dirty file, untracked file, index bytes, HEAD and empty source directory unchanged. Existing Git worktree metadata may legitimately change.
2. WSI-B02: Task2 Core write genuinely succeeds, then adapter deliberately closes HTTP without a response. UI must show uncertainty, read current Task, and explicitly recover original key/body. All DB rows/events and Task snapshot unchanged by replay; distinct workspaces, committed input only, one Task's generated output absent in the other/source.
3. WSI-B03: first revalidate Task1 successfully. Rename its actual generated worktree to retained sibling, replace original path with empty directory; original revalidation must return INPUT_CHANGED/not_started. Ordinary Task refresh must show stored-only evidence without resurrected filesystem proof (422 P2). Keep history; no SQL forge or user-workspace operation. Desktop and mobile PNGs.
4. WSI-B04: deny representative privileged/generic HTTP actions, verify all relational rows unchanged; schema16 FK clean, zero execution/authority rows. Final decision only after browser close, HTTP handlers/serve thread join, actual own daemon SIGTERM/wait0, original tool/assets hashes and generated-temp cleanup.

Root owns actual serialized execution after independent source review. No compilation, daemon, CLI, actual Git fixture, Node or Chromium was run by this author phase. Pure/metadata checks are independent dimensions and never substitute for these four runtime checks.
