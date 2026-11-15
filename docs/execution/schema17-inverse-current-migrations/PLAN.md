# Schema17 current migration fixture contract

Requirements AB-VAL-009, AB-PRJ-005, AB-TSK-008 and AB-TSK-010, partial validation work under AB-START-001. Baseline `9b695892a1814dc9daf7e09486e94cbefdc25b2f`; production schema, data, permissions and immutable design are unchanged.

The current Store17 adds exactly four Integration tables. A generated older migration fixture must explicitly verify and remove that empty increment, atomically, before using the original strict16 Factory inverse. Do not relabel a17 database as16 or leave new tables in a fake historical database. The previous strict16 inverse and its future17 refusal controls remain meaningful and unchanged.

Expose a read-only exact Integration17 proof for genuine14/15 consumers. It checks only the complete new domain and incoming dependencies; old nonempty history is allowed and never rewritten. The fixture-only standalone inverse owns one immediate transaction; the caller-owned inverse requires an active transaction. Both reject wrong versions, changed/missing DDL/indexes, extra domain objects, nonempty domains, incoming dependencies or broken FKs. New current17 wrappers compose both increments inside the caller's transaction, retaining original16 checks and SQL.

Root must independently review and actually execute all affected Rust targets, strict Clippy and migration rollback cases. These generated fixtures are never substitutes for genuine pinned historical producers. Actual Provider, Native/macOS and CI dimensions remain separate.
