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

- Frozen application source 733fff526f88d3438f7533a0b474ba0474954e4f: actual full Node547/34, selected10/2, type/build/diff all passed. Exact commands/hashes are in EVIDENCE and local verification.json.
- All old32 renderer files, four old actual/failure JSON fixtures, 36 original Native test blocks and byte-limit constants independently byte-checked against baseline; lib/native_parent/Core/CI/locks untouched.
- Standalone rustfmt parse/check passed. Three new actual nonempty-validation callback tests are still uncompiled/unrun, not macOS/OS-dialog evidence.
- Source remains frozen for Root's separate reviewer; only these four records are finalized. No formal GitHub Approve, PR, CI or remote action.

## Subsequent test-only Data / Command revision

- Integrity733 + docs e889 independent review passed in its own detached context; its source, 47-artifact manifest and conclusions remain frozen.
- Added exact real Data33c63615...867cd (119,716 bytes) and Command592ac803...04f89 (94,218 bytes) packets, including real TaskProjection and matching first Owner decisions.
- Initial targeted run actually returned 3 passed / 1 failed: Data guard and readonly history passed; Command graphDetailsShape correctly rejected its original unsafe inode. The direct parsed-JSON Command DOM check was not treated as trusted UI acceptance.
- Python inspection found four original Command observation root_inode fields equal to 9223372036855074700, above JavaScript MAX_SAFE_INTEGER. The original packet, initial test source and failed log are retained. No conversion to string or relaxed guard is used.
- Command592 is now an explicit renderer-unsupported negative, while its original u64 remains a meaningful lossless Native callback test. New Native callback test covers complete original Data/Command snapshots, fixed private frame, corresponding actual first receipt and Cancel/no-rejection.
- Root/Core agreed future Command UI positive must come from an independently produced real private-program-location packet, not edited inode fields. Current revision does not claim that positive or actual Native execution.
