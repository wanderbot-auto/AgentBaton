# Restore bounded readonly polling after an exact Busy reply

Base `16a968d240f7680bf44332aafd7f3c98aae457a0` (Parent15 line); independent sparse branch `codex/child-goal-readonly-poll-recovery` under `/tmp`. AB-START-001 remains the authority; Root explicitly authorized the component/test scope and the necessary minimal API error-metadata propagation. Root owns independent review and all Node/Core/browser/SQL/CI execution.

The source control flow is established: refresh clears `fresh`, its catch keeps it false, and the old automatic polling effect required true. A transient readonly Busy could therefore stop polling a previously known running Goal. Fix only the precise typed `WORKSPACE_BUSY`, strict `retryable=true`, `side_effect_state=not_started` case. Existing three-argument DaemonError construction must retain undefined retry permission; never infer permission from a code/string flag.

Bind recovery to the refresh's epoch, parent identity and same complete known polling Goal snapshot. Keep stale writes/Owner actions closed, pending/unknown/disconnected/changed-authority states blocked, original independent cancellation-summary semantics, and the existing twenty automatic reads at one-second intervals. No new retry pool, automatic write, timeout or protocol/Actor change. Preserve manual refresh's existing explicit budget reset.

Write meaningful component and transport controls, but leave Node/typecheck/runtime execution to Root. This fixes a source-established behavior, not a proven cause of the old remote normal-case failure.
