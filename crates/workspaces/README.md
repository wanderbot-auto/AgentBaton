# Workspace operations

This library is a partial WP05 implementation. It does not yet provide the persistent project registry, leases, policy routing, daemon/UI commands, or integration/merge executor. It never deletes a source directory and never stashes or resets user changes.

`inspect` accepts owner-selected existing Git roots or non-Git directories and returns canonical path, device/inode identity, exact HEAD and conservative dirtiness. Unborn/corrupt repositories fail explicitly. Reassociation accepts moved directory identity rather than treating path text as identity; copying across filesystems requires an explicit new mapping. The caller must bind host/project identity in persistent records.

`create_isolated` creates a detached worktree from an exact full commit OID in an owner-selected existing managed root, using a single inert directory name. Dirty source content is preserved. Git hooks and inherited Git selectors are disabled. Configured checkout/clean/process filters cause a fail-closed error before `status` or checkout can execute them. Git output is capped at 4 MiB per stream and execution at 15 seconds. Submodules are not recursively inspected or populated. Any tracked gitlink conservatively prevents adoption until an explicit nested-workspace policy exists.

`check_adoption` verifies exact expected target HEAD and clean state. It does not perform an integration, provide an atomic lock against external Git clients, or imply that a candidate has passed validation. The integration executor must fence its own writers and recheck the target head at the write boundary.

`read_artifact` requires the owner-approved absolute canonical root and a relative path. It opens root and path components through directory file descriptors with `O_NOFOLLOW`, rejects non-regular files, and bounds bytes actually read. Callers must render returned content safely; successful reading grants no script execution or network permission. A symlinked root alias must be resolved and approved before this API is called. Worktrees and these API controls are not an OS sandbox for Provider programs.

Verification: `cargo test --locked -p agentbaton-workspaces` and `cargo clippy --locked -p agentbaton-workspaces --all-targets -- -D warnings`. Tests use temporary local Git repositories with synthetic identities; no remote access, model calls or real user project mutation occurs.
