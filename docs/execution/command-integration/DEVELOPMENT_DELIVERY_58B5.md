# Command milestone development installers

PR12 protected merge bcf10bb802258645c554f37581237a6c0cb3cbe6. Source head58b5d329ff009f07eef5ec9776b15a2676504dc6; actual CI checkout8f19d8f7a4c6db032351a28c7629d8df7882c383. Current main merge/head/CI-checkout trees match. Product40458672955 and Environment40458672868 all7 actual checks passed before expected-head/current-main/clean protected merge. Native/source and actual packages were independently reviewed separately.

| Architecture | GitHub artifact | Actual DMG SHA-256 | Signature |
| --- | --- | --- | --- |
| Apple Silicon | https://github.com/wanderbot-auto/AgentBaton/actions/runs/40458672955/artifacts/12713549675 | 9411e1c69633c9c671bcbe54510f029110a6e13ab62cffc696d57e3b6d2c546e | Linker ad hoc; no TeamID/DeveloperID/notarization |
| Intel | https://github.com/wanderbot-auto/AgentBaton/actions/runs/40458672955/artifacts/12713957932 | 15ad5e4c1b3c7d12d825851a68c4046f88f33247ef14d1bc025b28ca3dc22292 | Unsigned, no notarization |

Both archives were actually downloaded, API-digest matched, safe-ZIP/CRC/bounded-member checked and DMG UDIF/footer+SHA256SUMS/build-record commit/architecture verified. Full build/signature records are in PACKAGES_58B5.json and each archive. Preserved per-architecture DEVELOPMENT_ACCEPTANCE_58B5 checklists are manual checks, not a claim they ran on the user's machine.

Actual macOS15.7.9 and Xcode16.4; Rust1.99.1. Apple Silicon Mach-O minOS metadata is11.0; the app config alone does not prove lower-OS support. No minimumOS or user-machine installation verification, formal signature, OSOwner dialog or real Provider/model acceptance is claimed. Both actual macOS core14/native11/harness6/WebView/readonly6/lifecycle/default-release-marker audits passed, with real587msARM/665msIntel admission under unchanged1000ms. Physical capture tab timing limitations are in NATIVE_58B5_REVIEW.md.

This milestone implements deterministic-command validation/cancellation/recovery and genuine safe-Git inspection performance improvement. FullDesignv0.2 continues: root/project command-budget integration, complete captured-stream history, Goal runtime/planner/delegation/adoption and remaining requirements are incomplete or unverified. Production/signing/private accounts/new spending/final license remain separately gated.
