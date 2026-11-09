# Integration Core lint repair

Resolve the strict Clippy findings against `0cee7d57907af547880db1367c0386c4bc108950` without changing integration permissions, wire fields, budgets, SQL, receipts, or child ownership. This supports the existing AB-VAL-009 implementation and AB-TSK-003/004/005 resource boundaries; it is not additional capability acceptance. OWNER-001/002 and AB-START-001 remain unchanged.

Use transparent Box payloads for large enum/error layouts, a schema-row tuple alias, equivalent encode/unit expressions, and move four complete cfg(test) modules to the end. Preserve original context/error handles and all real reap/join/writer-lock order. Root additionally authorized only the required integration_targets constructor wrappers and integration_approval exact-selection dereference.

Run standalone rustfmt and mechanical source inverses only. Root owns Cargo/Actor/Native verification and an independent reviewer must inspect the frozen source.
