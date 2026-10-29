# Progress

2026-10-29T21:08:18.733046+00:00; source `8366b26b71968b826893eb335ae81e5a411323af`, initial UI source `fbcff12644a40e438a264d8efad7064f315de6ef`.

Implemented folded Task Details UI using only taskGet/projectsList/workspacesResolve/workspacesPrepareTaskGit. Expansion/refresh checks actual Task and Project revisions, source Git pin/opaque decimal device+inode, local host and server preparation reference. Existing candidates are explicitly unassigned/non-revalidated. Personal/directory/archived/non-draft states cannot prepare a new workspace; source edits remain in place. No credentials, permissions, remote host or generic execution controls were added.

Explicit prepare submits the full server-derived pin and task/spec/project CAS with a stable key. Unknown outcomes retain original bytes/values/key across same-document remounts, require successful current Task reconciliation, and expose only explicit same-key recovery. Prior uncertainty survives a later not-started refusal. Original receipt/current binding/filesystem observation are independently displayed; fresh identity observations include their time and never imply execution authority. Pending/unknown bindings remain readable. Existing persisted receipts can explicitly revalidate through their original key, without rebuilding original CAS from the newer Task revision.

Async read epochs and local parent identity invalidate stale/foreign reads/actions; immutable receipt and task/binding revision floors reject rollback/substitution. Local intent also binds project identity even when numeric revisions coincide. Capacity is bounded32 and unresolved records are not evicted. Reads do not auto-submit.

Static follow-up8366: fixed Core resolver explicitly refuses archived projects. Archived history therefore reads only taskGet/projectsList and skips the creation resolver; it is not a fabricated preparation or permission exception. Original23 test specs were not run; one new blocked-unknown-task spec brings the source matrix to24. No actual test failure is being rewritten.

Light Python source assertions and git diffcheck0 verify allowed5files, exactly4fixed APIs, no generic bridge/numeric FS coercion, App inverse byte-equal, CSS prior prefix and protected Core/wire/Projects/DataWorkspaces bytes unchanged. These are not TypeScript parser/type or execution proof.

Typecheck,24 mock component cases, full existing test suite, build, actual IPC/browser/Native remain NOT_RUN. Independent review and Root actual tests are next; no product execution or cleanup result claimed.


## Review follow-up 422dd195c83c7f5b28e1a967a5dec00f3c243f30

Independent source review identified a P2 in the earlier8366/fbc path: after a successful FS identity observation, same-key revalidation could return INPUT_CHANGED/not_started while stored Task binding remained identical. Catch retained saved.observed; later ordinary TaskGet/Projects refresh could display it as revalidated again. This is a source-reachable invalidated-proof issue, not a claimed executed component/browser failure. Original source and sealed phase evidence are retained.

Fix only invalidates saved.observed plus mounted displayed observation in perform.catch. It does not change pending keys, full original payload, receipt, CAS, uncertain recovery or Core behavior. Added1 exact same-key success→FS failure→Task-only refresh remains stored-only→new explicit successful identity check restores proof regression. Original24test specs remain byte unchanged; total25 declared, all NOT_RUN. Python inverse/source checks and diff0 only.

Archived-history confusion in review used old fbc:8366 already avoids resolver on archived projects. Active-project cancelled/unknown/accepted records remain subject to real resolver's non-draft blocked result, and new prepare also requires Task.state=draft. No test-shaped permission bypass or global Unknown exception added. Root owns Node/type/build/browser validation.
