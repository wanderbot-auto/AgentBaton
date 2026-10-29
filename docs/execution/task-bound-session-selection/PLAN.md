# Task-bound workspace convenience in ManagedSessions

Requirements AB-PRJ-002/003/005 and AB-SEC-007, OWNER-002. ACK OWNER-001/002 and AB-START-001 product gate. Base `3008c8f55eba5921aec9d0f33acaca02076ae693`; independent `codex/task-bound-session-selection`. Prior TaskCodeWorkspace/shared-budget/Data trees and failures remain intact.

Only ManagedSessions, session-i18n, dedicated test, necessary original SessionWorkbench fixed-read mock/reset and these4records may change. No App/TaskCodeWorkspace/API/Core/Native/schema/locks/CI edits.

Read current Task through existing fixed taskGet. For a draft Task with a structurally checked matching task/spec/project revision/local/ready binding, locate only its receipt.workspace_id if exactly one matching same-project ready workspace also has its recorded managed root and source base. This is convenience selection, not fresh FS identity, a Grant, Owner approval or execution admission. No implicit prepare, Grant, Session or Provider call.

Pending/unknown/malformed/stale/missing/foreign binding/workspace cannot fall back to another workspace or reuse acknowledgement/Grant. Preserve explicit legacy unbound Task workspace and global manual Grant creation with no Task selected. Scope/task request epochs, read floors and acknowledgement/Grant context must survive async ordering. Root owns independent review and all Node/type/build/real IPC/browser execution.


## Independent P2 follow-up 0db56038faae62d2b458cd6bc8909d0980d6275c

Original f18 source had a synchronous parent-prop transition gap: same-project initialTaskId A→B could render stateTaskA before its passive synchronization effect; old freshness/ack/Grant/action handler context did not include the new initial props. Independent source/finite model finding is preserved, not claimed as an executed React reproduction. Root separately reported original f18 typecheck and targeted38 (26new+12legacy) PASS; these remain original-source results, not proof that this timing gap was absent.

Narrow fix uses local parent-initial identity plus render epoch (including A→B→A), matched against an applied-state stamp. The stamp enters Task-source and selection context, synchronously invalidating old TaskFresh/ack/Grant/handler context. Until effect applies TaskB and its stamp, refreshTask returns before requesting oldA; B must then complete its own fresh fixed Get. This is local UI provenance, never wire/approval authority. Subsequent manual Task selection is allowed; it does not have to equal initialTaskId. Existing active Session inspection/send/cancel code is byte unchanged and not gated by this new setup epoch.

Added2source-only specs: the actual equality helper's sync/ABA predicate and RTL parentB plus deferred B Get, rejection of a new A request during transition, and later manual A selection. Whole old26test inverse is byte-equal; legacy12 test file unchanged. New declared28, planned targeted40; all new-source type/Node/test/build/actual IPC/browser remain NOT_RUN. Python source/inverse and diff0 only. Root will independently test.
