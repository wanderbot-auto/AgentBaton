# Source progress

Source `dd1bf441bb51b9bd67f06895c2838881cd0e8fd3` based on `77792da462278b0ba812611b473ec631d9e26137` adds20 lines across integration_workers.rs and ipc.rs. The complete original tick_stages body and armed/read/shutdown paths remain byte-identical. Removing the two new IPC call/comment blocks restores the complete IPC file, and removing the new worker method restores the complete worker file.

Source rustfmt check and git diff --check passed using the existing explicit /workspace toolchain. The author ran no Cargo, product/Actor, Node/browser, Provider, native or Git product fixture operation. No new test count is claimed. Full14, Core, Clippy and runtime/performance remain NOT_RUN for this source. This is author source delivery, not independent SOURCE_READY or formal PR Approve.

## Independent P2 correction — source `59f2f3048fbe671ebb15069a69e9b0f03a6cef5b`

Independent review found that the new between-Factory-and-Goal service could execute after tick_factory retained factory_fatal_error while returning Ok. The original first guard runs before Factory and cannot observe that new error. This new service widens a known-fatal completion/continuation opportunity; the old later full pass also had a pre-existing fatal-propagation gap, so the whole path is not attributed to the new code. The original dd1bf44 finding remains preserved.

Add only an immediate take/return of the retained fatal before the new second service. This conservatively prevents subsequent Goal/full-integration admission on that iteration too; it does not settle from an error, grant permission, relax budget/clock or refactor the scheduler. Removing these exact five lines restores the complete pre-correction IPC file.

Daemon's existing error/unwind Drop requests integration cancellation, consumes original read/target/stage JoinHandles and retained Command context before unlinking the socket/unlocking writer.lock. join_stages does not mint settled receipts or fees; unjournalled observations remain subject to unknown-outcome reconciliation on restart. This is a source-lifetime check for owned product workers, not a newly executed panic/kill experiment or a universal claim about unrelated socket reader threads. Original cancellation, fee and unknown semantics remain unchanged.

Rustfmt, diff whitespace and exact inverse source check passed. New exact source compilation, actual full14 and fatal-error shutdown runtime remain NOT_RUN by this author. Root must obtain independent closure review and execute applicable regressions; the prior timing-failure and new P2 history remain intact.
