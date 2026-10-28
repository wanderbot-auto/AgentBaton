# Progress

- Isolated worktree created from exact delegated base; other trees preserved.
- ARM raw log SHA verified as
  `dcd64d5e1475e0acbf3f9042f76db918840d31f735b92d6aa04822d53dd6da0f`.
  Original suite: 18 passed, 1 failed, 0 ignored. Failing helper line119
  unwrapped Timeout/Unknown/no protocol error; mode was absent.
- Production source confirms executable digest verification/spawn precedes
  `exchange`'s `Instant::now() + timeout`; hash duration cannot consume that
  initialize deadline. Failure does not identify fault-mode wait/reap behavior.
  The separate 4MiB stderr/500ms test passed in that ARM log.
- Added caller monotonic preparation/start durations and bounded failure-only
  summaries of existing fixture.pid/wire.ndjson. No added writes or warm-up on
  the original fixture path. Recorded response does not establish stdout flush
  or parent receipt. Start duration includes verification/spawn, not solely init.
- Original 19 cases remain; one added regression checks an actual generated
  delayed child times out and is reaped, and poisoned wire cannot escape into
  diagnostic output. Root arranges independent review; this author self-reviews.
- Initial direct rustfmt lacked RUSTUP_HOME and failed creating read-only default
  home; corrected to provided toolchain homes before formatting. No source was
  changed by that failed invocation.
- First Linux run: 20 passed, 0 failed/ignored/filtered, 20.67s. Self-review then
  removed an availability assumption from the new regression: if Python was not
  scheduled before the deadline, the missing PID record is valid evidence. When
  a PID record exists, explicitly verify the child was reaped. Existing cases
  and original fixture remain unchanged.
- Final Linux run: 20 passed, 0 failed/ignored/filtered, 20.44s. Includes all 19
  original cases, original five-mode fault loop and original 4MiB/500ms stderr
  test. New delayed-start failure is intentional and is separately labeled.
- Python profile: one planned startup sample per original fault mode, all 5
  valid initialize responses and 5 children reaped. No warm-up/retry. Observed
  initialize 17.616–22.246ms. This profile does not exercise turn-time faults,
  the Rust transport, macOS scheduling or a vendor executable.
- Source commit: `f61842cf7d03b3967ceff60a0e6883d0d37dd682`. Private target 151MiB.
  Source self-review and rustfmt/diff checks passed. Independent review remains
  assigned by Root. No push/merge/CI interaction was performed.
- Original ARM startup failure was not reproduced. Its root cause remains
  unknown; no fixture/production performance fix or deadline change is justified
  by this evidence.
