# Checkpoint

Source `e2b6ddb69302199523bdaa85a5634ebc1eedfd60` on `codex/round-browser-lifecycle-fixture-fix`, base `8a6708e63fb22cb41178310451b19861ff2ceed1`. Only the new round driver and its corresponding pure tests changed. External evidence: `/workspace/.onboarding/round-browser-lifecycle-fixture-fix-source1/`.

Original runtime failure remains attributed to Root8a: one passed check, zero screenshots; the pre-terminal UI start stayed disabled after busy context/target reads. Root reports that owned cleanup stopped the daemon with exit0/no forced kill and joined the gate, while interrupted third preparation retained outcome_unknown/unsettled reservation/fence. Daemon cleanup does not prove a settled third stage. The retained fixture `/tmp/ab-round-ui-o57kfz6f` is not replayed here.

Next: independent source review, Root exact-HEAD build/frontend/freeze and fresh browser run with every original assertion. Current source/pure PASS is not browser/Native/macOS/Provider/full-product acceptance. Permission, deadline, budget, retry and ownership behavior are unchanged; live Provider, production, signing and added spending remain closed.


## Actual asynchronous same-run reply boundary

Root's later fresh fixture passed the terminal/wake-enabled boundary but failed the immediate post-click audit assertion. After completion, its private transcript contained exactly one successful same-run start response and the retained runtime was confirmed with false,false,true receipts. That does not make the failed browser assertion a pass: the JavaScript click initiated an asynchronous request whose Host audit append occurs only after the real Core reply. The original retained fixture is not replayed.

Use one original15-second absolute observation deadline before the original terminal read. Both terminal observation and the post-click read-only audit observation receive only its remaining time, with explicit checks before/after audit reads and after each observer returns. UI refresh/enabled/click time consumes the same deadline. The observer reads Host.audit under its original lock, accepts only one completed error-free response for the confirmed run and rejects duplicate, foreign, malformed or late responses. It sends no request, appends no audit data and never renews a bound. The original final exact-one/same-ID assertion and all terminal/history/fees/CAS/screenshot assertions remain.

Actual pure validation: 18 round-driver controls (prior12 plus6 reply/deadline controls), original35 and original18 publisher controls,71 unique pure tests;11 static inverse/unchanged checks pass. These are source/pure checks, not browser/Core/macOS acceptance. A first static verifier selected the new helper's similarly named local starts list instead of run_case's final audit; its failure record is retained and the selector was corrected without source changes.

Current source: `7fd3f7bf57d3b4a55f2e9e6bc4525715f440cfad`; Root fresh same-HEAD build/freeze/browser and independent review remain required.
