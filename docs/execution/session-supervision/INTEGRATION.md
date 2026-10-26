# Session PR6 integration preparation

Read protected main1f34b444821640bf88f32a8bb39c6434d4ca26cc and OWNER001/002; retained the approved Quiet Canvas reference. FoundationPR3 is merged there. RegistryPR4 and sessionPR6 remain separate incomplete increments, not main acceptance or full-product delivery.

The isolated session worktree was clean atd7acacb and fast-forwarded to remote7a6a46b (the independent audit). Local mergea159aed integrates registrydfe3d735; local merge8f94696 integrates its independently reviewed a429cb3 docs. No push or main merge was performed.

Initial conflicts: BOOTSTRAP_MANIFEST, IPC/storage comments, transport and stdio/installation fixtures. IPC/storage retain the already equivalent fd-close-before-SQLite and numeric-only diagnostics. The session transport already contains the reviewed readiness/progress fixes plus pending-frame polling/cancellation; its newer code is preserved rather than reverted to registry's older blocking exchange loop. Both fixture families retain their modes, original500ms/4MiB bounds, canonical workspace identity and actual4KiB capacity assertions; added small-pipe regressions remain. Manifest is the92-path union and STATUS preserves source reports as historical. The a429 docs-only followup conflicted only in STATUS/manifest; its independent review/checkpoint is retained.

Security fixbc47ea1 is intentionally narrow: grant_proposal_refs now reads project_name and execution_recipient name/provider/route/executable/argv/hash/status from the server's registered records. Schema4 supports only customACP, so absent protocol_route is explicitly custom_acp. The native host sees that fixed server snapshot; renderer labels are not authority. Existing checked finalization compares all newly displayed references again. The backported native_owner_grant_snapshot_identifies_the_exact_registered_recipient test verifies distinct readable recipients and rejects changed program metadata even with unchanged opaque references. No primary-tree schema5/native route/runtime-config module was copied.

Atbc47ea1,189 top-level Rust tests pass (core66/config45/providers58/workspaces18/daemon signal2; one nested FIFO child harness not double-counted), workspace fmt/Clippy pass,28 UI/wire mocked tests+typecheck/build pass, real daemon/CLI restart smoke passes, and design/infrastructure28+92-file integrity pass. Logs and source hashes are in EVIDENCE.json. Native host testing on this integration candidate was not rerun; prior7 Linux host tests are historical, not inherited acceptance.

Independent review and exact-candidate remote/native CI remain required. This unit has not pushed, retargeted or merged any PR. It invokes only generated local fixtures, no real vendor/account/model, production, signing or extra paid resource. Schema4 actor, persisted dispatch/grant/session identity, responsive cancellation, signal cleanup, native owner confirmation, crash-unknown fencing and partial history semantics remain intact. Full design requirements and OWNER002 UI amendment remain; no full-product claim.


## History byte-bounds follow-up

Tested code695144b minimally backports4MiB Store.events pagination and its two large-record regressions; schema remains4. SQL octet_length obtains size before reading/decoding the full document, preserves sequence order and last-returned next_cursor, and refuses an oversized first record with RESOURCE_LIMIT and event-sequence detail rather than skipping it. Response page_bytes/byte_limited are explicit.

The existing schema4 frontend accumulates at most8MiB encoded JSON (including array delimiters) and500 events. When combining earlier/current records would exceed either bound, it displays the requested bounded earlier page and states other records remain in the local journal. Incoming pages beyond display bounds are rejected. Backend byte-limited pages are labeled; history RESOURCE_LIMIT leaves authenticated task cancellation available. This copies no Quiet Canvas layout or native Provider selector. It does not claim full cursor navigation; UI7's separate pagination remains necessary. Session history is still a bounded page, not complete/raw-provider history. No journal record or assertion was deleted.

At695144b:191 top-level Rust tests(core68/config45/providers58/workspaces18/daemon signal2),35 UI/helper tests, typecheck/build/fmt/Clippy, actual daemon/CLI restart, design/infrastructure28 and integrity92 pass. Earlier8302 evidence is retained verbatim in EVIDENCE.integration-8302.json. Current EVIDENCE binds the new source and logs. Independent review/current-candidate remote/native checks remain required; no push/main merge or full-product claim.


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
