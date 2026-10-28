# Independent exit-zero artifact-contract fixture review

Verdict: **PASS for this test-only semantic correction and stronger artifact assertions; no blocking finding remains. Suitable for integration and fresh exact-head CI.** Reviewed source14a20fe15b957f9e41df2e322451f5fed00b7ea7/docs2016298693d3a5b012a212b376b413460f09d78d, independently by `/root/data_native_integration_review`. Source SHA `3b039348580055680dc7e229c2c26c8d34ee3e434ebe1209f6c166c5c828ea7e`. This does not turn the failed BF Intel run green or establish its historical reason.

## Why this preserves and strengthens acceptance

The target test concerns normal exit-zero processes whose required output is missing/oversized. It previously imposed30ms on both a finite printf and an infinite output writer, while requiring exitSome(0)/ArtifactContractFailed only for the finite case. A short deadline can prevent normal completion before the intended artifact check. The actual Intel log failed None versusSome0 after !passed/file_integrity=false had passed; its actual RunReason was not printed and remains unknown.

The correction explicitly changes the **finite test-fixture** selection from30ms to its already-existing1000ms default. This is disclosed as a test-fixture change, not claimed as all timeout literals unchanged. Production limits/defaults, authorization/RPC/native budgets and every other test deadline are byte-identical. Both original Some0/ArtifactContractFailed assertions are retained and now also apply to a new finite printf writing exactly5000 bytes against the unchanged4096-byte output requirement. Exact Missing/TooLarge output states, one output check, exit observed within the configured budget and owned leader reaping are additionally required. The finite5000-byte output is actually stat-checked. A mere timeout or any generic false boolean cannot satisfy either finite artifact case.

The **original infinite writer script,30ms deadline, !passed and failed file-integrity assertions remain**. Other existing30ms cancel/timeout/capture controls, delayed observations, cleanup bounds and complete contract guards are unchanged. This separates distinct failure reasons and adds direct acceptance constraints; it does not make invalid output acceptable or change product execution behavior. A real30ms command that times out still fails under the unchanged runner.

## Mechanical source review

Entire source commit changes only crates/workspaces/tests/command_validation.rs. Replacing just the target function body and the CheckState import/reflow with exact baseline bytes restores the entire original file byte-for-byte. All29 cross-platform test function names remain: one modified and28 others byte-identical. Linux selects27; two functions are macOS-only. An initial reviewer count assumption of30 was corrected against exact source, with no source change. Source/locks/manifests/CI/production runtime and normalFixture1000/cleanup100/output4096 defaults are unchanged. No screenshot or Native capture/product modification is introduced.

## Independent actual verification

Only the explicitly released private Provider target was used, with umask022, incremental0, jobs2 and RUST_TEST_THREADS1. Own workspaces lib entrypoint received mtime-only refresh, with content checked unchanged to prevent stale cross-worktree metadata.

- `cargo test --locked --offline -p agentbaton-workspaces --test command_validation -- --nocapture --test-threads=1`: **27 unique tests passed**, zero failed/ignored. Three nested real observation-child runs overlap existing cases and are not counted as30 unique. Whole command suite log SHA `4729a032008ff2d0c45893286eff89625fe58f10d36100f09c23ccd4b3178f70`.
- All-target workspaces Clippy with -Dwarnings, package fmt check and exact source diff check passed. Clippy log SHA `1fdf67df0125aa044fffb9cfa5116103a35f761fa1f9ff0a9ca8a25e23711acb`.
- All19 author manifest artifacts were independently rehashed, including exact old/new source/binaries/logs. Original BF Intel raw SHA49438587967d786eee591b202b9c3774bbf346e6352da1d6d8e22c481db51008 remains unchanged.

Independent replay of saved hash-verified generated50ms controls: oldsource30ms really FAIL101, DeadlineExceeded/None, false inbudget observation and ownedreaped; correctedsource default1000 really PASS0 with Some0/ArtifactContractFailed/inbudget/reaped, exact Missing and TooLarge states, plus retained infinite30ms DeadlineExceeded control. Control source inverse comparisons prove only the explicit50ms script literal and safe typed diagnostic/import reflow differ from exact baseline/current test sources. This is **a Linux mechanism control, not reproduction of the historical Intel timing or its unprinted reason**. Saved probe binaries were replayed, while the real final suite/lints were independently recompiled from14a.

## Handoff and limits

Private target `/workspace/.worktrees/provider-startup-evidence/target` is formally **released**, with no remaining Cargo writer from this review; main and Native targets were never used. Git worktree/source/index remain unchanged and clean. Persistent evidence under `/workspace/.onboarding/artifact-contract-fixture-review/`, manifest SHA `e41f746cba23a77b9a99e376bc47b36b9f221ab3ab0bf6aa43bcbe7c03174c53`. No remote/CI-trigger/retry/cancel/merge, Provider/model/account/credential/signing or production actions were performed.

Fresh exact-head Environment/Product checks, actual ARM/Intel command13/Native/defaultrelease audits and real dual development packages remain necessary. Linux success does not establish new macOS shell execution or minimumOS/user-install/OSOwnerDialog/realProvider acceptance. The earlier ARM blank-image interpretation was separately corrected as reviewer error and is not a capture defect or scope for this change.
