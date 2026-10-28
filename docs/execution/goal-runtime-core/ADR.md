# Runtime authority and atomic reservations

Use a dedicated goal_execution native proposal for the exact submitted plan and displayed run limits. This avoids silently replacing the old preview-only UI promise. Renderer flags cannot finalize it; the fixed private finalizer consumes proposal+run atomically and returns a stable run receipt.

One scheduler in the daemon owns dispatch. Reuse the existing session/dispatch journal, supervisor, workspace lease and current-data witness flow. A crate-private dispatch context is constructed from authoritative run/node records, never deserialized from IPC. Extend the existing session IMMEDIATE transaction with a Goal hook that validates exact plan/scope/ledger CAS and reserves root/project/ancestor dimensions before session/intent commit. Existing generic session semantics remain unchanged.

Charge conservative declared per-attempt runtime/output reservations cumulatively; expose charged bounds separately from actual observations. Actual runtime/output/token/cost are unknown unless measured by the owned executor. Never fabricate zero or refund unknown usage. The owned runtime duration and existing transport caps do not establish an OS filesystem quota or sandbox. Overshoot, missing stop or unverifiable effects block further execution.

The existing isolated workspace may be used only within its original explicit grant. New factory-created workspaces require an additional native factory capability and policy/grant lineage checks; this unit must refuse unapproved factory requests rather than weakening is_narrower_than or borrowing old Goal ceilings.

Normal protocol completion is candidate execution evidence. It is not independent verification, owner acceptance, integration or production release. Unknown effects prevent automatic replay even if the process has been reaped. Pending validation/owner nodes remain explicit waits until real bound receipts exist.
