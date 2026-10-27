# Progress

Original CI error and checkout verified. Old fixture grants/finalizes1000ms authority, then creates its task, then admits the session. The explicit old-sequence control inserted actual1100ms task preparation and reproduced PERMISSION_DENIED/not_started at sessions.create; grant return→request1111ms while admission itself11ms. Actual old test binary is saved independently with hash for reviewer replay.

Reordered only preparation/approval in the test. Task creation now precedes final grant, including the same1100ms preparation. Added an expired-grant negative control:1100ms real wait after approval; original1000ms budget unchanged; admission rejected with0sessions/0attempts, unchanged lease and no generated ACP request marker. Fresh final approval reported999ms TTL remaining, immediate request0ms and admission225ms on this Linux host.

Strengthened actual lifecycle observations: generated executable records initialize/session/new only; stop journal requires process_reaped=true/no_process_started=false/reason=session_creation_failed/task_accepted=false. Original stopped intent, empty lease, TaskOutcomeUnknown, command_passed and no TaskAccepted assertions remain. Fresh schema8 runtime assertion prevents accidentally testing shared Goal9 rmeta. A source-entry mtime refresh forced exact Core8 rebuild without modifying production bytes.

Actual fresh control,14 command tests, Core all-targets Clippy -Dwarnings and formatting PASS. Intermediate added-query compile/type and SQL-column mistakes were retained then corrected, without lowering any deadline or assertion. Actual new Mac candidate CI is pending; the controlled mechanism is not proof of the historical Intel timing cause. No production/lib/lock/gate/source behavior changed.
