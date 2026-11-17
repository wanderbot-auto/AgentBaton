# RHOST-001 delivery for leader

Draft PR: https://github.com/wanderbot-auto/AgentBaton/pull/30 (`main` ← `collab/remote-host-foundation`). Do not merge automatically. This delivers an independent foundation, not complete remote product acceptance.

- Base/exact fetched target: `b78ffbb5bcbe741de2e0cd8fcf2e568c4c7ef28c`.
- Clean implementation and verified commit: `9457b43b42858168b7a91e4c7c97c7c3a04ae111`.
- Final evidence-only descendant's exact head is published in the PR and leader handoff; no product/test/verifier source changes after this tested commit. This document cannot embed its own Git SHA.
- Source set SHA-256: `dad334672594171b9054f0509ba4b6a6828b716935bdeb5056d718fd042ff7f7`.
- Owned write scope only: `crates/remote-host/**`, `scripts/verify-remote-host.py`, `tests/infrastructure/test_remote_host.py`, `docs/execution/remote-host-foundation/**`.

## Interface and acceptance evidence

[ADR-001](ADR-001.md) fixes strict SSH/key-to-Host registration, trusted local bounded grant provisioning, complete execution binding and mapping, lease/fence/budget, framing limits and error paths. `Client` provides real private stdio and SSH adapters; `Ledger` persists local offline-cancel/cursors without credentials. Companion owns a restricted record worker, durable intent/receipt/events and real child-wait proof. A lease reopen pauses new effects; reconcile/replay/cancel continue. New fence cannot reset budget, expand ceiling or reuse an old unresolved intent.

Run `bash scripts/collab.sh run -- python3 scripts/verify-remote-host.py` explicitly; root cargo/required CI does not discover this standalone workspace. On macOS ARM64 with rustc 1.91.1, the clean implementation commit passed fmt, 18 Rust integration tests, strict all-target clippy (`-D warnings`), locked explicit build and 16 Python actual subprocess fault tests. The final evidence run is [20261117T153134Z-819857000](runs/20261117T153134Z-819857000/evidence.json); complete historical failures remain committed and are not acceptance evidence.

Failure coverage includes identity/config/root inode drift, auth/permission/budget/expiry, lock wait and rollback, stale controllers/grants, lease rotation, duplicate/conflicting payloads, loss of real response/ACK, disconnect/reopen/offline cancel, actual worker stop, whole-group crash recovery, before/after-effect journal failures, busy database, future WAL/corrupt schema, symlink/traversal, half/oversized/timeout frames, idle and event pagination/capacity. Assertions check actual files/effect counts, durable reservations, unknown states and child wait; not only return values.

- Development binary SHA-256: `d9d8b38815b4a74ca36be6ecf04bde84d4d708b891ba3ad5388d8510429b4bb0` (task target path in evidence; not a signed/released product binary).
- Sanitized actual stdio trace SHA-256: `6022140d15dbbc9a7c5378d70f0e13ce7097e27c4e118ec532103a2fca91035e` ([trace](runs/20261117T153134Z-819857000/stdio-trace.jsonl)). Outbound metadata excludes credential/body; inbound results use synthetic test data. Each owned supervisor exit and no-surviving-group check, plus resulting synthetic file sizes/hashes, is included.
- Log hashes and exact argv/environment/timings: [EVIDENCE.json](EVIDENCE.json), per-run manifest. Raw logs preserved byte-for-byte; task-local git whitespace attribute applies only to generated logs.
- Independent read-only source/ADR review: [REVIEW.md](REVIEW.md); all reported issues fixed. Reviewer did not run tests; leader still owns independent admission.
- Scope, diff whitespace and conflict preview passed against fetched target. Final descendant conflict preview is recorded in the PR/leader handoff against the latest fetched target. No root Cargo/Core IPC/database migrations/desktop/CI/global ledger changes.

## Process ownership and continuation

All test subprocesses were owned; ordinary paths close EOF and join, timeout paths kill/wait, whole-group crash remains explicitly unknown and is OS reaped. Suite asserts no survivors, and post-run process inspection found zero processes using this task's companion binary. Generated identities and per-test data were removed. No login service, remote listener, Provider, paid resource or background executor remains. Task runtime/target is retained for review/rebuild as collab prescribes. B released the build/test slot to PR29 A/B work and does not promise background continuation.

Scope reservation is retained during review so fixes return to B. `CHECKPOINT.md` records review state and docs-only dirty paths before final evidence commit; final clean state/head comes from Git/PR handoff. Leader updates global STATUS and requirement ledger; B did not write either.

## Remaining boundaries and integration dependencies

Actual SSH authentication/host-key rejection/packet partition, real cross-machine macOS↔Linux, Linux execution, and Provider compatibility are **not_run** pending approved Host/resources. Generated SSH identities, local pipes and `ssh -G` prove their respective local dimensions only.

AB-EXT-001/007/009/011 have a real foundation and failure evidence, not full requirement acceptance. Follow-up packages must provide trusted Core Host/key registration and install/provisioning path, shared root/project budgets across attempts, Provider executor, Core IPC/local history, routing/UI, data transport/version policy, remote retention/unpin and service lifecycle. AB-EXT-002/008/010 boundaries remain for that integration. Root/desktop/CI entry wiring is intentionally outside this claim. Production signing/release and any real remote/Provider accounts require their existing separate gates.

## Leader capacity review repair

The first-control failure is repaired without raising any limits. New effects/grants still reserve96 slots; settlement keeps one per pending wait plus one per unsettled worker terminal, each missing first control, and a necessary reopen. Worst initial debt is64+2+1=67. First controls do not consume promised completion slots, and unchanged controls/paused opens append no repeated state event. All terminal append paths enforce10000; 32jobs and4096receipts remain unchanged. In the 32 actual worker test, event counts are9904→9936→9938→9970→9971, real waits precede confirmed stop, and 32file effects/consumed operations remain intact.

The original legal9903 seed regression red log and intermediate failed verifier are preserved under review-capacity/ and runs/. The final clean implementation commit above passes18 Rust+16 actual subprocess tests. Native read-only targeted review confirms the corrected debt formula, no remaining capacity blocker. Receipt first-control natural exhaustion is not claimed by this finding.
