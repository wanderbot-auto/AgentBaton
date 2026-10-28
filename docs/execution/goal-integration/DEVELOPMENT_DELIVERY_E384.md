# Goal planning development delivery

PR13 merged to protected main18f4261da9285bec5446101d47786839b1020d9c only after exact7/7, independent code/native/visual/package review and Root final main/head/OWNER guard. Source e384ccaa2d05aa403327867c0d6c5b50baf437e0; tested/built checkout b6756c3200feb09c3a9a04b3690ae5ee579bf913; equal treef1b484c1a928f381fd6ef12c34a0616f2f1c40c7. Actual buildrecords macOS15.7.9/Rust1.99.1/Xcode16.4, NodeARM24.20.0/Intel24.19.0.

| Development package | Download | DMG SHA-256 | Signature |
| --- | --- | --- | --- |
| Apple Silicon arm64 | https://github.com/wanderbot-auto/AgentBaton/actions/runs/40658084175/artifacts/12802954121 | `24ef220c070c2c82497df452db4eea810780cd27a09a77d20392a5ed76775287` | ad hoc/linker-signed; TeamIdentifier unset; no DeveloperID/notarization |
| Intel x86_64 | https://github.com/wanderbot-auto/AgentBaton/actions/runs/40658084175/artifacts/12804745792 | `c21eb84d5afbce621a20228bad65aca4b3cd47628370130bfbd9079f85b792b8` | not signed at all; no DeveloperID/notarization |

Actual newZIP API digests, CRC/safe-entry/type/count/size audits, UDIF footer, SHA256SUMS, exact commit/arch filenames, buildrecords and acceptance checklists independently verified. Root reran the same helper from the same new downloads. Refer to PACKAGES_E384.json, CI_E384_VERIFICATION.json, NATIVE_E384_REVIEW.md and architecture-specific DEVELOPMENT_ACCEPTANCE files. All old failure/reviewer corrections remain separate. Artifacts use the existing14-day CI retention.

This package implements the current approved planning/preview slice, not full Designv0.2 completion. Actual Goal execution/runtime, Data snapshot copies and other full130 requirements continue in followup workstreams. Known sidebar long-project-name overflow is corrected in DataPR14, not claimed fixed in this package. Manual OS Owner dialogs, real Provider/model/account tests, minimum macOS and real user installation/quarantine are unverified; final license/formal signing/notarization/production remain separately gated. Do not disable macOS security to turn unsigned development packaging into a signing claim.
