# Bound child Goal readonly observation

Requirements AB-TSK-008/009/010, WP08, AB-START-001. Base f55a805a952f84dc65e83c5e33f98e975871c3f1. This is a test-harness correction for current schema15, not new production permission or a schema change.

The exact normal-case CI first failure follows owner.click while waiting for actual TaskAccepted. A strict Core readonly-history busy reply currently escapes PrivateHost.result and aborts the original polling loop. Use a narrow bound child-goal get observer; keep the generic Host/result, Owner/start/cancel writes and all business assertions.

Bind only scope/task/Goal IDs already obtained from the actual start and Bounds map. Each of the same five original read-observation sites retains one15s acceptance deadline, original pump/capture and20ms poll cadence. Existing ObservedHost.wire retains raw replies, finite diagnostics,5s socket and recorder limits and16MiB capture bound. Only the exact current error envelope WORKSPACE_BUSY / bool true / not_started with no result is pending. All other errors, transport failures, foreign bindings and late replies remain failures. No fresh operation retry pool, no second Owner click and no deadline reset.

Run closed pure controls and old finite/publisher/argv suites, independent review, then Root runs same-source browser normal and other applicable paths. Existing finite v2 already records real busy counts. A passing pure control does not establish the original CI race is resolved. Socket/recorder waits cannot be kernel-preempted by this helper; it refuses success after its original acceptance deadline.
