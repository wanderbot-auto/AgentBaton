# Three-round browser lifecycle fixture correction

Requirements: AB-VAL-009, AB-PRJ-004, AB-PRJ-005; approved Design v0.2 slice under AB-START-001. Base `8a6708e63fb22cb41178310451b19861ff2ceed1`.

The actual three-round browser fixture requested a fresh UI start while the independent Core watcher already held active integration work. Its context/target reads honestly returned WORKSPACE_BUSY, keeping the UI start disabled. Preserve that product guard. Observe the watcher through the original bounded read/finished loop first, then refresh the real UI and require its explicit same-run check. Do not infer that the UI created the run.

Keep the exact original observation call, gate health, stage/Command/Grant bounds, false/false/true receipts, seven settled stages, eight charged attempts, sole CAS, source preservation, same-run result and seven original screenshots. Add a pure order control that rejects the original order and retain the real failure without replaying its unknown outcome. Root must independently review, rebuild and freeze the exact new HEAD, and run the real browser; these source/pure records do not prove runtime success.


## Actual asynchronous same-run reply boundary

Root's later fresh fixture passed the terminal/wake-enabled boundary but failed the immediate post-click audit assertion. After completion, its private transcript contained exactly one successful same-run start response and the retained runtime was confirmed with false,false,true receipts. That does not make the failed browser assertion a pass: the JavaScript click initiated an asynchronous request whose Host audit append occurs only after the real Core reply. The original retained fixture is not replayed.

Use one original15-second absolute observation deadline before the original terminal read. Both terminal observation and the post-click read-only audit observation receive only its remaining time, with explicit checks before/after audit reads and after each observer returns. UI refresh/enabled/click time consumes the same deadline. The observer reads Host.audit under its original lock, accepts only one completed error-free response for the confirmed run and rejects duplicate, foreign, malformed or late responses. It sends no request, appends no audit data and never renews a bound. The original final exact-one/same-ID assertion and all terminal/history/fees/CAS/screenshot assertions remain.

Actual pure validation: 18 round-driver controls (prior12 plus6 reply/deadline controls), original35 and original18 publisher controls,71 unique pure tests;11 static inverse/unchanged checks pass. These are source/pure checks, not browser/Core/macOS acceptance. A first static verifier selected the new helper's similarly named local starts list instead of run_case's final audit; its failure record is retained and the selector was corrected without source changes.
