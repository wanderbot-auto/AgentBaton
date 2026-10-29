# Factory Owner start CAS — independent limited review

Verdict: no unresolved must-fix found within this narrow delta. This is not whole Factory/product acceptance, formal PR approval, or independent runtime verification.

Frozen source: `db949cb57d6fbcec5a47f1b2da8c271d29b1732e`, parent `c3e547308e549afc5bede12513686c9b2635998f`. Initial inspection observed precisely two modified files; the subsequent frozen commit contains those same two files. All 46 production bindings plus one integration-test binding are byte-identical to the candidate manifest and frozen git objects. Both candidate executables and all 15 manifest evidence items were independently SHA-256 checked. No candidate binary or test ELF was executed by this reviewer.

Candidate: `/workspace/AgentBaton/.onboarding/factory-owner-cas-candidate-20261028/manifest.json`, SHA-256 `6cdfc423d2c15c5f91087350fa9742123bfdf58a1f88eca3434502f75cb5cd01`.

## Causal assessment

The previous actual IPC control on saved pre-fix production returned `CANCEL_REQUESTED/not_started`, revision 17, when the caller supplied a wrong start revision. It failed its `STALE_REVISION` assertion (exit 101, 0 passed / 1 failed). This demonstrates incorrect admission/error ordering, not a successful stale execution or a measured permission escalation. Its source, ELF, response and failed log remain intact.

The only production addition is an early comparison of trusted persisted scope revision with the new request's expected revision. In `prepare_factory_owner_read`, exact-key replay is checked first using the original scope-specific idempotency namespace and serialized scope/revision payload. A historical exact match returns no read plan. Conflicting payloads retain `CONFIG_CONFLICT`. For new requests the scope is read through the existing authenticated, bounded SQLite scope reader (including approved snapshot/digest and trusted Owner receipt); mismatch returns `STALE_REVISION/not_started` before projection, companion selection, database FD pin capture, observation construction, runtime privacy pin, or I/O-worker admission. This does not mean zero SQLite reads.

IPC calls this Factory context stage before Graph/Data contexts and before defer/spawn. Therefore a new stale request cannot first occupy the read worker and then be cancelled by its stale watch. The original actor admission still performs live authorization/input checks, exact revision comparison, run conflict checks, observation equality and transactional mutations. The early check adds no execution permission, consumes no idempotency key, bypasses no grant/freshness/fence/TTL/budget rule, and changes no schema, lockfile, DTO or resource cap. Existing stopped/historical-result semantics are unchanged.

Historical recovery after companion removal remains read-only: the preparer returns no plan on exact history, the Data stage does not prepare inputs for that history, and the normal start implementation returns its original stored result before live binding/companion inspection. Some authenticated scope/SQLite reads still occur. It does not return the latest completed run as a substitute for the original queued response.

## Evidence reviewed

Author execution, not independently rerun:

- All 13 owner-read units pass (6.06 seconds); mechanical removal of the new 9-line CAS block and new unit exactly reconstructs the prior module, preserving the previous 12 units byte-for-byte.
- New unit removes the approved companion before submitting a wrong CAS, asserts `STALE_REVISION/not_started`, no observation plan, unchanged event sequence and zero runs.
- One actual Data-workspace IPC test passes (2.18 seconds). The wrong request leaves event sequence unchanged, six execution-related tables at zero and no idempotency record. The same unused key with the correct CAS then runs the existing complete parent/child/resumed-parent lifecycle. After completion it deletes the companion, repeats the original exact key, asserts the exact original queued result, unchanged sequence and exactly three existing Factory attempts.
- Actual debug daemon/CLI build passes (15.64 seconds), Core all-target Clippy `-D warnings` passes (8.38 seconds), and format check passes. Logs and binaries are hash-bound in `VERIFICATION.json`; no wider full-workspace result is inferred from them.

Independent checks performed here: 47 frozen-source/hash checks, all 15 evidence-file checks, both binary size/hash checks, source-order assertions for replay/CAS/projection/IPC-defer, and prior-unit byte-preservation. These static probes are not additional Rust test cases. Reviewer invoked no Cargo, test ELF, daemon, browser, Native UI or remote operation and changed no repository source/lock/schema/STATUS/target.

## Limits

This delta does not independently prove the full mixed ledger/ancestor/fault/unknown/restart matrix, large Data cancellation latency, metadata/Git-helper stalls, descriptor pressure, Mac Native/GUI behavior, real Provider/model execution or full Factory acceptance. Previously sealed owner-read and narrow-budget reviews remain separate. A bounded metadata observation is not continuous OS immutability. The saved uncompiled large-Data WIP is not execution evidence. No Cargo target lease was acquired or retained by this review.
