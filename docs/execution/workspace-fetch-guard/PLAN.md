# Workspace object-fetch execution guard

Base protected main dc5aea00013af0bf0fdae41d09c83ed0c4dacfd7; branch codex/workspace-fetch-guard in isolated worktree. AB-START-001 authorizes this work. OWNER001/002 re-read and ACK: full goal remains, UI amendment untouched. No Goal/task URL supplied by this entry.

Requirement AB-PRJ-001: importing owner-selected projects must not execute their contents. Related AB-PRJ-002/004: preserve workspace identity and source edits; this security increment does not complete those requirements. Config implementer and independent reviewer reproduced controlled missing-object promisor Git invoking a generated local ext helper. Reuse that focused guard/test design; no second adoption/validation framework.

1. Apply minimal GIT_NO_LAZY_FETCH=1, protocol.allow=never, fail-closed partialclone/promisor config guard before status/object work and checkout. Keep public types/API and existing filter/path guards.
2. Exercise generated missing-blob/ext-helper cases; no network/private identity/source execution. Verify unchanged config/index/content and no managed checkout on refusal. Run full workspace regression, fmt and Clippy.
3. Preserve source/log hashes and exact outcomes for independent review. Only workspaces lib/tests and this unit docs may change; no Cargo/locks/DB/UI, pushes or main merges. Parent integrates the reviewed focused commit into dependent branches.


Owner integration lead added the same read-boundary raw-object requirement after independent refs/replace reproduction: disable replacement objects and legacy grafts in Git command env, and verify committed bytes/parents with generated fixtures. This closes exact-SHA interpretation, not a new adoption engine.
