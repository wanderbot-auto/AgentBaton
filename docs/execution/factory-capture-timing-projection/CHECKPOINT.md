# Checkpoint

Tree `/workspace/.worktrees/factory-capture-timing-projection`, branch `codex/factory-capture-timing-projection`, baseline `34f8b23d4fdd529caf4f7da6f949604f2746e0e9`. Source changes are limited to the collector and its dedicated tests; own records occupy this directory. No PR, remote write or merge was performed.

Functional change: nine collector lines add only the six-field capture allowlist and diagnostic availability marker. Original event association, previous-node relationship, schema15 consumer, post-shutdown snapshot label, readonly access/FD/path/size limits, public capacity, budget accounting and not_a_product_or_test_pass flags remain intact. All prior functions except event_fields are byte-identical; removing the new event-field statements restores that function's original AST. All old test classes are byte-identical.

Exact final script SHA256: `2e997e922ee3d807d6b37b77c67647de62c0acf0d359b62bc96791850f13bffe`.
Exact final test SHA256: `44b3b5a39ec5af02eccb81f8a61c267c8d38b4d9befd9e02fa3fde824be01e1b`.

Dedicated72-method pure suite and syntax/source comparisons pass; git diff --check passes. Independent review and genuine runtime/event-producer/remote CI/macOS validation are NOT_RUN in this leaf. Numeric diagnostics never establish admission-time state from the later shutdown snapshot. Root should review this bounded slice, integrate if accepted and retain the full product's remaining requirements and gated Provider/spend/signing/license limitations.
