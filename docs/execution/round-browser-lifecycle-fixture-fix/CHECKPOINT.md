# Checkpoint

Source `e2b6ddb69302199523bdaa85a5634ebc1eedfd60` on `codex/round-browser-lifecycle-fixture-fix`, base `8a6708e63fb22cb41178310451b19861ff2ceed1`. Only the new round driver and its corresponding pure tests changed. External evidence: `/workspace/.onboarding/round-browser-lifecycle-fixture-fix-source1/`.

Original runtime failure remains attributed to Root8a: one passed check, zero screenshots; the pre-terminal UI start stayed disabled after busy context/target reads. Root reports that owned cleanup stopped the daemon with exit0/no forced kill and joined the gate, while interrupted third preparation retained outcome_unknown/unsettled reservation/fence. Daemon cleanup does not prove a settled third stage. The retained fixture `/tmp/ab-round-ui-o57kfz6f` is not replayed here.

Next: independent source review, Root exact-HEAD build/frontend/freeze and fresh browser run with every original assertion. Current source/pure PASS is not browser/Native/macOS/Provider/full-product acceptance. Permission, deadline, budget, retry and ownership behavior are unchanged; live Provider, production, signing and added spending remain closed.
