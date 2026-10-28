# Checkpoint

## Independent review R1 — joint evidence correction

Independent reviewer actually reproduced two UI failures at source8612138: a cancelled run containing outcome_unknown with stop_confirmed=true enabled a new approval; an unsupported future node state without a session enabled adoption. The old pure/API and React failure logs are preserved separately with exact source snapshot attribution, not described as a server authorization bypass.

Fix source **3daaec10dc422fe63d5f391efe51294bf8bd4ada** uses explicit known node states and positive closed evidence. Only not_started may have no attempted session; known candidate/cancelled/failed requires session+attempt+owned stop. Known terminal runs cannot conceal unknown/future/active effects. Honest outcome_unknown remains readable even after owned stop; unsupported future states are labelled unknown and cannot clear critical pending or unlock approval/adoption. A supported cancellation acknowledgement resolves only its request intent, never the effects uncertainty. Normal known candidate/never-started revision adoption remains tested.

Actual final **445 tests/24files, typecheck and build PASS**. The original23 test files/425 cases are byte-identical;20 permanent adversarial/normal controls are added in GoalExecution.evidence.test.tsx. Native approval/lib/harness bytes match861 exactly, so no unchanged Native rerun is claimed. Evidence /workspace/.onboarding/goal-runtime-ui/review-fix/verification.json SHA256 **217a59ab371d51e02327009aca8ea9fad8978cccf7b690b0c116eda721543e92**; commands and independent old failures are bound. The original425/Native28+12/real proposal evidence below remains attributed to861. Await independent final re-review before Root integration.

Previous source checkpoint retained:

Source86121385b8a4e4bb40579b7446b8dab2456a1fec on codex/goal-runtime-ui, baseline822c3bb. All11 owned source files match this Git object. Unit author work complete; final independent review and integration are pending. No remote push/PR/merge.

UI425/type/build and Native28+12/all-targetClippy/fmt/diff actually PASS. Root explicitly received Native target release; no Cargo/Node/probe writer remains. Only own exact Node dependency symlink was removed; ignored Core10 debug sidecars remain clearly compile resources, not Core11/GUI/release proof. Source/manifest/locks preserved.

Evidence directory /workspace/.onboarding/goal-runtime-ui, verification.json SHA256 a1caf8d3f30491fb1054e148e3b379057fa746c33f2b89e60325b3b0e89f813b. Real Core11 normal44b8 and legal largeac4a proposals plus callback reports/controls are under real-proposal/. All logs are persistent workspace files, not /tmp.

Next: independently review exact8612138; compose final Core11 wire and frontend and actually validate real IPC/browser, Native/macOS/default-release and packages. Current UI is mocked-transport validation, Native helpers use callbacks and compiled Core10 dependency; actual OS OwnerDialog and private Core finalize were not run here. Old Goal scope stays planning_only; new execution kind and explicit validated-plan adoption require their own current server checks, not renderer authority. Full130 requirements/remaining factory/NLP/acceptance scope is not declared complete.
