# Plan

Implement the narrowly approved repeat-observation exception for an already fully verified Factory provider. Base: `fbeb2e96854868735b9c07f27485c5cf8f748502`. Source: `9f412458d782d9513427c2575cc7d234918a2f66`.

Only the existing collector, a new pure test module and these four records change. Core, workflow, waits, budgets, stop/join rules, schema, signals and the existing test module remain unchanged. Requirements: AB-TSK-007/008/009 and AB-VAL-009 evidence; this fixture does not establish a product implementation of those requirements.

## Reversible decision

An exactly unchanged historical provider record may defer one observation of **empty bytes** only when its original parent history and current before/after PID, birth, parent, UID and PGID agree, and actual read-only Core metadata proves cancellation for this exact run/scope/root. The original snapshot is tried first; at most one fresh read-only snapshot per collection may see a later commit. Its SQLite timeout remains 2 seconds. No RPC, retry loop or outer clock reset is added.

DEFER omits the current discovery and makes no verified/ACK/parent-history update or stop/join/acceptance claim. It grants no signal permission. None, first binding, missing proof, changed identity/file/nonce/binding and nonempty incorrect argv retain refusal. The final identity-gone and Core journal checks remain authoritative. Missing/invalid proof fails closed.

Run only pure controls and static checks here. Root must execute the actual cancel browser case against its exact newly frozen candidate and assets; no local heavy test or OS result is claimed.
