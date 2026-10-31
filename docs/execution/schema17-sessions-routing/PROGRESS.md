# Progress

The retained Root run `/workspace/.onboarding/integration17-other-migration-and-sessions1` exited 101: sessions had 55 passing and three failing tests. These three failures are separate from the opaque-table migration failures.

Two existing concurrent-review tests reached an Integration `NOT_FOUND` before their original `WORKSPACE_BUSY` guard because every `approvals.finalize` was passed through strict Integration lookup. The candidate now probes at most 1 MiB of original proposal text using typed `ApprovalProposal` decoding. Only `kind=integration` continues through the unchanged strict Integration lookup and request-owned native observation. Missing, oversized, malformed, duplicate-key, unknown-field or other-kind records return to the original routes; fallback is not approval.

The accepted-history tamper test now opens a valid Store before external corruption and executes all original `get`, `list`, Graph-details and historical-finalization refusals for all 17 cases. For `task-project` and `task-id`, cold open separately must reject with the precise schema17 Task-binding error and preserve database/journal bytes. The other 15 cases also execute all original guards after cold open.

Three unit-test sources cover domain routing, malformed/duplicate/unknown/oversized fallback refusal and original exact-byte/native-finalizer/ID boundaries. Existing concurrent-review expectations are unchanged.

Only source/static checks and rustfmt were executed by the author. Compilation, SQLite, actual IPC and CI remain unrun for this candidate.
