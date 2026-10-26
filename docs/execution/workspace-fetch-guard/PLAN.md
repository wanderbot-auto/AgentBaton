# Workspace object-fetch execution guard

Base protected main dc5aea00013af0bf0fdae41d09c83ed0c4dacfd7; branch codex/workspace-fetch-guard in isolated worktree. AB-START-001 authorizes this work. OWNER001/002 re-read and ACK: full goal remains, UI amendment untouched. No Goal/task URL supplied by this entry.

Requirement AB-PRJ-001: importing owner-selected projects must not execute their contents. Related AB-PRJ-002/004: preserve workspace identity and source edits; this security increment does not complete those requirements. Config implementer and independent reviewer reproduced controlled missing-object promisor Git invoking a generated local ext helper. Reuse that focused guard/test design; no second adoption/validation framework.

1. Apply minimal GIT_NO_LAZY_FETCH=1, protocol.allow=never, fail-closed partialclone/promisor config guard before status/object work and checkout. Keep public types/API and existing filter/path guards.
2. Exercise generated missing-blob/ext-helper cases; no network/private identity/source execution. Verify unchanged config/index/content and no managed checkout on refusal. Run full workspace regression, fmt and Clippy.
3. Preserve source/log hashes and exact outcomes for independent review. Only workspaces lib/tests and this unit docs may change; no Cargo/locks/DB/UI, pushes or main merges. Parent integrates the reviewed focused commit into dependent branches.


Owner integration lead added the same read-boundary raw-object requirement after independent refs/replace reproduction: disable replacement objects and legacy grafts in Git command env, and verify committed bytes/parents with generated fixtures. This closes exact-SHA interpretation, not a new adoption engine.


## Conditional-include follow-up

Sourceb112185 adds explicit --no-includes audit of ANY include/includeIf.path before the first Git repository probe and again before checkout. Inactive rules are refused too, because futurechildGitdir may activate new executable configuration. Match→UnsafeRepository, no-match1→continue, fatal→NotRepository; IO/resource errors remain failures. No config is rewritten.

Two generatedtests show actualconfigurationvisibility: sourcefilterquery exits1, genuine no-checkoutfuture-control childquery sees filter.generated.smudge. This control executes no filter and is not oldBatonexecution evidence. Then inspect/create_isolated refuse future-negative with no helpermarker/destination/newworktree metadata and unchanged source/index/commonconfig/includedconfig. Even a benign unconditional include is refused unchanged.

All19 librarytests(previous17 plus2new),130 fullworkspaceRust(core35/config26/providers50/workspaces19),workspaceClippy/fmt pass under022. Initial18pass/1fail errorclassification result is preserved: malformed.git must remain NotRepository, fixed without modifying its originalassertion. Previous851 evidence archived verbatim as EVIDENCE.raw-guard-851.json. Independent /root/review inspected exactb112185 and independently reran all19testsPASS; no unresolved sourceblocker. Exactcandidate CI remains required; no currentcandidate Mac/native/remote CI or fullproduct claim. Followup performed no push and noRoot/session/QuietCanvas source access.


## Actual PR9 CI daemon restart follow-up

ARM run40403211132/job121854191652 at32ca76 failed existing project_ipc realrestart with WORKSPACE_BUSY at newbind afteroldserver joined. The copied rawlog remains a failure; no nativeFD/fork timeline wascaptured. This is not a guardregression testfailure and does not prove the historicalcause.

Source93ca7ea changes onlycore/ipc.rs: after removing ownsocket, DaemonDrop explicitly unlocks its ownFileExt writerlock. Thisoldbaseline has noSupervisor/ownedproviderworker; run no longerdispatches byDrop. fs2 0.4.3 uses Unix flock LOCK_UN, and its duplicate tests document sharedlocksemantics. A direct duplicateFD test actually fails old32production with WORKSPACE_BUSY, thenpasses fixed. It retains livewriterrefusal, immediatebind with olddup stillopen, olddupclose not unlockingnewowner, and finalrestart. No sleeps/retries/testweakening/fork or externalhelper added.

Under022 deterministic1, existingprojectIPC3, allCore36 and fullworkspace131(core36/config26/providers50/workspaces19),Clippy/fmt pass. Previous130testguard evidence archivedverbatim EVIDENCE.include-32.json. No workspacesguard/schema/actor/profile/lock/dependency change. Root/sessionactor ports must shutdown/reap everyownedworker BEFOREwriterunlock onallpaths; fieldDrop aftercustomDrop isinsufficient. Independent /root/review exactsource review+all36Coretests APPROVED; freshnativeCI remainsrequired, no push/mainmerge.
