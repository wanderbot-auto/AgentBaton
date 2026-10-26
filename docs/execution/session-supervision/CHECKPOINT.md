# Current Session first-CI portability checkpoint


## First real Session CI: FIFO fixture portability

PR6 head3b94d1c run40398801631 ARMjob121838837098 failed compiling the lib-test fixture: rustix::fs::mkfifoat unavailable on Darwin (E0425, policy.rs1066, exit101). Raw log copied to artifacts/session-fifo-portability/ci-arm-121838837098.log and hashed in EVIDENCE. This failure is preserved, not a test pass.

Test-only source5e4771de4992fe58bdcc612a583b9448f510ab32 replaces that single unavailable helper with absolute /usr/bin/mkfifo -m600 and direct path argument, asserts utility success and actual FIFO type. Both gitdir/commondir malicious pointers, original rejection assertions and killable3s child deadline remain. Production prefix is byte-identical to3b94; no dependency, lock or other-worktree change. rg crates/apps finds no remaining mkfifoat.

Under022: dedicated parent+child FIFO regression, all70 core tests(lib9; extra nested child not double-counted), workspace-all-targets Clippy andfmt pass. Previous193Rust/35UI/restart evidence is archived verbatim in EVIDENCE.pre-first-ci-3b94.json; those broader tests were not rerun for this test-only delta. Fresh macOS candidate CI and independent review remain required. No push or live vendor/model/account invocation.

## Preserved prior checkpoint

# Current protected-main session integration checkpoint


## Protected-main registry merge

Source `60715b9137e89ed20659094c9308e52b06b94956` merges reviewed session7124256 with protected-main dc5aea00013af0bf0fdae41d09c83ed0c4dacfd7, whose commit records PR#4 merged. OWNER001/002 re-read and ACK. IPC conflict resolution retains the complete session actor/run loop and existing worker delayed-frame regression, takes incoming configure_accepted_stream and authenticated delayed-frame/NOT_FOUND regression, and calls that helper from socket_worker. This preserves explicit blocking mode and3s deadlines; a configuration failure returns only that client worker. Source change relative712 is only this IPC consolidation/new regression. No schema5/native-provider code or dependency change.

STATUS is current session scope with PR3/4 merged; previous session and incoming-main STATUS bytes are archived verbatim in STATUS.before-main-7124256.md and STATUS.incoming-main-dc5aea0.md. Manifest path set and every hash exceptSTATUS are identical between both parents. Both committed INTEGRATION_REVIEW and PORTABILITY_REVIEW bytes are unchanged.

Under022,193 top-level Rust tests(core70/config45/providers58/workspaces18/daemon signal2; nested child excluded),35 UI/typecheck/build, Clippy/fmt, real daemon/CLI SIGKILL restart, design/infrastructure28 Python+4 Node andmanifest92 pass. EVIDENCE binds exact source/log hashes. Currentcandidate Mac/native/remote CI and independent merge review remain pending. Previous portability51ed evidence is preserved verbatim; previous failure logs remain failures. No push/main merge/live vendor/account/model action by this isolated integration task. Full product remains incomplete.

## Preserved previous checkpoint

# Current session portability checkpoint

## CI portability follow-up

Tested source `e261740f9e102142f3be47219dc495d26909a8b1` minimally adds explicit blocking reset before socket-worker first-frame reads and a bounded delayed-frame regression. Only fresh test-owned policy/registry managed roots get explicit Builder0700 permissions; imported sources, production root guards and all original negative assertions remain unchanged. No schema5/native-provider or primary-tree source copied.

The old-code controls failed101 as expected: the new delayed-frame test hits BrokenPipe; under022 all13 policy tests fail on private managed-root checks. These original failures remain in artifacts/session-portability. The corrected source passes under022:192 top-level Rust tests(core69/config45/providers58/workspaces18/daemon signal2; nested child not double-counted),35 UI tests, typecheck/build/fmt/Clippy, actual daemon/CLI SIGKILL restart, design and28 Python/4 Node infrastructure tests. Prior80e evidence is archived verbatim as EVIDENCE.history-bounds-80e.json. Current Mac/native/remote CI is not run and the historical ARM CI root cause is not claimed proven. No push/main merge or live vendor/account/model calls.

## Preserved prior history checkpoint

# Current session history resource checkpoint

Tested source695144b on isolated codex/session-supervision.4MiB raw journal pages are limited before fetch/decode;8MiB UTF-8 JSON/500-record visible history window cannot accumulate indefinitely. All persistent records retained, oversized first record not skipped; explicit page/window notices and history-only resource error handling preserve task controls.

191 top-level Rust tests(core68/config45/providers58/workspaces18/daemon signal2; nested child not double-counted),35 UI/helper tests, typecheck/build/fmt/Clippy, actual daemon/CLI restart, design/infrastructure28 and integrity92 pass. Full old-UI cursor navigation remains a separate UI7 unit; no current native/macOS/remote CI or full product acceptance. No push/main merge or real model/account activity. Parent next independently reviews this resource fix before retarget/push.

## Preserved previous8302 integration checkpoint

# Session PR6 integration checkpoint

Branch codex/session-supervision in isolated /workspace/.worktrees/session-integration; no primary-tree source modified. Fast-forward7a6a46b audit, prerequisite mergesa159aed(registrydfe3)/8f94696(a429docs), tested recipient fixbc47ea1. Protected main1f34b44 includes reviewed foundationPR3; registryPR4/sessionPR6 remain independent gated increments. OWNER001/002 ACK; QuietCanvas implementation stays dedicatedUI branch.

Schema4/session/actor/signal/native-owner-confirmation code retained.189 top-level Rust tests,28 UI mocks/typecheck/build, fmt/Clippy, actual daemon/CLI SIGKILL restart, design/infrastructure28 and integrity92 pass. Prior7 Linux host tests are historical; no currentcandidate Mac/native/remote CI claimed. Candidate awaits independentreview; parent handles push/retarget and exact required checks. Full product remains incomplete and all remaining requirements persist.

No real vendor/account/model, production, signing or added cost action. Preserve primary schema5/runtime work separately; never replace it with this older integrated branch. See EVIDENCE/INTEGRATION for exact logs, hashes and conflict decisions.
