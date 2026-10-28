# Progress

- Created isolated worktree from exact integration HEAD 87b4434; existing integration changes were untouched.
- Rehashed and copied original actual Integrity bundle 7cc70a97cac37d97967a292e519e5d6a5eaecf2267eec9a449d8f0a43e39a0f6, 71,707 bytes.
- Unchanged actual supported/reference/consistency check passed (1/1). Existing guards are compatible with this positive; no positive failure is fabricated.
- New missing/null/unsafe-integer observation negative controls actually failed against baseline: 1 positive passed / 3 negative controls failed. Original log preserved at .onboarding/goal-validation-observation-ui/baseline-missing-observation-actual.log.
- Core confirmed result_digest hashes the complete persisted observation (library view plus goal_io) using compact serde_json Value. Actual TaskProjection requested from Core; original packet has no task.
- Implementation in progress. No Cargo or macOS execution.

- Added complete observation DTO plus safe JSON projection guard, keeping finite fractional numbers and original wire values. No history component change was necessary because it already renders complete folded JSON.
- Added three Native callback tests: actual nonempty validation proposal/observation roundtrip; every page/final cancellation for accept/reject with zero decision writes; missing observation/goal_io and unjoined-pass rejection before any prompt.
- Minimal Native guards require the complete observation object/goal_io namespace and passed => owned stop + worker_joined. Old private actions/kinds/CAS/ref binding and byte limits are unchanged.
- Core supplied actual same-run TaskProjection packet 74c44f... (73,333 bytes), then final immutable packet 5821176997487a594416b740e7e40538ae7d1dfa1c9e133a1531a5e4e093a0d2 (73,731 bytes), including the corresponding first Owner decision. Portable final fixture uses these exact latest bytes, with original 7cc packet retained separately.
- Selected tests passed 10/2 and typecheck passed before the final packet refresh; final full verification follows at the frozen source commit. Earlier type failures (missing Node type definitions and readonly assignment) are preserved; tests use the existing standard WebCrypto API instead and no dependencies/tsconfig/locks were changed.
