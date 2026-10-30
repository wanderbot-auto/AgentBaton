# Progress

Functional source frozen at `6853844c87a54de41fdd86fd34d96e6ed2482817`. Two changed files only. Added one private predicate plus bounded changes in collect_started_records: known-provider disappearance branch; strict present PID/birth gate; getpgid/final-identity missing race after valid argv. No new persistent/process/stop/acceptance state is created. Whole-module specified inverse restores baseline bytes; every pre-existing other function/class remains byte-identical, including FixtureControllerReaper.

Actual author commands:

- `python3 -B -m unittest discover -s tests/infrastructure -p test_factory_bound_provider_exit.py -v` on original de0b collector:20 tests,14PASS/4FAIL/2ERROR, exit1,1.242s. This is a deterministic unit countercontrol, not six product/CI failures.
- Exact same test bytes after narrow fix:20PASS/0FAIL/0ERROR, exit0,1.162s. Old/new test SHA identical. Controls load actual collector/FD helpers and actual JSON duplicate checks via AST, use own temporary files and minimal SQLite relation surrogates, and inject proc/cmdline/getpgid outcomes. They never import/launch Playwright, Core, Provider, Actor or a process helper, and never read real /proc for their fake PIDs.
- Two Python AST syntax checks, whole-source explicit inverse equality, unchanged old functions/classes, and `git diff --check`: exit0.

Four old positive exit observations failed/errored before the fix; initial foreign-birth countercontrol also failed because old code skipped it. The remaining old difference is a permission-error type: old strict diagnostic wrapped it as AssertionError, new precise lookup propagates PermissionError. Both behaviors fail closed; this is not reported as an old permission bypass. No tests were deleted, relaxed or changed between red/green runs.

Old actual CI failure remains failed: Product40976684974/UI123806349231 for PR18 headde0b, CP01/CP02 passed then collector assertion. Shared child-planning caller's later checks were not reached; cleanup's six identities subsequently absent does not retroactively complete the workflow. Other Graph/Factory passes were reported by Root and were not reaudited/relabelled here. This is not the older Goal observer live-empty/pre-exec case or a macOS failure.

The new tree initially had a Root-created empty no-checkout index; author detected missing files without editing/resetting it. Root confirmed no user content and performed initial materialization. Author started source edits only after clean exactde0b. Original shared .onboarding and unknown user files were untouched.

Independent review and new actual Core/browser/CI are pending. Root owns those serialized operations. No Cargo, Node, browser, daemon, generated ACP/MCP, real Provider/account, network/GitHub mutation, signal or OS wait was executed by this author. Unit MCP reaper test is a call-sequence stub only; actual OS reaper code is preserved, not newly proven by that stub.
