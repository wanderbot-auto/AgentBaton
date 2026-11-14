# Root integration handoff

Tree `/tmp/workspace-open-core-e874`, branch `codex/workspace-open-core`, base `e874b123d3e7f329a551e21922baf1a7a643548f`. New files only: workspace_open.rs, workspace_open_tests.rs, workspace_open_ipc.rs and these private-unit docs.

Private Root hook patch: `/tmp/workspace-open-core-hooks-e874.patch`. It registers the private Core module, exports `pin_open_context`, `WorkspaceDirectoryIdentity`, `WorkspaceOpenSelection`, `WorkspaceOpenRequest`, `WorkspaceOpenContext`, `WorkspaceOpenDirectoryPin`, adds the strict Owner read arm, and invokes the method-only raw validator after authenticated Owner selection. It does not alter the Factory companion whitelist. The patch has not been applied by this author. Hash and applicability evidence are recorded in EVIDENCE.json.

Desktop agreed on canonical_path:String, selection equality, directory_identity:Vec of decimal-string fields, CoreError error codes, and pin_open_context/recheck. Root must compose the shared hooks before any new module/test can compile. Next: independent source review, Core workspace_open unit tests and real workspace_open_ipc target, clippy/format and applicable existing regressions; then Desktop real bridge and exact dual-Mac handoff evidence. All runtime/native outcomes remain NOT_RUN here. Do not advance the requirement or formal Approve from this author checkpoint.
