# ADR — Goal authority and persistence boundary (proposed)

Status: awaiting primary confirmation of public contract before dependent code.

An existing ExecutionGrant approves fixed execution capabilities, not Goal planning, acceptance criteria or a fabricated approved HostPolicy. Native Goal confirmation must approve its own exact server-resolved snapshot and produce a separate recorded authority. Reload reconstructs private Goal scope through verified records; it never deserializes authority tokens. Partial integrity/data/command checks remain partial; owner-approved Goal acceptance criteria are a separate policy and are not a passed result.

Read-only ready preview computes candidates only, never reserves, dispatches, increments attempts, resets shared budgets or accepts a task. Missing historical ledger/effect/consumption facts remain unknown and may block preview. Missing real execution/delegation capabilities are unsupported, not synthetic fields on a cloned grant.

Schema8 is still under active ownership. Schema9 and shared Store/IPC/native-finalizer paths must wait for its exact safe integration boundary. New independent module/fixtures may be prepared after public-contract confirmation.

Confirmed decision: a separate native `goal` proposal shows original grant ID/revision, old/new children/depth, immutable bindings/enforcement and existing shared consumed/active counts. Private finalization CAS-updates only those orchestration ceilings and preserves complete grant revision history atomically with Goal approval/result/journal. Grant approval alone cannot manufacture Goal authority. Existing attempt bindings retain their old revision, and later ceiling changes do not rewrite their historical snapshot. Renderer never constructs ApprovedGoalScope; it submits strict selections/proposals. Propose renders plain proposed policy data and does not construct private authority before confirmation.

Builtin fixed-input SHA checks are evaluated from actual bounded local bytes at preview, after writer/fence guards. Goal OwnerReview defines new exact acceptance criteria and output contracts; it is never a passed result. Existing integrity/data/command definitions are optional additional checks only when their actual consumed owner approval/result and exact contract match. Shared historic runtime/output unknown blocks preview; existing/new unaccounted legacy attempts cannot be filled with zeros or mapped to fictitious Goal node success.
