# Progress

Implemented test-only source commit `2ee515e7b2d97b376740a7514ffc2a7590bca5ca`.

The original Intel CI failed with fresh grant remaining997ms, sessions.create1011ms and Task cancelled. The original failure had no stop/request/event diagnostic, so it does not by itself prove a Task unknown-to-cancelled transition.

Local original-TTL diagnostics passed1/1 in3.81s. Actual ordering: session.outcome_unknown(seq24, provider_session_creation_failed), attempt.stopped25, session.stopped27(session_creation_failed, process_reapedtrue). Task outcome_unknown; project consumed2, new root1, all active0.

Controlled original-TTL negative used only a generated fixture1100ms session/new delay. It failed the original exact unknown assertion1/1 in4.34s: policy_stop_requested24 preceded session.outcome_unknown25, task.cancel_requested26, attempt.stopped27 and session.stopped29. The Task advanced from running revision3 to cancellation revision4/5. No Task unknown-to-cancelled transition was observed. session_failure changes session metadata, not Task state; its event name alone cannot prove Task unknown. This controlled run can hit the original1s runtime IO timeout, so it is not claimed as successful malformed-response handling.

The final positive keeps that real fixed delay, grants10000ms, verifies the malformed-response marker and original exact request sequence, and rejects any policy_stop_requested during startup. After true owned stop it explicitly revokes the same runtime grant, asserts revokedtrue, then approves/runs the independent fixed command. Task remains outcome_unknown, command task_acceptance is not_established. Root/project counters remain1/2, active0, actual Task has exactly one attempt and one session.

Final author execution: full14 tests PASS18.53s, Core all-target Clippy -Dwarnings PASS9.44s, workspace fmt and diff checks PASS. Original expiry block and all other13 test functions are byte-identical to652. Linux evidence only; independent review and new Mac CI pending.
