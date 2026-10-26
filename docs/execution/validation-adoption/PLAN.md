# Validation and local adoption primitives

Base: 636dbdd on codex/validation-adoption, isolated worktree. Scope: partial AB-VAL-001/002/004/007/009/010/011, AB-PRJ-004/005. Preserve complete acceptance; this unit is not full product acceptance.

Implement bounded descriptor-safe artifact verification with immutable input/output/attempt/validator/policy bindings; reproducibility metadata checks; explicit deterministic acceptance. Build isolated real Git integration candidates without repository command execution; validate exact target head and refuse dirty or checked-out-target ref mutation. Adopt only by local compare-and-swap of an owner-approved non-checked-out branch. Produce explicit development-delivery intents without remote actions; production always needs separately exact-bound confirmation.

Tests use synthetic Git/data roots only: missing/malicious/symlink outputs, source drift, evidence substitution, dirty/diverged targets, malicious merge drivers and exact-head races. No arbitrary validator command execution, stash/reset/force checkout, source index writes, remote push/PR/deploy or Provider calls.
