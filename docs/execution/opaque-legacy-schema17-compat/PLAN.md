# Opaque legacy schema17 compatibility

Baseline: `4f962940533241fdec74b96a87c01a99788a62ee`; branch `codex/opaque-legacy-schema17-compat`. Scope: AB-SEC-007 (WP12 database migration protection), supporting WP02 persistence transaction integrity under AB-START-001. ACK OWNER-001/002; no change to authority, UI, schema version, Provider, spend, signing or release permissions.

1. Keep trusted schema16/17 known-object proof exact, including all55/59 tables and all reference constraints/indexes/triggers.
2. Classify only extra ordinary main tables with no FK/generated columns and their own pure-column indexes as opaque; do not select, interpret or rewrite opaque records.
3. Keep256-object/1MiB definition bounds, full FK check, typed Task validation, transaction and private backup boundaries. Check the resulting17 schema budget before commit too.
4. Add generated compatibility positives and security/rollback negatives. Preserve the original schema7 positive unchanged; do not label generated inputs genuine historical producers.
5. Source author runs formatting/static checks only. Root owns independent review, original schema7 regression, actual SQLite/Cargo migration tests and CI.
