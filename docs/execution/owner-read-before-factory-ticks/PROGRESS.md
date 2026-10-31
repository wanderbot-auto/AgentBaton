# Progress

Source-only candidate on `codex/owner-read-before-factory-ticks`, based on f55a805a952f84dc65e83c5e33f98e975871c3f1. `ipc.rs` now observes one already queued FIFO head after the original worker/fatal pass. Only the exact Owner read envelope runs early. All other methods stay owned in one stack `Option<ReadyRequest>` until the normal drain and precede every later request. Early service counts as one of the original sixteen requests.

Original post-request statements and comments were moved to one private helper without changes except indentation. Early service immediately propagates any retained fatal Factory error before entering further ticks. Original normal drain error propagation and shutdown remain unchanged.

Rustfmt and static byte comparisons ran successfully. New Rust tests are authored, not compiled or executed by this source author. No Cargo, Actor, SQLite, browser, Provider or network work ran in this work unit. Root must independently review and actually execute focused and product regressions.

Diagnostic source limitation: f55 Intel's final read used 5004 ms and exhausted the existing shared two-retry pool after one classified busy and one earlier transport retry. Retained post-shutdown child acceptance/validator success does not prove the parent was complete at the failed observation. Queue delay is an unproved candidate, not a diagnosed cause. CI used dev/test debuginfo, but the observed CI CLI was smaller than the local Linux fixture; no debug-symbol/hash explanation is established.


## Root actual checkpoint 2026-10-31T00:21:23.878308+00:00

Actual source cbb16c7: Core180, Factory33, strictClippy/fmt and same-source normal browser4checks/4PNG pass. Test-only b183c86 fullinfra284 pass after retained NameError. Exact commands, immutable log/result/input hashes and f55 failed CI are in ROOT_RESULTS.json (SHA256 3a2e5cd21f2a3b00967b69e0543ab8348484f1c2af69ace2626a2760a7ebf627). New exact CI, dualMac/Native/audit/installer and full130 acceptance remain open; no deadline, retry, budget or authority expansion.


## Outer stop/read-cycle and same-click Owner observation verified — 2026-10-31T03:09:30.745908+00:00

Source033 Core182/Factory33/Clippy and source589 UI919/55 retain exact attribution; ec5harness uses actual9f Core/frontend bytes. Normal4checks4PNG, cancel4checks3PNG, loststart5checks4PNG allpass with completeownedcleanup; fullinfra284/fmt pass. Keep original9f timeout/cleanupfalse, bd pure1fail1error, firstcapacitypreflightrefusal and old52remoteIntel/UI failures. No original4s/15s/TTL/permission/singleworker change. New exactsevenCI/Mac/audit/installers/protectedintegration/full130 remain open. See OUTER_STOP_READ_CYCLE_OWNER_ORDER_RESULTS.json SHA256 5cbbcf60389f7f839a96044c4add29300c0f948acca6ab0f5458c71542955ce1.
