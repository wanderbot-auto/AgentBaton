# RHOST-001 delivery for leader

Draft PR: https://github.com/wanderbot-auto/AgentBaton/pull/30 (`main` ← `collab/remote-host-foundation`). Do not merge automatically. This delivers an independent foundation, not complete remote product acceptance.

- Base/exact fetched target: `b78ffbb5bcbe741de2e0cd8fcf2e568c4c7ef28c`.
- Clean implementation and verified commit: `6df13c36b5bbf82c608ff104588f0cd98e238121`.
- Final evidence-only descendant's exact head is published in the PR and leader handoff; no product/test/verifier source changes after this tested commit. This document cannot embed its own Git SHA.
- Source set SHA-256: `0e31277319804c089c8142ebef68a1f44cc05ffd7eb844517f75011794d3a2e3`.
- Owned write scope only: `crates/remote-host/**`, `scripts/verify-remote-host.py`, `tests/infrastructure/test_remote_host.py`, `docs/execution/remote-host-foundation/**`.

## Interface and acceptance evidence

[ADR-001](ADR-001.md) fixes strict SSH/key-to-Host registration, trusted local bounded grant provisioning, complete execution binding and mapping, lease/fence/budget, framing limits and error paths. `Client` provides real private stdio and SSH adapters; `Ledger` persists local offline-cancel/cursors without credentials. Companion owns a restricted record worker, durable intent/receipt/events and real child-wait proof. A lease reopen pauses new effects; reconcile/replay/cancel continue. New fence cannot reset budget, expand ceiling or reuse an old unresolved intent.

Run `bash scripts/collab.sh run -- python3 scripts/verify-remote-host.py` explicitly; root cargo/required CI does not discover this standalone workspace. On macOS ARM64 with rustc 1.91.1, the clean implementation commit passed fmt, 16 Rust integration tests, strict all-target clippy (`-D warnings`), locked explicit build and 16 Python actual subprocess fault tests. The final evidence run is [20261117T150538Z-544397000](runs/20261117T150538Z-544397000/evidence.json); complete historical failures remain committed and are not acceptance evidence.

Failure coverage includes identity/config/root inode drift, auth/permission/budget/expiry, lock wait and rollback, stale controllers/grants, lease rotation, duplicate/conflicting payloads, loss of real response/ACK, disconnect/reopen/offline cancel, actual worker stop, whole-group crash recovery, before/after-effect journal failures, busy database, future WAL/corrupt schema, symlink/traversal, half/oversized/timeout frames, idle and event pagination/capacity. Assertions check actual files/effect counts, durable reservations, unknown states and child wait; not only return values.

- Development binary SHA-256: `f841f43a72013b26602a101454f794a2fe671352bef53d9f746f3ad6f4c74468` (task target path in evidence; not a signed/released product binary).
- Sanitized actual stdio trace SHA-256: `eabc9b37f556734c5b5ef8386e2a9b3f876edc4f7b2aace3e526082ab99f37fa` ([trace](runs/20261117T150538Z-544397000/stdio-trace.jsonl)). Outbound metadata excludes credential/body; inbound results use synthetic test data. Each owned supervisor exit and no-surviving-group check, plus resulting synthetic file sizes/hashes, is included.
- Log hashes and exact argv/environment/timings: [EVIDENCE.json](EVIDENCE.json), per-run manifest. Raw logs preserved byte-for-byte; task-local git whitespace attribute applies only to generated logs.
- Independent read-only source/ADR review: [REVIEW.md](REVIEW.md); all reported issues fixed. Reviewer did not run tests; leader still owns independent admission.
- Scope, diff whitespace and conflict preview passed against fetched target. Preview tree for tested implementation: `8e9e3b7311bf8252bba19ec4ab2c151226bc4366`; final docs descendant requires its own reported preview. No root Cargo/Core IPC/database migrations/desktop/CI/global ledger changes.

## Process ownership and continuation

All test subprocesses were owned; ordinary paths close EOF and join, timeout paths kill/wait, whole-group crash remains explicitly unknown and is OS reaped. Suite asserts no survivors, and post-run process inspection found zero processes using this task's companion binary. Generated identities and per-test data were removed. No login service, remote listener, Provider, paid resource or background executor remains. Task runtime/target is retained for review/rebuild as collab prescribes. B released the build/test slot to PR29 A/B work and does not promise background continuation.

Scope reservation is retained during review so fixes return to B. `CHECKPOINT.md` records review state and docs-only dirty paths before final evidence commit; final clean state/head comes from Git/PR handoff. Leader updates global STATUS and requirement ledger; B did not write either.

## Remaining boundaries and integration dependencies

Actual SSH authentication/host-key rejection/packet partition, real cross-machine macOS↔Linux, Linux execution, and Provider compatibility are **not_run** pending approved Host/resources. Generated SSH identities, local pipes and `ssh -G` prove their respective local dimensions only.

AB-EXT-001/007/009/011 have a real foundation and failure evidence, not full requirement acceptance. Follow-up packages must provide trusted Core Host/key registration and install/provisioning path, shared root/project budgets across attempts, Provider executor, Core IPC/local history, routing/UI, data transport/version policy, remote retention/unpin and service lifecycle. AB-EXT-002/008/010 boundaries remain for that integration. Root/desktop/CI entry wiring is intentionally outside this claim. Production signing/release and any real remote/Provider accounts require their existing separate gates.
