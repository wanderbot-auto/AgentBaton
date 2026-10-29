# Actual progress

Source `9da4ce7d88b497527cd5c70c987dc5acc2f28199` introduced five permitted test files.38 pure controls passed (26 prior pure +12 new), with37 preservation checks. Original30 Python test methods and original10 Rust controller tests were preserved. An independent fixed-source review identified two must-fix evidence gaps: the non-target approval result column was omitted, and Factory Native executable path/hash/stability was not recorded. This stage remains blocked historical evidence and is not overwritten.

Source `6002b10f7d0522791efef05a3461d7e6d68bb99b` changes only the two Python files to close those gaps. It records complete typed/raw other-proposal rows and the actual canonical Native executable metadata/hash around the run and both phases. Native resource location is preserved; final successful result is written only after the last stability check. Original failure survives cleanup/postcheck errors; secondary diagnostics are separate.

Actual current controls:42 pure Python pass, with all43 prior test-method ASTs unchanged and4 new SQL/file/cleanup controls. Full discovery47 includes5 actual-Core cases explicitly NOT_RUN by this leaf. The new real-Core case prepares a real complete schema14 Factory proposal and explicitly never host-finalizes the target; it has not been executed. Rust source has16 controller test markers (old10 +new6), all NOT_RUN here. Rustfmt/parser check passes; it is not Rust type checking or AppKit compilation.

An exact-SQL generated one-table control reproduced6 result-only mutation failures on byte-exact old9da source; the fixed read-set detects all6. Document consumed/expiry changes were already detected before and after (not claimed as new failures). These are real SQLite unit queries on synthetic data, not a migration/runtime/Owner test.

Kept failures: unsupported sparse-checkout add option (no captured per-command exit); a broad test selector reached the new actual-Core test's missing daemon environment lookup (12pure passed,1error, no daemon spawned); first preservation summary accidentally counted an overwritten string variable, later corrected from125 to43. All original logs remain. Current final command results bind exact source6002 and show no tracked source changes.

CI and audit changes are proposals only, not applied by this leaf. Tiny shell/strings controls on generated text reproduce the old scanner's acceptance of the two new test-only markers; the proposed scanner passes8 strict cases. This does not scan an actual default application or prove release isolation dynamically.

Independent fixed6002 closure: both P2 findings closed; reviewer independently ran42pure,6 result-only cases through both pending/approved guards, and7 metadata/order controls. These counts overlap author/unit coverage and are not added to a product-test total. Review status is SOURCE_READY_RUNTIME_PENDING; no actual Core/Rust/OS result is borrowed.
