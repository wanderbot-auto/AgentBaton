# Graph Mac fixture stability — test-only plan

Design v0.2 / AB-START-001, base652c5f0bc4983f53375ccb10d897c1088cd0e57f. Related validation obligations: AB-TSK-008 unknown recovery, AB-TSK-010 honest metering, AB-TSK-012 root budgets, AB-SEC-001 authority/enforcement and AB-SEC-006 fault recovery. This leaf hardens existing test fixtures; it does not mark these full product requirements accepted.

1. Preserve actual652 ARM source/raw failure: Corelib69 and command14 pass, sessions53pass2fail. Integrity's failed RPC method/stage and recovery bind errno are not in the historical log; do not invent them.
2. Keep all production/default/lock/CI/negative budgets unchanged. Match only Integrity's enclosing bootstrap/review Grant lifetime to existing Data/Command30000ms; retain Graph runtime10000/node4000/5000/queue1200/command500/1500 and expiry controls. Add failed RPC method/key/elapsed and Integrity Grant timestamps/phases.
3. Use a short private recovery sibling while preserving0700 directory/0600 snapshot DB, original daemon shutdown/join, recovered unknown/no replay/fence/meter checks. After the snapshot is frozen and original daemon stopped, probe an exclusively created private same-length sibling socket shape and record actual bind outcome/OS error without deleting an unknown path. Do not claim a Linux result proves Darwin or reconstructs historical errno.
4. Freeze source first. Until Root explicitly leases a Cargo target, perform only standalone format/parser and mechanical scope checks; no Cargo/Chromium. Then run actual55 sessions tests plus required Clippy/fmt on that one target, preserve failures, and obtain independent review from another agent. Root owns integration/new exact Mac CI.

Allowed source path: crates/core/tests/sessions_ipc.rs plus this unit's four documents. No product, other tests, migration, locks, CI, credentials, accounts, production, spending, remote mutation or formal Approve.
