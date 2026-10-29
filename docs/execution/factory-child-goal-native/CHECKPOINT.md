# Checkpoint

Frozen source d250d3bd20d8295f238974c4c2f64564ffbdd735 on codex/factory-child-goal-native, worktree /workspace/.worktrees/factory-child-goal-native. Exact Core contract2a3, publicDTO files copied/hashed in evidence. This leaf implements Native wiring only; Core15 migration/handoff/IO/shared ledger/Graph dispatch are separate WIP, not verified by static source or Native14/PR16 results.

Fixed commands:

- factory_child_goals_start{scopeId,request,expectedRevision,idempotencyKey}
- factory_child_goals_get{scopeId,taskId,goalRunId:null-or-ID}
- factory_child_goals_list{scopeId,taskId}
- factory_child_goals_cancel{scopeId,taskId,goalRunId,expectedGoalRunRevision,expectedRevision,idempotencyKey}
- factory_child_goals_advance_plan{scopeId,goalRunId,request,expectedGoalRunRevision,expectedRevision,idempotencyKey}

Inner request remains{task_id,plan:fullFactoryChildPlanRef,expected_factory_run_revision}. Scope CAS/key stay in the Core envelope, Goal-run CAS stays distinct. No method/HTTP/token/finalize selector. Existing goal_node_approve/reject handles both verified issuer categories through Core; a Factory display flag alone cannot authorize a legacy conversion or TaskAccepted.

Integration: cherry-pick two source deltas onto Root's current combined source; retain laterc0a shared review-limit fix/feature view and other Native14 fixes. Do not restore this older whole lib.rs over Root. This branch intentionally keeps existing strict schema14 test fixtures unchanged; Root must separately coordinate schema15 adaptation and actual current binaries before claiming product/native acceptance. No broad schema acceptance or skipped checks is permitted.

Next: independent fixed-source review, actual Rust/type/Clippy and normal/error tests after a Root lease, real schema15 Core/IPC and UI integration, ARM/Intel macOS Native verification and exact CI/artifact binding. Current source markers, mock JSON examples and static contracts are not those results. No push, PR, merge, approval, provider credential, spend, production release or package is claimed here.
