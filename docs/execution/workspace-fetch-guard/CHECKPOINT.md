# Focused guard checkpoint

Branch codex/workspace-fetch-guard from protectedmain dc5aea00013af0bf0fdae41d09c83ed0c4dacfd7. Tested source4d43989;126 fullworkspaceRust tests,15 workspace tests(including2new negatives),Clippy/fmt PASS under022. No source/dependency change outsideowned library/testpaths. Config/adoption/native/session work remains separate.

Independentreview is requested. Parent should cherry-pick only the focused reviewed sourceguard into root/session/QuietCanvas as appropriate, resolve later stat/index/adoption context without replacing newer library features, then run exactcandidate CI. No push/mainmerge performed here. No Mac/native/runtime/fullproduct acceptance claimed. See PLAN/PROGRESS/EVIDENCE for scope,commands and hashes.
