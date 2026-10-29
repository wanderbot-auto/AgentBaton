# Actual progress

Sourcec0a31a0da116694a95dd00036bc46c8ddc528463 changes exactly native_parent.rs, lib.rs and native_owner_dialog_test/controller.rs.16 finite source checks, standalone Rustfmt/parser and diff check passed; no source change during these commands. Full inverse-byte comparisons prove parent changes are visibility only, lib adds only the feature-gated readonly aliases, controller adds only the import. All original10+new6 Rust test bodies remain byte-identical to6002.

Root's original failed Cargo log is preserved byte-for-byte:5 E0425 diagnostics for REVIEW_MESSAGE_BYTES/REVIEW_ROW_ENCODED_BYTES/REVIEW_ROWS_PER_PAGE, exit101,0tests. Runner reports original tool inode/mode/link-count/hash and tracked source restored. This is an implementation omission missed by pure tests/format checking; it is not a passing Rust test result. Old9da/6002 review/evidence remain unchanged.

Separately, Root actually ran Python47 (including5 actual-Core cases) on62518: unittest8.247s, exit0. Both Python files are byte-identical to this source, but these results are not borrowed as compilation, Native controller, current readonly-view or macOS proof. This author only verified/read the exact Root logs/hashes and did not independently execute that group. New Rust16/type/Clippy/AppKit/macOS remain NOT_RUN until Root's next lease.
