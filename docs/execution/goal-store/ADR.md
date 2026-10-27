# ADR — Goal authority, persistence and raw evidence

Status: implemented under primary-approved public contract; independent Backend review passed R1/R2 on source 1a3236a; later H1/H2 correction source 2cad05b passed authored and independent 210 unique Core / 48+30+3 targeted / Clippy / fmt / diff, with no remaining blocking finding.

An existing ExecutionGrant is not Goal approval. Separate native kindgoal binds a complete server-resolved snapshot and authorizes only displayed children/depth changes via same-root CAS. Complete old/new history is retained; existing attempts keep their revisions. Other permissions/bindings/ceilings and shared counts stay intact. Private authority is reconstructed only after actual approval/result/event checks, never from Deserialize or Agent labels. Partial validation is not Goal/Task acceptance.

Schema 9 follows safely merged Core 8 and preserves private pre-v9 plus historical pre7/pre8 backup semantics. Grant CAS/history, actual budget reconciliation, scope/result and events share one IMMEDIATE transaction. Ready preview creates no reservation/attempt/dispatch/acceptance. Opaque Goal evidence/action history requires future actual scheduler reconciliation and cannot be cleared by new keys/revisions.

Fixed input hashes are checked from actual bounded bytes after writer/fence guards. Optional validator references require actual native-confirmed versions and exact full contracts. They grant no new action rights: consumed one-shot Command policies reject new use; owner criteria remain separate new rules.

Plan wire is scope_id+original plan_json string. Direct strict typed decoding under 256KiB preserves duplicate-key rejection; exact original text defines replay identity and typed canonical serialization defines revision digest. Historical committed replay does not resurrect revoked permission.

The native 16KiB cap remains unchanged. Full immutable grant fields appear once with explicit before/after ceiling/revision references that reconstruct exact grants. Full approved_contract_json is lossless, not digest-only/truncated. Full private-record digests plus source re-resolution bind metadata. Actual native dialog/manual readability is unrun.

Actual execution budget rows influence prepare_attempt, so preview-only maxima are insufficient. Missing rows inherit recovered consumed/active facts, existing counts move upward only, and established total/parallel ceilings stay exact. Reconciliation events and counts roll back atomically on approval failure. Runtime/output/token/cost history remains unknown when unmeasured.

Existing oversize meter/idempotency rows are never absence or zero-use evidence. One SQL CASE checks octet_length before materializing bounded text, distinguishing missing rows from oversized ones without a second-read race. Regressions cover both; no cap/assertion/timeout/permission was weakened.

## Public IPC contract for integration

Authoritative types: crates/core/src/goal_store.rs:135-308; routing: crates/core/src/ipc.rs:484-501. Methods use the existing private Unix Request envelope. Write keys must be nonempty ASCII alphanumeric/._:-, at most 128 bytes. All listed selections/submissions deny unknown fields; inner plan_json is parsed directly with strict nested DTOs, never JSON.parse/re-serialized first. The public Rust typed convenience submission is for trusted Core callers, not renderer wire.

| Method | params | Envelope requirements | Result/authority |
| --- | --- | --- | --- |
| goal_scopes.propose | GoalScopeSelection below | idempotency_key | ApprovalProposal kind=goal; no approval yet |
| goal_scopes.list | {task_id} | none | GoalScopeView[] |
| goal_scopes.get | {scope_id} | none | Historical GoalScopeView; not a dispatch grant |
| goal_scopes.revoke | {scope_id} | idempotency_key, expected_revision=current scope row revision | Revoked GoalScopeView; does not claim running work stopped |
| goal_plans.propose / goal_plans.revise | {scope_id,plan_json:string} | idempotency_key, expected_revision=plan expected_revision | GoalPlanView; initial compare0/revision1, revisions compare previous/revision+1 |
| goal_plans.get / goal_plans.list | {scope_id} | none | Historical latest GoalPlanView / immutable revision views |
| goal_plans.ready_preview | {scope_id} | expected_revision=current plan revision | GoalReadyPreview, strictly no reservation/dispatch |
| goal_scopes.approve / goal_plans.dispatch / reserve / accept | none supported | always rejected AUTH_REQUIRED | No renderer consent/execution path |

Native integration uses the existing private approvals.finalize handler with proposal id/revision, only after independently checked kindgoal and real native confirmation. It must not expose finalize to renderer. New selection+key resolves current records; exact same selection+key may return the old proposal to recover a lost response even after grant CAS. Finalize itself rechecks exact snapshot/counters/source policies, or returns its committed result. Same key with different selection/raw text conflicts. Oversize stored replay entries reject RESOURCE_LIMIT, not a fresh request.

```text
GoalScopeSelection = {
 task_id, expected_task_revision, grant_id, expected_grant_revision,
 max_nodes, new_max_children, new_max_depth,
 root_budget:BudgetLimits, project_budget:BudgetLimits,
 inputs:ArtifactRequirement[],
 outputs:{artifact:ArtifactRequirement,validators:GoalValidatorRef[]}[],
 writable_prefixes:string[], owner_acceptance_criteria:string
}
ArtifactRequirement = {artifact_id,relative_path,max_bytes,expected_sha256:string|null}
GoalValidatorRef = {kind:integrity|data|command,policy_id,revision}
BudgetLimits = {attempts,parallel,runtime_ms,output_bytes,children,reworks,
                tokens:number|null,cost_microunits:number|null}
GoalBinding = {project_id,root_task_id,task_id,spec_revision,input_revision}
GoalScopeView = {
 id,revision,revoked,task_id,project_id,root_grant_id,root_grant_revision,
 authority_revision,scope_digest,goal:GoalBinding,
 root_budget:BudgetLimits,project_budget:BudgetLimits,
 outputs:Map<artifactId,contractId>, required_acceptance_rules:string[],
 contracts:Map<contractId,requiredValidatorId[]>,
 inputs:Map<artifactId,{contract_id,sha256}>, binding:ExecutionBinding,
 program_sha256,expires_at_ms,rules:Map<ruleId,digest>,workspace_id,
 input_revision,planning_only:true
}
GoalPlanView = {scope_id,proposal:GoalPlanProposal,digest,created_at,
                state:validated_proposal,dispatched:false}
GoalReadyPreview = {
 scope_id,plan_id,plan_revision,journal_sequence,read_only:true,
 reservation_made:false,dispatched:false,unknown_dimensions:string[],
 blocked_reasons:string[],decision:ReadyDecision|null,
 budget_usage:{root:GoalBudgetUsageView,project:GoalBudgetUsageView}
}
GoalBudgetUsageView = {attempts,active,children,runtime_ms:number|null,
 output_bytes:number|null,tokens:number|null,cost_microunits:number|null}
```

Scope row revision starts1 and changes on revoke. It is distinct from authority_revision (the new root grant revision, typically2); plan.scope_revision must use authority_revision. Plan goal/spec/input, executor bindings/program, contracts, acceptance rules, lineage and budgets follow the reviewed goal_plan.rs strict public types. No Agent policy/receipt authority is accepted. UI numeric values must be safe integers before encoding; Core u32/u64/i64 conversions remain checked.

Bounds: raw inner plan256KiB; full native snapshot16KiB; selections at most16 inputs/16 outputs, criterion2048 bytes, max256 nodes, scope listings bounded at 256 per task and plan histories at 256 revisions, stored documents1MiB. A real full snapshot may still exceed cap with many/long contracts; rejection is explicit, not truncation. Current reliable enforcement does not support requested token/cost caps: these must be null. Ready missing historical metering or retained unmapped runtime/action evidence returns no decision; unknown values remain null. Ready has no counter mutation/reservation. A ReadyDecision's own requires_atomic_store_commit=true is a future runtime precondition, not proof such a commit happened.

Root/project budget usage is readonly live usage. Original baseline counts and unchanged grant bindings appear in native snapshot. Approval only authorizes explicit delegation ceilings on the same root; it does not reissue an old consumed command policy or adopt a historical partial validation result. Final Goal scheduling, generated planner and settlement remain subsequent interfaces.

## Historical plan data is not current authority

A fresh native-approved scope with genuinely no head, revisions, proposals or committed plan journal lists as [], while get remains NOT_FOUND. Orphaned or oversized history rejects and never masquerades as empty. get/list return immutable typed historical data after validating original native scope/result and full saved bindings, immutable grant revision, scope-bound canonical hashes, contiguous previous-digest/revision chain, head, original raw/typed proposal and actual goal_core validated journal. They never construct an ApprovedGoalScope using old timestamps or read current input files. Revocation, expiry, later input/spec/policy changes do not erase history. New revisions and ready_preview continue through unchanged live scope, writer/fence/input/source-policy/one-shot checks and cannot gain authority from a historical DTO. Integrity/Command denial assertions are retained on those real new-action methods; historical reads are now tested positively.
