# Durable Goal runtime

Baseline16924e82f5ee7d3438007a3f297821fb9249d299, isolated branch codex/goal-runtime-core. AB-START-001 and trusted origin/main OWNER001/002 ACK. Scope Core/runtime/session/necessary migration tests and own execution documents; no UI/native/lock/CI/authorization changes, real Provider/account/model/paid calls.

Implement a separate native `goal_execution` proposal bound to exact existing Goal scope, complete fixed plan, ledger sequence, connection/identity/configuration/host/input/workspace and cumulative limits. Earlier `goal` proposal explicitly promised preview-only execution in that version; do not reinterpret it as an unchecked start action. Existing `goal_plans.dispatch/reserve/accept` stay refused. New methods are `goal_runs.propose/get/list/cancel` and authenticated `goal_runs.dispatch` for advancing an already approved run, never for minting authority.

Persist run/node/attempt lineage and root+project+ancestor reservation within the same IMMEDIATE transaction that creates session, attempt, dispatch intent and lease. No reserve-then-session gap. Keep previous shared attempt consumption; unmapped legacy runtime/output stays unknown and blocks unsafe new admission. Unknown token/cost remain unknown and disallow finite unsupported dimensions.

Use actual Supervisor protocol execution, native-cancel/owned kill/reap and current input observations; cancellation is a request until actual stop receipt. A correlated terminal turn yields an execution candidate, never TaskAccepted. Restart prepared/sent work is unknown and never replayed automatically. Candidate dependencies require real bound validation/owner acceptance, not process exit or Agent prose.

Initial runtime authority is restricted to the workspace already named by the root grant and Goal scope. It cannot create a child workspace or expand an identity/recipient/host. Dynamic factory delegation requires a separate native capability and full host-bound immutable factory snapshot; old Goal children/depth ceilings are insufficient. No same-path label/boolean is accepted as fresh Git/Data authority.

Validation: actual generated account-free protocol processes, exact native-finalizer fixtures, normal/error/cancel/timeout/rollback/restart, two roots sharing project budget, unknown meter, input drift and no-authority controls; original workspace and full regression checks remain required. Both source and tests need independent review and later macOS/native/UI CI.
