# Source checkpoint

Branch `codex/integration-finished-stage-service`, source `dd1bf441bb51b9bd67f06895c2838881cd0e8fd3`, base `77792da462278b0ba812611b473ec631d9e26137`. Root must cherry-pick source and records into its active candidate without changing that candidate during an owned execution. Author changes are only two permitted production files and these self-owned records; no lockfile/migration/test-budget/CI modification.

Next: independent review should check live cancellation/lineage/fence/queue-clock invariants and unchanged original full scheduling order. Root then runs exact full integration IPC14 using the existing bounded runner and supported single-link Git PATH, followed by relevant Core/format/Clippy and real UI pairing as required. A later success does not prove this optimization's magnitude or that scans caused the old failure; collect real phase evidence if the unchanged limits still fail.

Preserve exact old full14:13pass/1fail at777, command log SHA15e801596af1c38c22ea6dd71d51d86a10916eca87d6da4dd9ed8617ee9a2686 and the retained failed ELF/database. No raw database is included here. No new fee, provider/model, credential, remote-host, signing or production authority is requested.

## Independent P2 correction — source `59f2f3048fbe671ebb15069a69e9b0f03a6cef5b`

Independent review found that the new between-Factory-and-Goal service could execute after tick_factory retained factory_fatal_error while returning Ok. The original first guard runs before Factory and cannot observe that new error. This new service widens a known-fatal completion/continuation opportunity; the old later full pass also had a pre-existing fatal-propagation gap, so the whole path is not attributed to the new code. The original dd1bf44 finding remains preserved.

Add only an immediate take/return of the retained fatal before the new second service. This conservatively prevents subsequent Goal/full-integration admission on that iteration too; it does not settle from an error, grant permission, relax budget/clock or refactor the scheduler. Removing these exact five lines restores the complete pre-correction IPC file.

Daemon's existing error/unwind Drop requests integration cancellation, consumes original read/target/stage JoinHandles and retained Command context before unlinking the socket/unlocking writer.lock. join_stages does not mint settled receipts or fees; unjournalled observations remain subject to unknown-outcome reconciliation on restart. This is a source-lifetime check for owned product workers, not a newly executed panic/kill experiment or a universal claim about unrelated socket reader threads. Original cancellation, fee and unknown semantics remain unchanged.

Rustfmt, diff whitespace and exact inverse source check passed. New exact source compilation, actual full14 and fatal-error shutdown runtime remain NOT_RUN by this author. Root must obtain independent closure review and execute applicable regressions; the prior timing-failure and new P2 history remain intact.
