# ADR — Separate trusted authority schema from isolated legacy data

Status: implemented source, pending independent review and runtime evidence. Parent approved the minimal policy before implementation.

Problem: comparing the entire database against an exact55/59-table reference mistakes an uninterpreted historical original_blob table for incomplete authority schema. The retained original schema7 migration acceptance exposes this production compatibility failure.

Decision: prove every known reference object exactly as before, but classify remaining schema metadata separately. Accept only isolated ordinary main tables without foreign keys/generated columns and their own nonpartial pure-column indexes; forbid case-insensitive known-name collisions and Integration* names. All other unknown object classes are refused. Bound the complete input and resulting output schema using the unchanged caps, keep global FK and Task authority checks, and preserve original transactions/backups.

No opaque content is parsed as policy or authority. Unknown data can resemble permission JSON without creating records. This avoids arbitrary other-domain prefixes or a general legacy schema migration framework. The conservative exclusions may reject richer unknown databases; support for them requires a separate evidence-backed decision. Historical generated fixture provenance remains explicitly generated, not genuine predecessor evidence.


Review correction: ordinary-table classification uses direct `PRAGMA main.table_list`, never a SELECT source named pragma_table_list. SQLite may resolve an ordinary same-named table before its eponymous PRAGMA module; its stored rows must not supply trusted object kinds. A harmless ordinary table named pragma_table_list remains permitted and its rows remain opaque. The direct PRAGMA reports real virtual/shadow types regardless of that table's contents. Existing explicit metadata TVFs fail closed if their names are shadowed; no new metadata namespace grants authority.

Added a generated RTREE input with no hidden/generated columns or FKs, plus an ordinary pragma_table_list table claiming every object is ordinary. Both16 migration and17 reopen must reject with logical/raw/WAL/backup preservation. The positive roundtrip separately includes the harmless same-named ordinary table. These are source assertions pending actual execution.
