# Current corrected evidence

Independent review OS-HARNESS-1 caught bc0 querying nonexistent goal_plans; old manually constructed test schema masked it. This was a meaningful test limitation, now fixed without Core changes or if-exists fallback. OS-HARNESS-2 corrected cleanup success ordering, exit/signal/group ownership proof and Rust capture child guard. OS-HISTORY-3 added exact per-event sequence/order.

Actually run on Linux after corrections:

- `AGENTBATON_OWNER_FIXTURE_DAEMON=/workspace/.onboarding/goal-graph-owner5541-native94157/batond python3 -m unittest discover -s apps/desktop/src-tauri/tests -p 'test_native*.py' -v`:36 tests passed,0 failures/errors/skips,5.151s. Log `/workspace/.onboarding/native-owner-dialog-smoke-fix-final/python-tests.log`.
- Same new-file-only suite:11 passed,2.608s; real Core-created database is required, no optional skip. All original25 runner test bytes unchanged.
- Direct rustfmt --edition2021 --check on three new Rust files, py_compile of runner/new Python tests, git diff --check:exit0. Rust formatting is not compilation.
- Exact comparison: seven frozen product/readonly source files unchanged; Cargo lock still only the two authorized root edges, every package/version/checksum/other edge identical.

Actual immutable binary used: batond SHA `fc5d33dca48d4b46dc7c5aa8eaf2235be8222f04ea959ba139561c0fa380d77e`, Root94157/Core5541 schema12 provenance in `/workspace/.onboarding/goal-graph-owner5541-native94157/manifest.json`. No build/cache write. Actual generated local --version connection and pending Grant, actual read-only DB, schema12/sequence8/19zero counts. Transcript excludes auth token. Daemon was alive before cleanup, received owned-group SIGTERM, actually exited0, group quiescent before leader reap, no errors/forced kill. This is pure real Core bootstrap, no OSdialog or Grant finalize.

Old bc0.database actually reproduced `sqlite3.OperationalError: no such table: goal_plans` on this real migrated database; log `/workspace/.onboarding/native-owner-dialog-smoke-schema-fix/old-bc0-real-core-failure.log` SHA `d95225ac6a4d3144e2021a63ac3383e24b7f7546eda5c23642f1bf0a584658f2`. Old bc0/old32 green/independent report preserved; do not call that old test a real schema check.

Current source SHA-256:

- `apps/desktop/src-tauri/src/bin/native_owner_dialog_smoke.rs` `553b4bd415dd6a794256bf02d4f460ec0f276660ee25eb87472e33c30ab87d19`
- `apps/desktop/src-tauri/src/bin/native_owner_dialog/controller.rs` `3df3613f7f73ebed47a339aba6d3c3aef197f7e3cebd0f33ff61d9bf1d68d724`
- `apps/desktop/src-tauri/src/bin/native_owner_dialog/macos.rs` `e032eb04c604edeed76a074009197f2c0be9ca0097eb5cc2921559a97c537cae`
- `scripts/native-owner-dialog-smoke.py` `77d52009365d4576e87838cbdb701980101121337fb8a607ea7faa92db33e6c8`
- `apps/desktop/src-tauri/tests/test_native_owner_dialog.py` `d3cd8d55001b229de5c229f6efa6e3cf2673de82c27cba7afb846dfd948e6403`
- `apps/desktop/src-tauri/Cargo.toml` `5f7276d2bdbd99925e1aba38759fccffe8add0e8f62eacc055e2301f0462f723`
- `apps/desktop/src-tauri/Cargo.lock` `9a74078ffe6b6ff7e1382ee5db9ddae49395c3660d722d29893b6ed663c8082c`

Current exact artifact hashes are in `/workspace/.onboarding/native-owner-dialog-smoke-fix-final/manifest.json`. Actual raw bootstrap/cleanup record and readonly DB snapshot remain there; no secrets/production credentials. Mac OS dialog/driver/real screenshot/Grant approval and Rust compile/lints still NOT_RUN. No provider/model/account/OS permission/extra resource/signature/release activity.

# Historical bc0 source-only evidence (superseded, retained)

# Evidence

Candidate source fingerprints (only source checks/runner controls; no OS evidence):

- `apps/desktop/src-tauri/src/bin/native_owner_dialog_smoke.rs` SHA-256 `553b4bd415dd6a794256bf02d4f460ec0f276660ee25eb87472e33c30ab87d19`
- `apps/desktop/src-tauri/src/bin/native_owner_dialog/controller.rs` SHA-256 `3df3613f7f73ebed47a339aba6d3c3aef197f7e3cebd0f33ff61d9bf1d68d724`
- `apps/desktop/src-tauri/src/bin/native_owner_dialog/macos.rs` SHA-256 `468aeadfd85b2e44039d4a8725b0bba1b9744046829ad66644b95bdc09e77367`
- `scripts/native-owner-dialog-smoke.py` SHA-256 `d664bb2cd6749372b007e56200cc1c4b5eccaa5deddf767ecee8d64bf19bd4be`
- `apps/desktop/src-tauri/tests/test_native_owner_dialog.py` SHA-256 `75d503d48e76ae7545f043caa3ecdc6045865126525219aa0fab80babebcb4e4`
- `apps/desktop/src-tauri/Cargo.toml` SHA-256 `5f7276d2bdbd99925e1aba38759fccffe8add0e8f62eacc055e2301f0462f723`
- `apps/desktop/src-tauri/Cargo.lock` SHA-256 `9a74078ffe6b6ff7e1382ee5db9ddae49395c3660d722d29893b6ed663c8082c`

Actually executed in this tree:

- `python3 -m unittest discover -s apps/desktop/src-tauri/tests -p 'test_native*.py' -v`: exit0, 32 passed, no skips on Linux. Original25 test bodies/file untouched; seven new controls include actual readonly SQLite fixture observations plus schema/count/CAS/event/foreign-scope/host-no-Grant-finalize controls, and Linux explicit not_run.
- `python3 -m unittest discover -s apps/desktop/src-tauri/tests -p test_native_owner_dialog.py -v`: subsequent exit0, seven passed.
- `RUSTUP_HOME=/workspace/.toolchains/rustup /workspace/.toolchains/cargo/bin/rustfmt --edition 2021 --check apps/desktop/src-tauri/src/bin/native_owner_dialog_smoke.rs apps/desktop/src-tauri/src/bin/native_owner_dialog/controller.rs apps/desktop/src-tauri/src/bin/native_owner_dialog/macos.rs`: exit0; syntax/format only, not type checking.
- `python3 -m py_compile scripts/native-owner-dialog-smoke.py apps/desktop/src-tauri/tests/test_native_owner_dialog.py`: exit0.
- `git diff --check`: exit0.
- Python TOML/source comparison to ccec: desktop lock only adds objc2-app-kit and objc2-foundation root edges; all packages/version/checksum/other edges equal. Existing single versions objc2=0.6.5, AppKit/Foundation=0.3.2 verified; selected features all actually exist. Seven frozen lib/approval/native_parent/old native_smoke/runner/mac script/Python files exactly equal baseline.

Original failed parser evidence: rustfmt exit1 reported `unmatched angle bracket` at macos.rs capture Option type and proposed removing one `>`; source corrected. Original Python run: `Ran 7 tests ... FAILED(errors=1)` at test_expected_message_keeps_full_production_formatter_and_utf8_snapshot -> ValueError `Production full-message formatter changed; audit before OS click`; extractor audited against actual ccec format and corrected, changed-formatter rejection retained. No test was deleted, skipped or lowered to pass.

Locked public AppKit APIs read from actual cached generated sources: NSWindow.attachedSheet/sheetParent/contentView/windowNumber/screen/isVisible/frame; NSView.subviews/window/visibleRect/bounds/isHiddenOrHasHiddenAncestor/displayIfNeeded; NSTextField/NSControl.stringValue/cell; NSCell.cellSizeForBounds; NSButton.title/isEnabled and NSControl.performClick(Option<&AnyObject>). Tauri caller ns_window is retained only on main; control objects dynamically downcast and retained through immediate public click. rfd uses real parented NSAlert; the driver never casts a sheet window to NSAlert or manufactures its completion.

NOT RUN: Cargo compile, four Rust controller tests, Clippy, default/release binary audit, testhost actual Core bootstrap, real Cancel, real same-key OK, macOS Intel/Apple Silicon GUI, screenshots/capture permissions, independent pixel inspection and actual Core grant commit. Existing Parent53lib+22harness results are for prior production parenting and do not verify this driver. No CI trigger or new limits. No Provider/model calls, private credentials, OS permission changes, signing or production release.
