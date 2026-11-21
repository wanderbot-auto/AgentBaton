# PR #33 macOS packaging split

Base `6308a3d4d3bc97727eba641a463d86ad2d21c75d`, branch `collab/pr33-ci-package-partition`. The previous hosted attempt passed Core/migrations and actual Native, built and audited the default app, then reached GitHub's 30-minute cap during DMG packaging. Preserve those receipts.

- Keep the Core job and its 60-minute limit, and the Native job and its 30-minute limit. Run default application build, release audits and development packaging under the existing required check name in a fresh 30-minute ARM64 job.
- Bind all three jobs to the same repository, event, actual checkout, PR parents/tree, current run/attempt and actual upload ID/digest. The required job explicitly fails for an unsuccessful dependency before checkout.
- Keep the original tests, pins, permission scopes and diagnostic retention. Restore the new workflow to the exact prior split bytes in test fixtures, then apply the existing exact inverse chain. Exercise the actual new handoff code, hostile input rejection and the full infrastructure suite.
- Deliver a local scope-checked commit. Integration, pushing and GitHub runs remain with the leader.
