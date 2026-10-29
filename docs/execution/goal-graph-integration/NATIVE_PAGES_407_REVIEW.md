# Narrow independent re-review — NP87-01 / fixed 407ab9e

**Disposition: NP87-01 is closed at source and independently executed Python-mirror level. No new must-fix in this narrow change.** The newly added Rust regression has not been compiled or executed by this reviewer. Actual macOS page presentation, identity/window-number behavior and clicks remain unverified for this revision. This is not formal approval or product acceptance.

Fixed revision `407ab9e7b014a1af81e39bf90ffac5034708285f`, baseline `87ec9ad8647b549311fc90077e9757bc257c4b70`. The old 87 report/manifest remain unchanged: report `fdf12232867ae5984ce87433ef80126a1e547e8fb2bfec9ac1363656a4302818`, manifest `4ff7c53ee91b0bf7020b11f04e127dc44b675e99a48f4e984641cde748b571b4`. Their copies and original failed boundary log are retained in `evidence/`. No Cargo, daemon, target, product source, remote, STATUS or permissions were touched.

## Finding closure

Original NP87-01 (P2) was the missing explicit escape of Unicode Bidi_Control U+061C ARABIC LETTER MARK. The old representation was byte-lossless but still emitted that invisible direction-control character in displayed rows.

- `apps/desktop/src-tauri/src/native_parent.rs:43` now includes `ch == '\u{061c}'` in the explicit visible-escape branch. The rest of the production parent implementation is byte-identical after removal of that condition and the one added Rust test.
- `scripts/native-owner-dialog-smoke.py:179` adds exactly U+061C to the same explicit branch. Removing that one predicate change yields the complete old runner byte-for-byte. U+2028/U+2029 and ordinary UTF-8 behavior are unchanged.
- The new Rust and Python tests enumerate exactly the 12 Bidi_Control codepoints: 061C, 200E, 200F, 202A–202E, 2066–2069. They require no raw control, visible escape, readable Chinese/emoji and exact reverse reconstruction. The Rust test is static evidence only in this review.
- Independent old/new replay of `recipient: abc<U+061C>123` reproduces the original failure on the fixed old Python source: raw control true / visible escape false. Fixed 407 gives raw control false / visible escape true. Both restore the original UTF-8 bytes exactly. `provenance.json` and the preserved replay script/stdout record this comparison.

The original Python test source is byte-identical after removing only the added bidi test. No earlier test, snapshot limit, short page bound, cancellation/finalization behavior, authorization or threshold was weakened. This delta contains **three changed source files** and **four previously authored 87-era documentation additions** in the cumulative Git diff. The latter are historical 87 records, not new 407 execution evidence; current bidi provenance comes from the separate BIDI12 manifest. No other production source changed.

## Checks independently executed

`commands.json` preserves exact commands, working directories, exit codes and full stdout/stderr. All four commands exited **0**:

1. Five fixed-source PresentationControls: **5 passed**, 2.049s. This is the old four presentation tests plus one new 12-subcase test; the other original daemon-dependent tests were not executed by the reviewer.
2. Two independent finite boundary tests: **2 passed**, 0.001s. The exact previous all-12-bidi test now passes. The separate U+2028/U+2029 visible-escape and reverse-byte check continues to pass.
3. Eleven additional deterministic mixed UTF-8/control/Chinese/emoji vectors pass exact byte reconstruction and encoded-row/page bounds; these are additional vectors, not eleven product integration tests.
4. The provenance/replay command verifies all seven fixed exported files, all **104** author source-map bindings, the three changed source hashes, both original report hashes and source-only delta invariants. The separate fixed-range `git diff --check 87ec9ad… 407ab9e…` exits **0**. (The provenance script and diff check are separate recorded commands; the first command above contains the five tests plus eleven vectors.)

These execute Python mirror/control logic, not compiled Rust or a native GUI. The exact prior negative condition is preserved as old evidence rather than deleted or silently changed to expected success.

## Author evidence and limits

`evidence/BIDI12_MANIFEST.json` SHA-256 `698b08a023d654f83c0052fbd72388fd741363b361f29464df662b48d12e5867` binds fixed 407 and the source hashes. Its actual final `python-bidi12-final2.log` SHA `8573a7fef5e87e369391c95f64759f5d2359ac5c782e6b2579246e61e92e8afe` records **20 tests passed, 6.124s**: previous 19 plus the one 12-subcase bidi regression. Author provenance identifies the unchanged frozen schema12 `40322…` daemon for the original three real bootstrap/SQLite controls. This reviewer did not execute that daemon or borrow those cases as independent native presentation evidence.

The initial `python-bidi12-final.log` SHA `72d21014913955ba3ff02bec42c22b99d7c4da4c50d6288413a86974487be59a` is preserved: the initial new assertion literal caused a unicodeescape SyntaxError, so discovery produced one failed import test and no normal test execution. The corrected assertion uses an explicit backslash character; it does not weaken the runtime escape predicate or assertion. The source-map file is independently checked against fixed Git rather than read as active worktree source.

Author formatting/syntax claims are not converted into Rust execution by this report. The new Rust bidi unit, any expected new Native test count, and Root's separate compile lease remain outside this review's actual execution. In particular the old 87 **61 lib +8 controller** results are not borrowed as tests of 407.

All original Mac limits remain: new revision's AppKit text/cell/onscreen/capture/DB-before-permit/real Cancel and final OK still require actual dual-architecture evidence. If AppKit reuses the previous sheet window number, the controller waits and fails closed at the original watchdog; no successful new-page identity is claimed. Ordinary Chinese/emoji are retained literally, normal RTL text is not newly restricted, and no universal visual-safety or localization completion claim is made. Prior macOS failures, schema13 Factory work, real Provider/model calls, signing, packaging and whole-product acceptance remain separate.
