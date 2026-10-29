# ADR: schema14 is an exact readonly contract with two independent plan counters

The real Core migration adds factory_child_plan_heads and factory_child_plan_revisions. Merely changing the version literal would leave the old nine-table readonly query blind to retained plans. Both tables therefore join the fixed count tuple, each must independently be integer zero, and missing tables must fail. The count snapshot has eleven counters plus schema and sequence. Core's named JSON declaration order does not matter; the exact field set and SQL-to-field mapping do.

Require only schema14, rejecting13 and15. Update existing fixtures/negative arrays instead of accepting a range, removing tests or relabeling a schema13 tool. Old readonly counts, journal interval, phase/order/refusal and owned-process assertions remain. This is no new planning/Goal/workspace authority and does not relax permissions or timing.

Core owns schema/counter SQL; the formal count DTO4b278 is copied unchanged. Root-designated writers coordinate scripts and final manifests. Native approval/lib do not need schema changes and remain frozen8d. Generated SQLite read queries and source-derived projections are useful early checks, but compiled Core/Native and fresh macOS evidence are required separately. No standalone data replay or CLI build establishes Native success.
