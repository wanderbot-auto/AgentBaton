# Independent review — real Native Grant dialog candidate bc0a54b

**Withheld: three concrete test/evidence findings remain in this immutable candidate.** Source `bc0a54b92871950663a6aa6848a04743e49926d7`, baseline `ccec51536a60e247b924d10b5cead25b30307e60`. No Cargo/target/CI, OS permission, production or source edit was performed. Earlier Native parent erratum and other reports were not modified.

## Actual verification and its limit

Reviewer independently re-ran the immutable candidate Python snapshot: **32 passed,0 skipped,2.570s onLinux**. These are the original25 runner controls plus7 new controller/read-only synthetic SQLite controls. They do not execute real Core bootstrap or an OSdialog. In fact their invented goal_plans table masks findingOS-HARNESS-1 below. Four pureRust controller tests and all macOS code remain uncompiled/unrun. Git diff --check passes. Seven frozen files (lib/approval/native_parent/oldsmoke/oldrunner/macscript/oldPython) are whole-file byte-identical; parsed lock comparison proves only the two existing AppKit/Foundation root edges were added, with every package/version/checksum and other edge unchanged.

## Findings

### OS-HARNESS-1

Locations: `scripts/native-owner-dialog-smoke.py:85-86,98,117, tests/test_native_owner_dialog.py fake COUNTS/DDL, crates/core/src/goal_store.rs:1372-1380`. Requirements: AB-SEC-002, AB-REL-001.

Runner queries goal_plans, which does not exist in real schema12. Real Goal migration has goal_plan_heads/revisions/proposals. First database observation after public bootstrap fails before OS dialog. The7 unit controls create a fake goal_plans table, so32 green does not catch incompatibility.

Required control: Use exact fixed real migration tables including actual Graph namespace where zero-Graph is claimed; never skip missing tables. Run public fixture bootstrap against an explicitly supplied immutable real schema12 daemon, with no model/session, and add actual namespace/missing-table/mutation negatives. Keep original failure evidence and fake-fixture coverage labelled.

### OS-HARNESS-2

Locations: `scripts/native-owner-dialog-smoke.py:262-279, native_owner_dialog/macos.rs:270-302`. Requirements: AB-SEC-006, AB-REL-001.

Success result is written before cleanup, daemon returncode is ignored, and cleanup SIGTERM/SIGKILL use is not recorded or disqualifying. A daemon exiting1 after Grant commit may still let runner exit0. Rust screenshot child try_wait Err uses? immediately and can drop a live Child without kill/wait; after app exits Python may skip its process group because leader already ended.

Required control: Retain/reap owned screenshot child on all postspawn errors. Record actual app/daemon status and signals; unexpected exit or required forcekill must retain failure, not a success file. Only publish success after cleanup succeeds. Add real subprocess abnormal-daemon/cleanup-failure controls without OS permission changes.

### OS-HISTORY-3

Locations: `scripts/native-owner-dialog-smoke.py:130-137, tests/test_native_owner_dialog.py approved()`. Requirements: AB-SEC-002, AB-REL-001.

approved_error verifies overall sequence=baseline+2 and matching source/count, but never each matching event sequence. Its synthetic positive events omit sequence entirely and pass. Old/moved/swapped matching events can satisfy this verifier without proving the two new records at this approval boundary.

Required control: Check real Core event order against source, then require Grant-approved sequence=baseline+1 and Owner-confirmed=baseline+2; missing/old/swapped/duplicate/source-tampered events refuse. Do not infer exact authority chain from source labels plus aggregate counter.

## Static production/test boundary

The new binary is required-feature native-smoke and has a release compile_error; the existing library also rejects native-smoke in release. Default production registration is unchanged; only the separate test bin adds owner_dialog_report. It forwards grants_create to the actual production dispatcher and caller-owned dialog path. The host allowlist cannot directly finalize a Grant, only its generated bounded --version connection; no session/model/account is launched by thatbootstrap. Actual default/release artifact exclusion must still be audited on built binaries, including new test identifiers rather than borrowing oldaudit success.

The driver observes the actual Tauri caller NSWindow and only its attachedSheet with matching sheetParent. It does not enumerate otherprocesses or cast NSWindow to NSAlert. It checks full exact expected message/title, one real enabled visible OK and Cancel, complete text sizing/visibleRect, stable positive sheetnumber and fit within visible screen; any clipping must fail honestly. After raw exact-sheet screencapture succeeds, it re-observes before public NSButton.performClick(None), and requires a phase/proposal/revision/baseline-bound host permit based on readonlyDB. The intended Cancel→samekey OK path does not replace a completion callback or call private Grant finalize fromhost.

Typed public AppKit/Foundation methods were checked against locked cached generated source, including method safety signatures and feature gates. NSArray iteration is supported by objc2’s&Retained blanket iterator and existing Wry/rfd NSEnumerator feature; no visible API error was identified, but only real Mac compilation can prove types/features. Actual screenshot remains raw WindowServer PNG, not reconstructed text or a browser image. Image dimensions/header are checks only; independent visual inspection remains explicitly pending. No AX/TCC/Accessibility/global input injection or fakehumanconsent is granted.

## Remaining acceptance

Fix the three findings with actual controls using real immutable schema12 Core bootstrap and real subprocess failures. Then compile the4pureRust tests and Mac-specific binary under a proper lease; run both existingMac architectures, realCancel/samekeyOK, before/afterDB exact journal evidence and actual PNG inspection in the already-approved CI. This review did not triggerCI or runMac. An automated Grant test cannot stand for GraphOwner, TaskAccepted, human approval or fullDesign completion; original readonlyruntime/harness remains separate. Exact source hashes, dependency API snapshots and logs are preserved in VERIFICATION.json/source-binding.json.
