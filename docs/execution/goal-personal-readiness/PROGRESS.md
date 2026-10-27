# Progress

ACK OWNER-001/002 and AB-START-001. Isolated branch `codex/goal-personal-readiness`, base `802273c6b262e96e25192b69996ed40c5243fbe2`; source freeze `78badec275ec25c84363834ec73c8ad74e65024f`. This is a follow-up branch, not a modification to the primary's PR13 candidate.

Implemented only the exact reserved `task.project_id === 'local'` prerequisite branch in the existing lazy Goal disclosure. It shows an English/Chinese explanation that a registered-project task and approved root grant are required, without mounting the RPC/action panel. It does not reassign the personal task or manufacture a registered project, grant, scope, plan, history, successful response or authority.

Read Core storage `create_registered`, tasks.create dispatch and registry import/lookup before implementation. `local` is a deliberate legacy/personal project exception; imported registry projects have UUID IDs. Other missing projects must still fail honestly. The complete registered GoalPanel function and all 259 existing tests in16 files are byte-identical to base; genuine NOT_FOUND/network errors and exact cached approval recovery remain tested.

Added seven cases. Old product source actually failed four and passed three; repaired targeted seven pass, then full266 in17 files, typecheck and build pass. One initial new-test factory return type did not refine Task's optional project_id; typecheck caught it, the constructor signature was corrected without changing product/API types, and the original failure log remains.

Source is frozen; no further source edit is planned before independent review. Own exact dependency symlink removed. No App/API/Core/native/lock/CI change, Cargo/cache operation, Provider/account, push or merge. Parent has the source/hash/command results and owns independent review, later real browser verification and integration.
