# Independent review — same-inspection Git batching

**Verdict: no blocking issue found in frozen source `cb82ac6d1f8072e0b2fc18a90e7e13572fb6d8bb` (author metadata `ff8598bdc543a21dc34a6b876928edcc57181a0b`).** Linux safety/regression checks pass; this is not evidence that the original 1000 ms macOS path passes. A new exact-head dual-architecture CI run remains mandatory. Reviewer `/root/review` modified only this report and ignored control/evidence files, with no source/lock/index/push/merge operation.

## Scope and safety review

Base is exact ada4/schema8. Production diff is only `crates/workspaces/src/lib.rs`; Core sessions/policy/registry/IPC/storage, permissions, workflows, manifests/locks and original 1000 ms / 1 s / 2 s / 3 s assertions are unchanged. The new Core integration test and new workspace tests add evidence, not production control flags.

- Same-inspection config checks use Git's no-includes NUL-record list before metadata/object/status operations. It rejects include/conditional-include, partial-clone/promisor and filter declarations, retaining fail-closed malformed/capture errors. Parsing only the first LF preserves multiline values as data. Actual Git 2.52 controls confirm section/variable case normalization, case-sensitive/dotted subsections, and rejection of LF keys through both CLI and manually malformed config. Include and filter execution remains refused before status.
- The hardened Git launcher is unchanged: inherited GIT selectors removed; global/system config, hooks/fsmonitor, attributes/submodule recursion, lazy fetch, replace/graft and protocol controls retain their original restrictions. No new external helper, network, hook or arbitrary command is enabled.
- One rev-parse obtains top-level/common-dir/verified commit only when its fixed three-line framing is unambiguous; internal LF paths use the original independent path queries. Selected canonical root equality remains checked. This is not an atomic filesystem snapshot and creates no durable or cross-request authority cache.
- Combined `ls-files --cached --stage -v -z` preserves lowercase assume-unchanged rejection, skip-worktree/sparse rejection and mode-based gitlink detection before status. Path bytes do not become flags/modes. Submodules remain conservatively dirty and status ignores their contents. Git still parses sparse booleans/duplicate last-value semantics itself; no approximate boolean parser replaces it.
- Existing direct-child 15-second and per-stream raw 4 MiB implementation is byte-unchanged. New real boundary tests pass for stdout, stderr, oversized config and the owned PID timeout/reap. This does not add a general escaped-descendant containment guarantee.
- Both reservation and launch independently perform a complete inspection. Actual post-reservation tracked-input mutation returns INPUT_CHANGED before the generated process starts, retains one Prepared attempt and Draft task; restoring approved bytes allows that same reserved attempt to launch and be reaped, ending TaskOutcomeUnknown without acceptance. No persisted transaction or final drift guard was removed.

## Independent execution

Explicitly coordinated exclusive existing target, Linux umask 022; CARGO_HOME/RUSTUP_HOME existing toolchains, CARGO_TARGET_DIR `/workspace/.worktrees/validation-integration/target`, CARGO_INCREMENTAL=0, CARGO_BUILD_JOBS=2. Target was formally released to Primary before further static controls/report writing; no new target/cache deletion.

- `cargo test --locked -p agentbaton-core -p agentbaton-workspaces --offline`: **Core 130 + workspaces 97 = 227 unique top-level tests passed**, plus **4 compile-fail docs**. One nested preexisting FIFO child emits an extra passing harness result and is excluded from the unique count. Includes all original Core command 14, migrations, session/IPC/lease/adoption/unsafe repository guards, new framing tests and real 15.01 s owned-command boundary.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: **passed**.
- `cargo fmt --all -- --check`; `git diff --check ada4c81..HEAD`: **passed**.
- Repeated the one new profile test with `--nocapture --test-threads=1` using an independently generated transparent PATH wrapper that logs args then execs the original `/usr/local/bin/git`, without fake output/sleeps. Actual readonly/reservation/drifted-launch/launch counts are **5 each**, elapsed **53531 / 54476 / 53611 / 59520 microseconds**. This repeat is not counted as a new unique test.
- Independently rehashed all **8** final source/log entries in author EVIDENCE; all match. Saved successful old test-source snapshot is identical to current test (SHA below), and old workspace snapshot matches ada4 source. Independently recounted old author transcript **10 per phase**, new author transcript **5 per phase**. Author's full-workspace **377 unique + 4 docs** remains attributed to the author; unaffected Provider/Config/bin tests were not independently rerun here.

The single Linux profile includes instrumentation overhead; reducing command count is proven, but no latency percentile, actual Mac cause or deadline guarantee is inferred.

## Additional reviewer controls and preserved failures

A standalone public-inspect binary was linked from the exact tested workspace rlib before releasing the shared target. Eleven generated-repository controls were then run without Cargo: inert multiline config, mixed-case direct filter, active include, conditional include, false-promisor declaration, partial clone, assume-unchanged, skip-worktree, gitlink, supported internal-LF/TAB path identity, and the known legacy trailing-whitespace failure. For three executable-config cases, ordinary Git status first really creates an inert marker under the owned fixture; after deleting it, inspect refuses with UnsafeRepository and cannot recreate it. Every control verifies config/index/tracked-source bytes unchanged. No user repository/helper/account/network is involved.

Two initial reviewer-control observations remain preserved rather than presented as product failures: (1) different-length changed input did not trigger the ordinary-Git clean-filter positive control; same-length changed bytes produce the real marker, with no product change; (2) a root containing internal LF **and trailing TAB** reaches the original fallback `.trim()` and returns NotFound. That `.trim()` limitation exists in the ada4 reader by static comparison; it remains a fail-closed unsupported path, not an introduced permission bypass. The final supported-path control keeps LF and internal TAB and proves exact identity. No claim of universal whitespace-path support is made and no existing test/acceptance assertion was weakened.

This change's broader all-config query can reject an otherwise inert config exceeding the existing 4 MiB capture cap; that is explicit ResourceLimit behavior, tested with bytes unchanged. It neither silently drops config nor grants fallback authority. Filesystem races within inspection and uncontrolled same-user mutations remain outside any claimed OS sandbox.

## Exact fingerprints and evidence

| Source | SHA-256 |
| --- | --- |
| crates/workspaces/src/lib.rs | 2065213a1eb32be1e2a953adb0c5f0435d55987b4f8d0b40885797f5d11eb6e6 |
| crates/workspaces/tests/safety.rs | cc5fe6204309fc5a7a678e38830f63e24f30efc1a91faa449ffad6c82afa9b59 |
| crates/core/tests/admission_performance.rs | 4e1e1427c28cd8ae3b8305bb81a5ca87f8c3dfa225f4ce5539c040713a232c04 |

| Independent evidence | SHA-256 |
| --- | --- |
| `/tmp/ab-review-admission-affected.log` | `e393c6fe6ba8a44cb73f393313dfae07692a18297c0b7cf3eadd5eee1e1cd493` |
| `/tmp/ab-review-admission-clippy.log` | `8c233a54790c4800d3dd585436af679aa838da259bfdb129412e2d51cdb75cdc` |
| `/tmp/ab-review-admission-fmt.log` | `88492d9c5afc4b945db9939ef8248d402dd40fe8872003553c56a17ae29d4dbb` |
| `/tmp/ab-review-admission-profile.log` | `5f2f359e378884db464e2e3535e3df563e603088151d4d05a2f2513278f10ef6` |
| `/workspace/.onboarding/admission-performance-review/profile/git-calls.tsv` | `264aa13163bad121101087de201ba9d2c33f2702ca43845f841d99c2e4116c3e` |
| `/workspace/.onboarding/admission-performance-review/config-framing.log` | `fa70f6f5f5e4a95717d37a8187bf8a20d42542630988437937cd825fc5600f69` |
| `/workspace/.onboarding/admission-performance-review/matrix.py` | `8d2d7c013f6c87c37b422866991a38d09a1708c3b47421e9f5cd38d49d0a04d7` |
| `/workspace/.onboarding/admission-performance-review/matrix.log` | `0c4a2abc2e92fb1bab2f4b60cf9705b055b4306e4f07b46c7eea4ccc621521f2` |
| `/workspace/.onboarding/admission-performance-review/matrix-first-fixture-failure.log` | `31deaf6e3da02dda4c0728e465ebccb30ff43aa2604a88163253913c62111f35` |
| `/workspace/.onboarding/admission-performance-review/matrix-second-path-limit.log` | `c4e0aed040b64eccdb8ca73ded0206cee6ead33384d5f79662afb217b3ee1ca0` |

## Integration and Mac diagnostic flag

Primary reports cherry-picks `5cea4be`/`47ab520` into PR12. Read-only inspection confirms product changes retain the exact reviewed production source. Independently read the only extra uncommitted CI delta: Mac workspace command gains standard libtest `-- --show-output`, retaining its existing RUST_TEST_THREADS=1, commands, test count/order, permissions, timeouts and all Environment checks. **Static approval:** this prints captured successful profile output from the same execution before later command tests; it introduces no extra run, warm-up, skip, production flag or larger budget. Inspected diagnostics contain generated fixture timing/control results. Child stdout/stderr capture limits are enforced separately and unchanged; no new global CI-log-size guarantee or secret-redaction guarantee is inferred. Real-account/model tests remain unauthorized and are not enabled. The standard reporting flag was not redundantly Rust-tested.

Original failed ada4/201f logs are preserved and not replaced. No browser/native GUI/owner dialog, real Provider/model/private identity, macOS performance, default-release audit, exact-head CI or installer result is claimed by this Linux review. Both Mac original lifecycle paths and all required checks must actually pass at the next exact head before integration acceptance.
