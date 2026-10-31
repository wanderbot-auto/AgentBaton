# Checkpoint

Branch: `codex/schema17-sessions-routing`; isolated worktree, based on `4f962940533241fdec74b96a87c01a99788a62ee`. Allowed source changes are Integration approval classification, its one IPC preflight call and sessions corruption-test coverage. Actor scheduling and the Owner-read-priority candidate are untouched.

Next: independent review; integrate only reviewed source; Root compile the three new unit-test sources and run all 58 sessions tests with the actual supported Git path and declared frozen companions. Preserve the original failed run and any new failure. Then re-run relevant Core/Integration/native checks before acceptance. This checkpoint is not a product completion or test pass.

Latest source correction addresses Root candidate `c5dcf0490d557da4992a565ed0e2bd5dd2b9410e`'s real `task-project` cold-open test failure. The batch was still running at diagnosis; retain its eventual full result separately. The fixture now explicitly proves its valid accepted Task has no Task-managed binding. Only Task row-ID mutation must fail startup; all 16 other corruption cases must reopen and execute all four original history guards. Independent review and actual rerun remain required.
