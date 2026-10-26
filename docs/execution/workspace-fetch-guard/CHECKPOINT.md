# Current writer-lock restart checkpoint


## Actual PR9 CI daemon restart follow-up

ARM run40403211132/job121854191652 at32ca76 failed existing project_ipc realrestart with WORKSPACE_BUSY at newbind afteroldserver joined. The copied rawlog remains a failure; no nativeFD/fork timeline wascaptured. This is not a guardregression testfailure and does not prove the historicalcause.

Source93ca7ea changes onlycore/ipc.rs: after removing ownsocket, DaemonDrop explicitly unlocks its ownFileExt writerlock. Thisoldbaseline has noSupervisor/ownedproviderworker; run no longerdispatches byDrop. fs2 0.4.3 uses Unix flock LOCK_UN, and its duplicate tests document sharedlocksemantics. A direct duplicateFD test actually fails old32production with WORKSPACE_BUSY, thenpasses fixed. It retains livewriterrefusal, immediatebind with olddup stillopen, olddupclose not unlockingnewowner, and finalrestart. No sleeps/retries/testweakening/fork or externalhelper added.

Under022 deterministic1, existingprojectIPC3, allCore36 and fullworkspace131(core36/config26/providers50/workspaces19),Clippy/fmt pass. Previous130testguard evidence archivedverbatim EVIDENCE.include-32.json. No workspacesguard/schema/actor/profile/lock/dependency change. Root/sessionactor ports must shutdown/reap everyownedworker BEFOREwriterunlock onallpaths; fieldDrop aftercustomDrop isinsufficient. Independent /root/review exactsource review+all36Coretests APPROVED; freshnativeCI remainsrequired, no push/mainmerge.

## Previous Git boundary checkpoint

# Conditional-include guard checkpoint

Tested sourceb11218506e835554b2d08600e8cc1ab25e4fe7e9 on codex/workspace-fetch-guard. All130workspaceRust/19librarytests,Clippy/fmt pass under022. Include/includeIf audits failclosed before firstrepositoryprobe and beforecheckout; benign/inactive rules unsupported, metadata never repaired. Prior17tests unchanged.

Actual future-child control proves only configurationvisibility, not oldBatonfilter execution. Protected calls reject before helper/destination/index/config effects. Initial malformedrepository errorclassificationfailure retained; fixed without testweakening. Independent /root/review exactsource review and19test rerun APPROVED. Primary can cherry-pick b112185 only atop851 and arrange exacthead CI. No push/mainmerge performed here; parentreports existingPR9/851push is historical. No Root/session/QuietCanvas source access, dependencies or othermodules.

Previous851 checkpoint and128-test evidence are historical and retained below/inarchive. Fullproduct remains incomplete.

## Previous checkpoint

# Focused fetch/raw-object guard checkpoint

Branch codex/workspace-fetch-guard, base protectedmain dc5aea00013af0bf0fdae41d09c83ed0c4dacfd7. Final testedsource8dcb4a3f951ca9223c5eb7b803e7de72340de64c includes source4d43989(fetchguard) and8dcb4a3(rawobjectguard); middle9430a1d is documentation only. No dependency/lock/database/UI or otherworktree code changes. OWNER001/002 ACK; complete Designv0.2 goal remains.

128 fullworkspaceRust tests(core35/config26/providers50/workspaces17),17 librarytests,Clippy/fmt PASS under022. Independent /root/review source review and17-test rerun APPROVED, no unresolved aggregate blocker. Actual local controls prove replacement/graft redirection while safehelper reads raw committed bytes/parents and preserves repository metadata. Earlier fetch126-test evidence remains archived.

Parent may cherry-pick the two sourcecommits(4d43989 then8dcb4a3), or the whole focusedbranch including unitdocs. Preserve later hidden-index/stat/adoption guards when resolving context. Run exactcandidate CI on each integration; no current Mac/native/remote CI claimed here. No push/mainmerge/vendor/model/account/network action performed by this focused unit. Full product remains incomplete.
