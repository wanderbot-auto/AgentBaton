# Progress

Source4d43989 implements the focused no-lazyfetch/protocol/partialclone guard only. inspect refuses configured partial externalobject repositories before HEAD/status; checkout repeats the refusal. New generatedmissingblob/helper-marker tests prove rejection/noexecution/no new checkout/source/index/config preservation. Existing13 workspace safetytests remain unchanged and pass. PublicAPI and filter/pathguards preserved.

Under022,2 newguardtests,15 totalworkspacestests,126 fullRustworkspace tests(core35/config26/providers50/workspaces15), workspace-all-targets Clippy andfmt pass. Source/log hashes and commands are in EVIDENCE.json. All exercised inputs are generatedlocal fixtures; no real remote/helper/provider/credential access.

ACK OWNER001/002 from latest fetched protectedmain dc5aea0. Scope limited to workspaces library/tests and thisunit docs; no globalSTATUS/manifest, Cargo/locks,DB/UI or otherworktree source edits. Configimplementer provided focused patch; independentreview requested before parentpush/integration. Native/macOS/remoteCI not run by thisunit. Fullproduct remains incomplete.


## Raw commit and ancestry follow-up

Source8dcb4a3 adds GIT_NO_REPLACE_OBJECTS=1 and GIT_GRAFT_FILE=/dev/null. Actual control Git HEAD==approvedA/statusclean nevertheless reads replacedB; safehelper inspect sees dirty against rawA and rejects adoption, while exactA isolatedcheckout contains approved bytes. The sourceB/index/replaceRef stay unchanged. A realgraft C→A control returnsA parent; helper returns recordedB parent without changing graft/index.

17 workspaces tests(13existing+2fetch+1replacement+1graft),128 fullworkspaceRust tests(core35/config26/providers50/workspaces17), Clippy/fmt pass under022. Independent /root/review inspected aggregateguard and independently ran all17workspaces testsPASS; no unresolvedsourceblocker. Parent exactcandidateCI remains required. Source, commands and hashedlogs in EVIDENCE; initial126-test fetch checkpoint archived verbatim.
