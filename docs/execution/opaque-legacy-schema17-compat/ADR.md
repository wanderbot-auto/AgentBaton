# ADR — Separate trusted authority schema from isolated legacy data

Status: implemented source, pending independent review and runtime evidence. Parent approved the minimal policy before implementation.

Problem: comparing the entire database against an exact55/59-table reference mistakes an uninterpreted historical original_blob table for incomplete authority schema. The retained original schema7 migration acceptance exposes this production compatibility failure.

Decision: prove every known reference object exactly as before, but classify remaining schema metadata separately. Accept only isolated ordinary main tables without foreign keys/generated columns and their own nonpartial pure-column indexes; forbid case-insensitive known-name collisions and Integration* names. All other unknown object classes are refused. Bound the complete input and resulting output schema using the unchanged caps, keep global FK and Task authority checks, and preserve original transactions/backups.

No opaque content is parsed as policy or authority. Unknown data can resemble permission JSON without creating records. This avoids arbitrary other-domain prefixes or a general legacy schema migration framework. The conservative exclusions may reject richer unknown databases; support for them requires a separate evidence-backed decision. Historical generated fixture provenance remains explicitly generated, not genuine predecessor evidence.
