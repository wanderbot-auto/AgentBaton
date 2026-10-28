# PR14 37BD independent CI review

**BLOCKED — final 5 success / 2 failure.** Product run [40669158389](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40669158389) failed; Environment [40669158380](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40669158380) succeeded. Both are actual attempt 1 for exact head `37bd032f0e1b2dfa75fc9ff768e85801f1693a99`. PR14 remains open, ready, mergeable-state unstable; no merge is approved by this review.

## Exact subject and independently retained evidence

GitHub API commit objects and completed checkout raw logs establish actual checkout `e79a267584d1dcbb4f329b1e3044e5c66eb9a6bb`, parents `[18f4261da9285bec5446101d47786839b1020d9c, 37bd032f0e1b2dfa75fc9ff768e85801f1693a99]`, and equal source/checkout tree `1ae60dd9e162d064d5e6ebe45be481112a294e35`. All seven final check-runs bind this head. Source copies were read with `git show HEAD:path` using the exact literal subject rather than the mutable working tree. The source/hash inventory is in `source-and-checkout-verification.json` plus `failure-source-verification.json`.

The three Environment jobs (122751238474 Intel, 122751238776 ARM, 122751238952 Ubuntu) passed; retained raws show actual infrastructure 34 tests. Product Rust 122751238850 and UI 122751238576 passed. Rust raw SHA-256 is `a3c172485455ccce69032d3a8ade832b704cde20412947b3545b859c40ead62e`; UI is `4447505fb90ff8da228981eca8e43c0a44033b20d19a97e89f6234c26d9b9a86`.

## Both real macOS failures

Both jobs failed during step 6, `Core and native host tests`, while executing `cargo test --workspace --locked -- --show-output`. The failing executable is `crates/workspaces/tests/data_workspace.rs`: 23 passed / 1 failed. The exact test is `full_internal_target_path_budget_is_rejected_before_reserving_any_directory`. In both raws, `Fixture::put` panics at line 36:52 in `fs::create_dir_all(path.parent().unwrap()).unwrap()` with `Os { code: 63, kind: InvalidFilename, message: "File name too long" }`. This occurs while preparing source directories, before `f.snapshot` or the product `create_data_workspace(...).err() == ResourceLimit` assertion. No product ResourceLimit result or destination-empty assertion was reached in that test. The exact fixture source SHA-256 is `b61b0896ac373d98ade5c444da314af1dbbce4d85a5205a863f8aa4cf2657157`.

The saved source constructs sixteen components against a 4090-byte source-path target and later asserts a destination exceeding 4096 bytes. The direct observed cause is OS refusal of fixture directory creation on both Mac runners. This is a different failure from DE150; it is not evidence that the product path guard itself accepted an unsafe path. A platform-valid fixture/contract must be reviewed without removing the ResourceLimit or no-reservation assertions.

| Actual job | Raw SHA-256 | Failed raw lines |
|---|---|---|
| ARM 122751238994 | `54321ddf9b21c0ee29a235caf3e075d2d44d84e87f5e43cf128ba1ac4772025e` | 1699–1711 |
| Intel 122751238886 | `abff8c77f929268f0536c0d27153e831a0c61ca3b0f4e388b52002bd83654f76` | 1801–1811 |

## New preparation observer actually passed on both Macs

Both new Core Data executables completed 21 tests successfully, including the exact 0DB 32MiB cancellation case; migration executable completed its three tests successfully. The nested process-exit child is not an extra unique top-level Data test. Reviewed Core fixture SHA-256 remains `17178a8c943fa0c5225bb4fd543a8ef50281808458da031b59fe5ea5c54043dd`.

| Observation (milliseconds) | ARM | Intel |
|---|---:|---:|
| preview start RPC | 54 | 41 |
| parallel system snapshot / preview get | 27 / 28 | 17 / 17 |
| preparation, terminal verified | 3748 | 4869 |
| subsequent copy cancel / join | 2104 | 2352 |

Each emitted terminal copy result is `cancelled` with `io_receipt="worker_joined"`. The existing tests enforce preparation ≤35s, each measured RPC <3s, and existing copy/join 5s budget; no production budget is changed by this evidence collection. Full64 Core preview/proposal serialization also passed with emitted 35222/35486-byte proposal JSON and explicit `native_dialog_not_executed`. This does not establish an actual OS Owner dialog or actual64-file copy. Historical DE150 preparation root cause remains unknown.

## Fresh Linux UI evidence and limits

The actual new UI run reports 367 unit tests passed plus fresh real-IPC Data eight checks. Independently downloaded current artifact **12806619258** (`frontend-real-ipc-e79a267584d1dcbb4f329b1e3044e5c66eb9a6bb`) has API/local ZIP SHA-256 `3a46c9a55e40f43fdead3f197acbca5676b97326d0415b0c159128b1b8b1204f`; strict safe ZIP/CRC validation passed for 57 entries. Data report SHA-256 is `8538f299c0f026da2bd71aed27615584550d1ed666d8ad0787fbcbe32eaa8c3e`. `LINUX_DATA_UI_VERIFICATION.json` records built React/Chromium/test-only HTTP bridge to real Unix schema10 daemon: actual Ready 2 files/39 bytes, worker joined, journal sequences 8<9<10; actual cancellation `cancelled + worker_joined` before destination reservation, no selected partial output, attempts/sessions/dispatch_intents all zero. This is not an in-copy partial-stop observation, actual OS approval dialog, macOS native screenshot validation, or real Provider acceptance.

## Explicitly not reached

The workspace failure short-circuited the remainder of step 6: native-host 22 library/12 harness tests were not executed. WebView acceptance step 7, application build, default-release test-control audit, package creation and development upload were skipped. Both always-run native upload steps only warned `No files were found ... artifacts/native-smoke/`; an upload step success is not a native artifact pass. Final artifacts API contains only current provider-contract 12807976661 and frontend 12806619258. There is no new native PNG, no sidebar visual result, no Goal3 independent DB/Command3/Data3/native lifecycle evidence, no current DMG pair and no current signing record. The success-only package helper was deliberately not executed. Local copies of audit helpers are unexecuted preparation, not passing evidence.

Old E384, BF, DE150 artifacts/checks/screenshots were not used to satisfy this head. This report makes no actual Owner-dialog, live Provider, minimum macOS, user installation, formal signing/notarization, production release or whole-design completion claim.

## Independent scope, commands and next step

Read-only commands were `gh api repos/wanderbot-auto/AgentBaton/actions/runs/40669158389` and `/40669158380` with `/jobs` and Product `/artifacts`; `gh api .../actions/jobs/<job>/logs`; exact commit/check-run and PR14 API reads; exact `git show 37bd032f0e1b2dfa75fc9ff768e85801f1693a99:<path>` source reads; bounded read-only monitoring, SHA-256 and safe ZIP/CRC inspection. The watcher naturally exited when all Product jobs completed. No rerun, cancellation, ready toggle, CI edit, push, merge, source/index/Cargo write, credential or account operation was performed. No Cargo target is held.

Return the platform-specific fixture failure for an independently reviewed correction, preserve these failed runs and original raw hashes, then require new exact seven checks, both real native lanes and freshly verified dual development packages before merge. Evidence manifest hashes all retained current files; no failure is reclassified as success.
