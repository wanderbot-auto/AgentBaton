# Factory child Goal execution wire contract v1

Source schema15 / factory.rs. This contract is for real implementation, not a runtime success claim. Existing scopes omit optional fields and retain their original four tools (planning scopes five). New execution policy is separately fully Native-reviewed; create_workspace remains false. No implicit Grant revision/ceiling change.

## Native scope review

Existing fixed Factory approval kind `factory` shows full `FactoryScopeSelection` and `FactoryScopeBindings`: both separate planning and execution policies, all selected input SHA/contracts/outputs/criteria, complete validator definitions and their reference/digest/allowed action provenance, original root task/project, real Grant ID/revision/scope/children/depth/attempt/TTL/identity/connection/workspace, complete execution binding and planning-policy digest, requested/effective/enforced distinctions. `child_goal_execution_policy` fields are max_nodes (≤256 and original Grant.children+1), max_runs_per_task exactly1, max_total_runs, root_budget, project_budget, require_owner_acceptance=true. The existing Native complete-envelope bound remains1MiB, never truncates. These fields do not approve any actual candidate.

## Five fixed Owner bridges

Tauri command names map exactly to corresponding Core methods below. No generic JSON method selector or private finalize bridge is added.

| Tauri | Core method | params | envelope | result |
| --- | --- | --- | --- | --- |
| factory_child_goals_start | factory_child_goals.start | FactoryChildGoalStart `{scope_id,request:{task_id,plan:FactoryChildPlanRef,expected_factory_run_revision}}` | expected_revision=Factory scope CAS + idempotency_key | FactoryChildGoalSummary |
| factory_child_goals_get | factory_child_goals.get | FactoryChildGoalQuery `{scope_id,task_id,goal_run_id:null-or-ID}` | neither CAS nor key | FactoryChildGoalView `{summary,origin:FactoryChildGoalOriginView,details:GraphRunDetails}` |
| factory_child_goals_list | factory_child_goals.list | FactoryChildGoalQuery; goal_run_id must null | neither CAS nor key | Vec<FactoryChildGoalSummary>, complete ascending creation order, no rawplan duplication |
| factory_child_goals_cancel | factory_child_goals.cancel | FactoryChildGoalCancel `{scope_id,task_id,goal_run_id,expected_goal_run_revision}` | expected_revision=Factory scope CAS + idempotency_key | FactoryChildGoalSummary |
| factory_child_goals_advance_plan | factory_child_goals.advance_plan | FactoryChildGoalAdvance `{scope_id,goal_run_id,request:{task_id,plan,expected_factory_run_revision},expected_goal_run_revision}` | expected_revision=Factory scope CAS + idempotency_key | FactoryChildGoalSummary |

A Summary is exact factory authority_kind/factory_scope_id/factory_run_id/factory_root_task_id/parent_task_id/task_id/saved-plan-ref/execution_policy_digest plus `goal:GraphRunSummary`. Get additionally returns full existing Graph details: plan nodes/version, actual sessions/jobs/attempts/stop/unknown/measurement, original candidate receipts and exact SHA/root/input/attempt origin, lossless validation observation, private Owner decisions, durable Task acceptance. UI must expose complete selected original plan through the existing fixed child-plan get and preserve its raw text/reference; summary alone is not execution evidence.

Start/adoption must compare the full immutable saved reference, current saved head, same target/Factory run, original input version, run/scope CAS and fresh capability/read witness. Same exact key recovers the original action receipt; changed payload conflicts; no new action may use old receipt as authority. Cancel request is not actual stop. Get/list after expiry/revoke/terminal preserve authenticated history without authorizing new I/O/dispatch. Unknown effects/no join never become zero/accepted or automatically replay.

## Fixed Agent tools

New execution scopes expose `execute_child_goal` with only FactoryChildGoalSubmission `{task_id,plan,expected_factory_run_revision}` and `advance_child_goal` with only FactoryChildGoalAdvanceSubmission `{goal_run_id,request:{task_id,plan,expected_factory_run_revision},expected_goal_run_revision}`. Core derives scope/caller session/attempt/fence/lease/family ancestry; no caller/root/grant/authority flags are accepted. Existing `cancel_task` is the scoped cancellation path, and must propagate to related child Graph workers while preserving their actual owned-stop state. Existing status/result return child-goal summary when present; full Graph inspection/Owner approval is an Owner UI operation. Default false retains four tools, planning-only retains five, execution explicitly adds these two to the five. No generic RPC/finalize/approval tool.

## Owner acceptance

Reuse existing fixed Native Tauri `goal_node_approve` / `goal_node_reject`, Core `goal_node_approvals.propose`, private exact `goal_node_acceptance` finalize/decline. No new direct Owner decision route. Core must classify and verify typed issuer before snapshot/propose/finalize/replay. `GraphRunBinding` gains optional omitted-None `factory_origin:FactoryChildGoalOriginView`; Native old snapshots retain original bytes. For a derived run the complete view includes real original Factory approval ID/scope digest, original root Grant/revision and execution binding/policies, full parent/ancestor lineage, initial saved raw-plan receipt/ref. Existing snapshot also includes exact run/plan/input/workspace/epoch, criterion text/digest, candidate artifacts+origin receipts, prior validation receipts and expected ledger/run CAS. Native must structurally preserve/fully display these fields; it cannot synthesize a derived capability. No approval outside this exact candidate means TaskAccepted.

`GraphAuthority` itself is a private enum-backed Core value, not a wire authorization flag. Display authority_kind is `factory_child_goal_execution_v1`; it never grants authority by itself. Legacy Native Goal/Graph and Factory-derived mutations use their corresponding issuer/provenance validators. Generic legacy scope/run APIs cannot convert a derived run into new Native consent. The existing read-only Graph detail/Owner-node operations can cover both only after the typed issuer check.

## Frozen Tauri top-level argument spelling

The renderer invokes the five fixed commands with these exact camelCase top-level objects; inner DTO field names remain their original snake_case (no JSON key renaming/normalization):

- factory_child_goals_start `{scopeId, request:FactoryChildGoalSubmission, expectedRevision, idempotencyKey}`; Native constructs Core params `{scope_id:scopeId,request}`, and puts `expected_revision:expectedRevision,idempotency_key:idempotencyKey` in the Core envelope.
- factory_child_goals_get `{scopeId,taskId,goalRunId:null-or-string}`; Core params `{scope_id,task_id,goal_run_id}`, no CAS/key.
- factory_child_goals_list `{scopeId,taskId}`; Core params `{scope_id,task_id,goal_run_id:null}`, no CAS/key.
- factory_child_goals_cancel `{scopeId,taskId,goalRunId,expectedGoalRunRevision,expectedRevision,idempotencyKey}`; Core params `{scope_id,task_id,goal_run_id,expected_goal_run_revision}`, scope CAS/key in envelope.
- factory_child_goals_advance_plan `{scopeId,goalRunId,request:FactoryChildGoalSubmission,expectedGoalRunRevision,expectedRevision,idempotencyKey}`; Core params `{scope_id,goal_run_id,request,expected_goal_run_revision}`, scope CAS/key in envelope.

Get now includes mandatory `origin`, generated by the same verified Core origin projection as `GraphRunBinding.factory_origin` for Node Owner snapshots. It is never assembled from UI summary/scope. `origin.initial_plan` remains immutable; `summary.plan` is the current explicitly adopted saved reference. Both must be shown distinctly when different. The original planning binding/context `goal_execution_authorized` remains false even when a separate execution policy is approved: a planning record itself never grants execution. The execution binding has its own authority_kind/policy/provenance; UI reads that actual separate capability.

## Conditional reworks (same DTO)

Old false/planning-only scopes still require root/project reworks0. New complete execution scopes may explicitly approve bounded reworks: selection.root_budget.reworks==execution_policy.root_budget.reworks; same equality for project; each≤its attempts−1, root≤planning.max_revisions−1 and root≤project. Actual shared and ancestor counters are checked atomically, not reset by a planner recovery or plan revision. New Goal workers gain no Factory MCP handle. Only the original existing Factory planner recovers as a new attempt after all failed Goal workers have confirmed owned stop; original config/identity/Grant/TTL/budgets/input lineage remain fixed. Missing stop/unknown refuses automatic recovery.
