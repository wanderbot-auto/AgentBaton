# Progress

Implemented source separation between exact known-object validation and conservative opaque extras. All known definitions must still appear identically after the existing formatting-only normalization. Extra names collide with known names using SQLite ASCII case semantics; Integration* remains reserved. Extras may only be ordinary main tables without FKs/hidden generated columns, plus their own ordinary nonpartial, nonexpression column indexes (including genuine own unique/PK autoindexes). Unknown views, triggers, virtual/shadow tables, authority-table indexes and other sqlite_ objects are refused.

Added source tests for generated16→17 plus17 reopen preserving raw Task/opaque typed bytes, sparse rowids, quoted names, index DDL and private backup;20 negative variants at both16 and17; virtual/shadow refusal at both versions; and transaction rollback if additive17 exceeds the unchanged256-object budget. These are not executed results.

Original command_validation_migration positive/negative tests and historical producers remain untouched. Root reported the original schema7 positive failed at baseline with INPUT_CHANGED because of original_blob; this branch addresses that specific structural rejection. Source author did not rerun it.

Formatting and diff whitespace checks passed. Cargo, SQL execution, Provider, Actor, Node/browser, network, native macOS and CI were not run here. Independent review and all runtime claims remain pending Root.

Independent review found the unparenthesized pragma_table_list SELECT source could resolve to an allowed opaque ordinary table. Replaced that source with direct PRAGMA main.table_list, retaining safe ordinary same-name data. Added RTREE virtual/shadow forgery refusal at both16 and17 and expanded the positive roundtrip with a harmless same-named table. Five added test functions now exist; all remain not_run by the source author. Review fix awaits independent re-review.
