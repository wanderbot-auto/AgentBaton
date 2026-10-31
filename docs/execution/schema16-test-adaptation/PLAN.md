# Schema16 test adaptation

Requirements: AB-SEC-007, AB-PRJ-002, AB-PRJ-003, AB-PRJ-005. Baseline `61c730ba4fdb5bdd0858cb4fc52ee07d29dce6c8`.

Adapt only current-version tests to the existing semantic migration16. Preserve genuine14 generators, raw inputs and manifests, production source and permissions. Use strict inverse fixture validation before rebuilding legacy DDL; never strip a workspace binding or rewrite unknown Task JSON. Primary performs independent review and real Rust execution.

Decision: physical Factory/Goal DDL still uses original schema15 constants because migration16 adds no DDL. Inverse16→14 is allowed only after exact current schema, empty12Factory tables, exact indexes/FKs and old-representable Task semantics pass in an active transaction. This helper produces synthetic old fixtures; it does not manufacture historical14 evidence.


Revision: explicitly require a JSON object before decoding Task. Serde struct-sequence compatibility alone cannot establish old10-field semantics when a complete current11-slot array includes a null workspace-binding slot. No production migration changes.
