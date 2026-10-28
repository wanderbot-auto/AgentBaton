# Independent BF676 CI and native evidence review

Verdict: **NOT ACCEPTED for merge, native visual acceptance or dual-package delivery.** Six required checks succeeded; Intel Product failed a30ms workspaces script fixture. ARM machine Native reports passed, but its exact new screenshot is visually blank. No new Intel Native or DMG exists. Reviewer `/root/data_native_integration_review`; independent, read-only CI evidence work.

## Exact subject and final checks

Head bf67629f7911531fec52ec31e59c9508f814f9fd, actual checkoutb0daefb57a35a47e2f708e02df8d14b578e76148, parents[mainbcf10bb802258645c554f37581237a6c0cb3cbe6,bf], equal tree e447a3a18ed193ee439dcfa341d438573b5ec80b. All3completed Environment raw logs confirm actual checkout and full Infrastructure34; exact Git API objects/source hashes are saved. The reviewed narrow finite policy and guards are precisely present, Environment workflow unchanged.

Environment40652731215: ARM122696392148/Intel122696392555/Ubuntu122696392608 all success. Product40652731429: Rust122696733814/UI122696734178/ARM122696734194 success; Intel122696734328 failure. Exact head check-run API has six success/onefailure and Product overallfailure. New Rust/UI completedraws bind this checkout and record generated protocol20, Goal contracts48/store34, runner17, realCLI SIGKILL/restart, frontend259 and actualbrowser38/Command5/Goal9/Editor10. Prior A2/03 checks/artifacts are not counted.

## Actual Intel failure

Core/nativehost step failed in workspaces `--test command_validation`, script suite12pass/1fail. Test `scripts::expected_exit_zero_cannot_accept_missing_or_oversized_output` panicked at line898: exit_code actualNone, expectedSome(0), in the first finite printf stdout-only branch. Both !passed and file_integrity.passed=false assertions had already passed, so this did not accept missing output. The actual RunReason is not printed and remains **unknown**. Source assigns wall_time_ms30 to both this finite missing-output branch and the infinite oversized-output loop; normal fixture default is1000. Original source is saved/hash-bound.

This is not Core command14 (actuallypassed, fresh grant997ms), Provider500ms (20 and original4MiB/500ms case passed), Goal Store or the overall45-minute CI envelope. Intel raw SHA `49438587967d786eee591b202b9c3774bbf346e6352da1d6d8e22c481db51008`. Native host/harness/window/defaultreleaseaudit/packaging were not reached successfully; actual upload annotation says no native-smoke files, final API has no Intel Native or development artifact. No reason/timing cause is invented, no assertion removed and no deadline relaxed by this review. A separate narrow correction and fresh exact-head evidence remain required.

## ARM machine evidence versus failed visual evidence

New ARM native artifact12800709441 ZIP SHA `eb1c3cc0c13893f5f81ea7142d9d2978917279ced5552200e81a0c8d9944c531` matches API and passes bounded safeZIP/CRC, exactcheckout/architecture, strictreport/runner checks. Actual host13/harness12, Goal3 JS plus independent readonlyDB0/0/0 sequence3 unchanged, originalCommand3/Data3, first/reopenedWebViews sharing task/project, daemon lifecycle and ownedcleanup all reportpass. ARM raw SHA `24f516962bbb38d891aff35c9bb310d5fcd6fab8d1519cf965dbfd38ec6e9771`.

However, actual newPNG1136x793, SHA `ec5d9903e51d4a1bb33df7edabbc6b12ab9bf78dec9ec913db508d95551939d7`, was opened using view_image and is **almost entirely blank white**, showing only window frame/title/scrollbars/sidebar lines, without task heading, icons, task content or composer. It therefore **fails actual UI screenshot acceptance**. The machine artifact audit checks PNG signature/dimensions, which cannot establish meaningful captured content. DOM/lifecycle flags can remain passed while this visual dimension fails. Paint/capture/compositor timing or actual display cause is not established; no product UI regression is inferred without evidence. ARM_VISUAL_FAILURE.json binds the actual image and observation. No old PNG substitutes for it.

## One ARM package, no accepted pair

ARM wholejob passed actual default-release identifier audit and generated development12801096492. API/local ZIP `sha256:e9683a0dfb607eb8d33a40ed63ca239d9a9fe7c539169a39b5ffdb2524bda851` matched, safeentries/CRC/UDIFkoly/SHA256SUMS/exactcheckout-arm64 buildrecord/nonemptychecklist checked. DMG SHA `645c8c688b4b95479c23fd9173d923a9e9869d8cb67a1f16cf17413972da5a43`. Actual record: macOS15.7.9, ad hoc/linker signature, TeamIdentifier unset, no DeveloperID/notarization. This is a single actual package from a failed Product run, not a deliverable pair or visual acceptance. The success-only dual-package helper was not invoked.

## Handoff

All raw Git/CI API snapshots, original source, failure logs, new ARM archive/actualimage, partialpackage and exact check results persist in `/workspace/.onboarding/goal-native-bf676/`. Manifest SHA `d4cc44dc9875a0abbeb75ca57d9f45166a9cd47c621a84b40e3ac44e8559a55e`; all listed files were rehashed. The bounded readonly watcher naturally exited after19 observations when all4Productjobs completed, with no remaining watcher process.

The earlier03 Environment failure and reviewer omission remain separately preserved; this new head really passed all3Environment. Historical0a ARM initialize cause remains unknown; failure-only diagnostics are not a production performance fix. Intel30ms fixture diagnosis and ARM native screenshot capture/validation remain distinct unresolved issues. New independently reviewed corrections and exact7/newnative/visual/defaultreleaseaudits/dualDMGs are necessary before merge. Actual OS Owner dialogs, live Provider/model, minimumOS/userinstallation/formalsigning and wholeGoal/full130product acceptance remain unverified. No Cargo/target/source/index/CI-trigger/cancel/retry/remote-write/merge/account/model action was performed here.
