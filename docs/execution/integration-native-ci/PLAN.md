# Integration Native CI plan

AB-VAL-009 partial Native acceptance. Root authorized this isolated implementation from `b22b6d4835ecb86ea52a8e2c194ff28ff6e4b0e2` on branch `codex/integration-native-ci`; the older Native implementation branch remains unchanged. Root approved the plan before code changes. Allowed paths are product.yml, one new infrastructure test, and these four records. This is author implementation, not independent review.

Preserve the existing seven required check lanes (three Environment plus Product Rust/UI/two Mac), their names, actions, contents:read permission, concurrency, matrix, old steps/commands and 25/60-minute job limits. No Core/Rust, Python runner, release scanner, dependencies, locks or other workflow changes.

After the existing Owner binary build and Factory Owner step, add the missing native-smoke/custom-protocol **library test configuration**, then exercise the existing Native integration runner twice using the already built Owner binary and current Core17 normal daemon/companion. First invocation uses the default last-page Cancel; second explicitly uses `--integration-cancel-at 0`. Both existing invocations run Cancel **then full approval** on their own fresh fixture. They are not cancel-only invocations. Only each Cancel phase must leave the proposal/DB unmodified and have no finalize.

Keep original raw fixture/Native reports, snapshots, screenshots and logs in a new canonical RUNNER_TEMP directory with mode0700 and logs0600. An always step uses the existing strict finite projector; publish only its two finite JSON results plus a fixed scalar execution summary into the old public native-smoke artifact. Preserve nonzero or unknown driver results independently of projection success; missing/invalid reports, projection failures or partial execution must fail. Retain the existing always collectors/upload and actual default release audit.

Run source/inverse/YAML/AST and pure mocked-subprocess controls only. Root will assign independent review and actual Mac CI. No Cargo, Actor, Native process, Node, browser, CI trigger, account/spending/host/signing or production action is authorized in this author slice.

## Exact approved-block layer for the legacy workflow guard

Root separately authorized this test-only follow-up from `09414ea041fe9ad6e73b6fdd1784f8e237e387fe` on `codex/native-ci-baseline-guard`. Only test_schema17_migration_workflow.py plus these four existing records may change; product.yml and the original Native CI source remain frozen. This is author implementation, not independent approval.

First validate the three approved ea9 raw step blocks independently: exact names/count1, raw header positions42/43/44 (zero-based), consecutive ordering, original Factory Owner predecessor and native migration-retention successor, and desktop membership. Compare each complete UTF8 block SHA256 before removing exactly that contiguous region. Then execute the original six historical-target argv inverse and unchanged 4b7543 baseline hash. No current/old Git object or network access is required by the test guard. Future unrelated workflow evolution requires an explicit new reviewed layer, not tolerance or token rewriting.
