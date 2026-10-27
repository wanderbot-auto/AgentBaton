# Same-model Goal plan editor

ACK OWNER-001 / OWNER-002, read at approved base `d30bb9e` from the primary's Goal integration. This independent worktree is `/workspace/.worktrees/goal-plan-editor`, branch `codex/goal-plan-editor`.

Requirement: AB-TSK-011 and `docs/06-orchestration.md` section 6. Add real editing of the existing `GoalPlanProposal` nodes, dependency/data/lineage edges, approved executor/rule selection, fixed input/output contracts and bounded budgets. Keep the existing versioned proposal API and advanced original JSON workflow. No second DSL, new execution/approval method, planner simulation or dispatch.

1. Read current execution authority, OWNER inbox, Quiet Canvas amendment, public Goal DTO and Core graph semantics. Identify missing public editing metadata; coordinate a readonly backend projection rather than infer writable paths or Owner/validator rule kinds.
2. Add folded `GoalPlanEditor`, mounted only inside the existing task Details plan section. Require explicit preparation/load, local edits and apply. Applying changes JSON draft text only; the existing separate store button owns CAS/idempotency/unknown handling.
3. Enforce current source revision/digest/raw-draft/context conflict checks. Hold existing acceptance rules and execution binding fixed. Select only original approved inputs, outputs, rules and paths; validate DAG, output ownership and budgets without representing local validation as authority.
4. Keep older daemons safe: absent/null/invalid `edit_context` disables structured changes and leaves advanced JSON/history available. Implement the optional DTO in TypeScript only; primary owns backend dependency/integration.
5. Exercise normal edits and semantic failures, original-byte preservation, versions/concurrent changes, metadata refusal, keyboard form controls and real Goal API boundary through mocked transport. Run unchanged original 229 tests plus new tests/types/build. Freeze local source for independent review and primary's subsequent real backend/Chromium checks.

No Core/native/App/locks/old tests/harness/CI/global status changes, new dependencies, Cargo, agents, push or PR in this unit. Entire-product completion, generated NL planning, scheduling, conditional workflow/runtime routing and actual Goal execution are outside this slice and remain tracked by the primary.

Independent review F1 follow-up: reproduce the numeric draft collision on unchanged source first; replace delimiter/prefix node keys with exact node identity storage, preserving valid IDs and other nodes' text. Keep the original failing evidence and all prior assertions, run the full suite/types/build, and freeze a separately reviewable source commit. No authority or backend dependency change belongs to this fix.
