# Progress

2026-10-29T21:08:18.733046+00:00; source `8366b26b71968b826893eb335ae81e5a411323af`, initial UI source `fbcff12644a40e438a264d8efad7064f315de6ef`.

Implemented folded Task Details UI using only taskGet/projectsList/workspacesResolve/workspacesPrepareTaskGit. Expansion/refresh checks actual Task and Project revisions, source Git pin/opaque decimal device+inode, local host and server preparation reference. Existing candidates are explicitly unassigned/non-revalidated. Personal/directory/archived/non-draft states cannot prepare a new workspace; source edits remain in place. No credentials, permissions, remote host or generic execution controls were added.

Explicit prepare submits the full server-derived pin and task/spec/project CAS with a stable key. Unknown outcomes retain original bytes/values/key across same-document remounts, require successful current Task reconciliation, and expose only explicit same-key recovery. Prior uncertainty survives a later not-started refusal. Original receipt/current binding/filesystem observation are independently displayed; fresh identity observations include their time and never imply execution authority. Pending/unknown bindings remain readable. Existing persisted receipts can explicitly revalidate through their original key, without rebuilding original CAS from the newer Task revision.

Async read epochs and local parent identity invalidate stale/foreign reads/actions; immutable receipt and task/binding revision floors reject rollback/substitution. Local intent also binds project identity even when numeric revisions coincide. Capacity is bounded32 and unresolved records are not evicted. Reads do not auto-submit.

Static follow-up8366: fixed Core resolver explicitly refuses archived projects. Archived history therefore reads only taskGet/projectsList and skips the creation resolver; it is not a fabricated preparation or permission exception. Original23 test specs were not run; one new blocked-unknown-task spec brings the source matrix to24. No actual test failure is being rewritten.

Light Python source assertions and git diffcheck0 verify allowed5files, exactly4fixed APIs, no generic bridge/numeric FS coercion, App inverse byte-equal, CSS prior prefix and protected Core/wire/Projects/DataWorkspaces bytes unchanged. These are not TypeScript parser/type or execution proof.

Typecheck,24 mock component cases, full existing test suite, build, actual IPC/browser/Native remain NOT_RUN. Independent review and Root actual tests are next; no product execution or cleanup result claimed.
