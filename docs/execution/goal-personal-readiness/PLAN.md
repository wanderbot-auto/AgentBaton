# Personal-task Goal readiness

ACK OWNER-001/002 and AB-START-001. Isolated branch `codex/goal-personal-readiness`, base `802273c6b262e96e25192b69996ed40c5243fbe2`; this follow-up is separate from the primary's PR13 CI candidate. Only GoalPlanning.tsx, goal-i18n.ts, one new test file and these four records are owned. No Core/native/API/App/lock/CI, Provider, paid account or permission change.

Core evidence: `storage.rs::create_registered` explicitly permits reserved `project_id="local"` without registry lookup; IPC tasks.create calls it. Registry imports allocate UUID project IDs; `registry.rs::project_from` returns actual NOT_FOUND for missing registered projects. Therefore only the exact reserved personal-project ID gets prerequisite UI. Do not catch NOT_FOUND globally, infer that other IDs are registered, invent scopes/grants, or add RPC.

1. Add negative tests and run them against unchanged product source; retain actual failures.
2. Keep the same lazy Goal disclosure; show bilingual registered-project/root-grant prerequisites for reserved personal tasks without mounting the Goal RPC/write panel. Keep registered-project panel, history, CAS and cached pending recovery byte-for-byte unchanged.
3. Run all original259 plus new tests, typecheck/build, compare old test bytes and scope; freeze source and record actual evidence for the primary's independent review.

Engineering choice: gate panel mounting using Core's explicit reserved `local` semantic. This avoids impossible reads and all Goal controls for personal tasks without interpreting arbitrary server errors as empty results. It is a frontend readiness hint, not an execution authorization rule. Actual Core rules remain authoritative.

Requirement contribution: AB-UX-009/010 progressive and honest capability state; Goal prerequisite readiness. This fix does not establish NL planning, dispatch, real Provider or whole-product completion.
