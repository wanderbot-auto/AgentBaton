# Read-only Goal plan editing context

AB-TSK011 / WP08, AB-START-001 full Designv0.2. Base4798a189910367bf3038e19729c9d2daf2d138b7 (draftPR13, Backend2cad/GoalUI98). Protectedmain6606143 and OWNER001/002 reread/ACK; no new instruction or permission. Root owns this Core DTO; sibling GoalPlanEditor owns UI. No schema/approval/permission/meter change.

Expose GoalScopeView.edit_context as optional plain metadata: max_nodes, writable_prefixes, output_artifacts keyed by artifact ID, rule_executors owner/validator. Derive only from the already native-confirmed stored selection and host policy. This lets the same-model editor select real approved paths/kinds instead of guessing. It is never an ApprovedGoalScope or authorization proof; live request validation stays server-side and unchanged. Missing cached fields deserialize as None so older results remain readable; UI must not invent missing metadata.

Verify original approval projection, legacy cached-result compatibility, revoked immutable metadata without journal mutation, and a client-mutated projection failing to widen the actual approved write set. Independent source review then integrated editor/actualIPC/browser/native and exact CI follow. The current draftGoal9browser proof belongs to2cad, not this untested delta. Fullplanner/dispatch/adoption remains separate dependent work.
