Implemented real named custom-ACP connection registration, native-owner-confirmed server proposals, bounded grants, immutable persisted session/turn/attempt bindings and daemon-owned child workers. Renderer cannot finalize approval or invoke arbitrary shell/IPC. Worker terminal receipts cannot be dropped by stream backpressure; timeout/cancel/shutdown distinguish request, native turn stop and owned process reap. Restart unknown writers remain unreplayed and fenced. Existing-session grant checks use bounded filesystem identity/linkage validation, permitting the writer's own edits while retaining revocation/expiry and snapshot checks.

Native source adapters now parse selected Codex TOML, Claude settings/MCP JSON and SKILL catalogs without executing content or exposing private values in preview. They are not yet wired to complete runtime materialization. Hidden Git-index state is rejected for clean-input admission. YAMLalias OOM and FIFO pointer hazards found by independent review were fixed with preserved evidence.

Integration:186 top-level Rust tests,28 mocked-transport UI tests, typecheck/build, fmt/Clippy and actual daemon/CLI restart smoke pass. Linux Tauri check and7 native-host tests now actually pass with private signed-package dependencies. These are not macOS GUI/dialog acceptance or real vendor compatibility.50 independently reviewed source hashes match. See EVIDENCE.json and REVIEW.md for precise limits. Continue full remaining goal after this checkpoint.


## PR6 prerequisite and recipient integration

Fast-forwarded audited7a6a46b; local mergesa159aed/8f94696 retain registrya429cb3, protected main1f and foundation/readiness fixes. OWNER001/002 read/ACK. Narrow readable-recipient proposal backportbc47ea1 stays schema4/customACP. Local189 Rust,28 UI mocks/typecheck/build, fmt/Clippy, actual daemonrestart, design/infrastructure28 and integrity92 pass. See INTEGRATION.md/EVIDENCE.json for conflicts/exact source and remaining native/review gates. No push/main merge/full product acceptance.


## Byte-bounded journal and display history

Source695144b bounds raw event pages at4MiB before document fetch/decode, preserving cursors and exact oversized-record error. The legacy frontend has8MiB/500-record display bounds with explicit local-retention/page notices and keeps task controls usable when history alone is refused.191 top-level Rust+35 UI/helper tests, typecheck/build/fmt/Clippy, actual restart smoke and infrastructure/integrity pass. No schema5/QuietCanvas/native-route copy; full pagination remains UI7 work. Original8302 checkpoint/evidence is preserved as history.


## CI portability follow-up

Tested source `e261740f9e102142f3be47219dc495d26909a8b1` minimally adds explicit blocking reset before socket-worker first-frame reads and a bounded delayed-frame regression. Only fresh test-owned policy/registry managed roots get explicit Builder0700 permissions; imported sources, production root guards and all original negative assertions remain unchanged. No schema5/native-provider or primary-tree source copied.

The old-code controls failed101 as expected: the new delayed-frame test hits BrokenPipe; under022 all13 policy tests fail on private managed-root checks. These original failures remain in artifacts/session-portability. The corrected source passes under022:192 top-level Rust tests(core69/config45/providers58/workspaces18/daemon signal2; nested child not double-counted),35 UI tests, typecheck/build/fmt/Clippy, actual daemon/CLI SIGKILL restart, design and28 Python/4 Node infrastructure tests. Prior80e evidence is archived verbatim as EVIDENCE.history-bounds-80e.json. Current Mac/native/remote CI is not run and the historical ARM CI root cause is not claimed proven. No push/main merge or live vendor/account/model calls.


## Protected-main registry merge

Source `60715b9137e89ed20659094c9308e52b06b94956` merges reviewed session7124256 with protected-main dc5aea00013af0bf0fdae41d09c83ed0c4dacfd7, whose commit records PR#4 merged. OWNER001/002 re-read and ACK. IPC conflict resolution retains the complete session actor/run loop and existing worker delayed-frame regression, takes incoming configure_accepted_stream and authenticated delayed-frame/NOT_FOUND regression, and calls that helper from socket_worker. This preserves explicit blocking mode and3s deadlines; a configuration failure returns only that client worker. Source change relative712 is only this IPC consolidation/new regression. No schema5/native-provider code or dependency change.

STATUS is current session scope with PR3/4 merged; previous session and incoming-main STATUS bytes are archived verbatim in STATUS.before-main-7124256.md and STATUS.incoming-main-dc5aea0.md. Manifest path set and every hash exceptSTATUS are identical between both parents. Both committed INTEGRATION_REVIEW and PORTABILITY_REVIEW bytes are unchanged.

Under022,193 top-level Rust tests(core70/config45/providers58/workspaces18/daemon signal2; nested child excluded),35 UI/typecheck/build, Clippy/fmt, real daemon/CLI SIGKILL restart, design/infrastructure28 Python+4 Node andmanifest92 pass. EVIDENCE binds exact source/log hashes. Currentcandidate Mac/native/remote CI and independent merge review remain pending. Previous portability51ed evidence is preserved verbatim; previous failure logs remain failures. No push/main merge/live vendor/account/model action by this isolated integration task. Full product remains incomplete.


## First real Session CI: FIFO fixture portability

PR6 head3b94d1c run40398801631 ARMjob121838837098 failed compiling the lib-test fixture: rustix::fs::mkfifoat unavailable on Darwin (E0425, policy.rs1066, exit101). Raw log copied to artifacts/session-fifo-portability/ci-arm-121838837098.log and hashed in EVIDENCE. This failure is preserved, not a test pass.

Test-only source5e4771de4992fe58bdcc612a583b9448f510ab32 replaces that single unavailable helper with absolute /usr/bin/mkfifo -m600 and direct path argument, asserts utility success and actual FIFO type. Both gitdir/commondir malicious pointers, original rejection assertions and killable3s child deadline remain. Production prefix is byte-identical to3b94; no dependency, lock or other-worktree change. rg crates/apps finds no remaining mkfifoat.

Under022: dedicated parent+child FIFO regression, all70 core tests(lib9; extra nested child not double-counted), workspace-all-targets Clippy andfmt pass. Previous193Rust/35UI/restart evidence is archived verbatim in EVIDENCE.pre-first-ci-3b94.json; those broader tests were not rerun for this test-only delta. Fresh macOS candidate CI and independent review remain required. No push or live vendor/model/account invocation.


## Reviewed Git guard integration

Merge3bd37e614047c7b3a33616cdfdab0c4c4d895061 integrates guard32ca76 into sessionb85a64d (which committed only the pre-existing independentFIFO_REVIEW). Sole sourceconflict was WorkspaceError Display; retain incoming generalUnsafe text and existingUnverifiedIndex arm. All18oldworkspace safetytests/stat/hidden-index/filter/path guards remain byte-identical;6newraw/fetch/include guardtests add to24. Actor/schema4/policyFIFO/locks/CLI/daemon and all trackeddesktop source bytes unchanged. No profiles/schema5/adoption/dependency import.

Under022:24workspaces and199top-level fullworkspaceRust(core70/config45/providers58/workspaces24/daemon signal2; extraFIFOchild excluded),35UI/typecheck/build,fmt/Clippy,binaries+actualdaemonCLIrestart pass. IncomingguardunitEVIDENCE/docs are preserved byte-identical32; priorSessionFIFO evidence archived verbatim as EVIDENCE.fifo-cf00.json, including realARMcompilefailure and originalreviews. Each candidate's evidence remains separate.

Latestmain fetched dc5aea0; OWNER001/002 re-read and ACK. Parent will mergeguardPR9 only after its exactcandidate checks/review, then integrate latestmain ancestry and run onefreshSessionCI. That futuresequence is not claimed performed here. CurrentSessionmerge still awaits independentreview and native/remoteCI; no push/mainmerge or realmodel/account/remote-helper call by this unit; the authorized repository fetch is recorded separately.


## Protectedmain PR9 and actor writerlock integration

Tested merge17b71d5b6cd046ecaad3777cdefbd3f3cac0174a combines reviewedSessiona763d89 (FINAL_GUARD_REVIEW committed alone) with actualprotectedmain eb115e0915a4dd0ac46c8c188101b64b89e9d35f. PR9 is nowactuallymerged; lead reports allseven exact34 checks and both samecheckout developmentDMGs verified. OWNER001/002 re-read/ACK; no newauthority. These prerequisite checks are not Sessionnativeacceptance.

Only IPCadjacenttest conflict: retain workerfirstframe +main duplicateFD tests, plus existingauthenticatedfirstframe. Add selectedRoot573 actorcleanup unit. Existing SupervisorDrop cancel/join/probejoin body is movedverbatim into noDBmutation pub(crate)reap_owned_workers; DaemonDrop invokesit FIRST, thenunlinkownsocket, thenFileExtunlock. SupervisorfieldDrop can safelycallagain afterdrain. ExistingWorkerReceipt alreadyhad matching fields, so no model/schema/binding changes. No native/profile/adoption/UI sourcecopied.

The intermediate badport (mainunlock with cleanup deferred toSupervisorfieldDrop) actuallyFAILS101: a newbind succeeds duringoldownedworker cleanup and checkedflag fails. CorrectedportPASS asserts BUSY+oldsocket duringcleanup, then immediatebind afterfullDrop despiteolddupFD, and olddupclose cannotunlocknewowner. Testfixture is cfg(test) only anddoesnot journalfakeprocess acceptance; production reaping makes noDBmutation, leaving unpersisted ambiguousoutcomes fenced forrestart. Nativehistoricalcause remainsunproven.

Under022:201top-level Rust(core72/config45/providers58/workspaces24/daemon signal2; extraFIFOchild excluded),24workspace,35UI/typecheck/build,Clippy/fmt,binaries+realdaemonCLIrestart pass. Guardmainunitdocs/EVIDENCE retained byte-identicaleb; previous199-test Sessionevidence archivedverbatim EVIDENCE.guard-334.json. Existingstorage/policy/locks/desktop and dispatch/normalshutdown/run_worker source remain unchanged. Parentindependentactorcompositionreview and freshSessionPR6 exactcandidate/nativeCI stillrequired. No push/mainmerge/model/account action by this localintegration; authorized repositoryfetch recorded.
