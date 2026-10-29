# Progress

Sourcea9baaa49b2b259be1e93d1e9894441be02b3095b changes only the Native smoke test bin. Exact schema14 replaces13; old13/future15 rejection is explicit and older10/11/12 controls remain. Factory SQL reads both new tables, countfields grow9→11 and the snapshot has13 keys including schema/sequence. The Core named-count projection expects11. Existing30 Rust test functions are retained with three schema-label renames; nonzero/missing controls now cover both new tables.

188 pure controls pass, including29 actual readonly embedded-SQL subprocess queries on generated minimal SQLite files. Both plan table mutations are observed; missing tables produce real query failures; every query preserves file hashes. Rust predicate projections are source-derived Python checks, not execution of Native functions. Rustfmt parses/checks the source and diff exits0. Core declaration field order differs from query presentation order; names match exactly.

Initial DDL capture assumptions and two checker failures are preserved, corrected only in collection/validation code. Approval/lib remain byte-identical to8d. No Cargo, CoreStore14, Native Rust30/planning8, AppKit/browser/Mac/CI was run. Independent review and the later integrated Native lease remain pending.
